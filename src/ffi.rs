//! C ABI for readcon-db (always linked into cdylib/staticlib).
//!
//! Status codes mirror a small subset of readcon-core style (negative = error).

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::{Arc, Mutex};

use crate::corpus::ConCorpus;
use crate::keys::{hash_frame_bytes, ContentHash, FrameKey};
use crate::select::Select;

pub const RKRDB_OK: c_int = 0;
pub const RKRDB_ERR: c_int = -1;
pub const RKRDB_NOT_FOUND: c_int = -2;
pub const RKRDB_NULL: c_int = -3;

struct Handle {
    /// Shared so ingest runs **outside** the handle-table mutex (no app-level writer serialize).
    corpus: Arc<ConCorpus>,
    last_keys: Vec<FrameKey>,
    last_error: String,
}

static HANDLES: Mutex<Vec<Option<Box<Handle>>>> = Mutex::new(Vec::new());

fn push_handle(h: Handle) -> usize {
    let mut g = HANDLES.lock().unwrap();
    for (i, slot) in g.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(Box::new(h));
            return i;
        }
    }
    g.push(Some(Box::new(h)));
    g.len() - 1
}

/// Brief table lock for bookkeeping only — not held across ingest/select CPU or LMDB work.
fn with_handle<F, T>(id: usize, f: F) -> Result<T, c_int>
where
    F: FnOnce(&mut Handle) -> Result<T, c_int>,
{
    let mut g = HANDLES.lock().unwrap();
    let slot = g.get_mut(id).ok_or(RKRDB_NULL)?;
    let h = slot.as_mut().ok_or(RKRDB_NULL)?;
    f(h)
}

fn corpus_arc(id: usize) -> Result<Arc<ConCorpus>, c_int> {
    let g = HANDLES.lock().unwrap();
    let slot = g.get(id).ok_or(RKRDB_NULL)?;
    let h = slot.as_ref().ok_or(RKRDB_NULL)?;
    Ok(Arc::clone(&h.corpus))
}

fn set_err_id(id: usize, e: impl ToString) {
    let mut g = HANDLES.lock().unwrap();
    if let Some(Some(h)) = g.get_mut(id) {
        h.last_error = e.to_string();
    }
}

fn set_err(h: &mut Handle, e: impl ToString) {
    h.last_error = e.to_string();
}

/// Open corpus directory. On success writes opaque handle id to `out_id` (>=0).
/// Returns RKRDB_OK or error code.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_open(path: *const c_char, out_id: *mut usize) -> c_int {
    if path.is_null() || out_id.is_null() {
        return RKRDB_NULL;
    }
    let cpath = unsafe { CStr::from_ptr(path) };
    let path = match cpath.to_str() {
        Ok(s) => s,
        Err(_) => return RKRDB_ERR,
    };
    match ConCorpus::open(path) {
        Ok(corpus) => {
            let id = push_handle(Handle {
                corpus: Arc::new(corpus),
                last_keys: Vec::new(),
                last_error: String::new(),
            });
            unsafe { *out_id = id };
            RKRDB_OK
        }
        Err(_) => RKRDB_ERR,
    }
}

/// Open an existing corpus `MDB_RDONLY`. No mkdir, no write txn.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_open_readonly(path: *const c_char, out_id: *mut usize) -> c_int {
    if path.is_null() || out_id.is_null() {
        return RKRDB_NULL;
    }
    let cpath = unsafe { CStr::from_ptr(path) };
    let path = match cpath.to_str() {
        Ok(s) => s,
        Err(_) => return RKRDB_ERR,
    };
    match ConCorpus::open_readonly(path) {
        Ok(corpus) => {
            let id = push_handle(Handle {
                corpus: Arc::new(corpus),
                last_keys: Vec::new(),
                last_error: String::new(),
            });
            unsafe { *out_id = id };
            RKRDB_OK
        }
        Err(_) => RKRDB_ERR,
    }
}

/// Pack one frame as RCSO bytes for `MPI_Bcast` on the *caller* communicator.
/// `buf == NULL` returns the required size. Rank 0 of that comm calls this;
/// other ranks never open the env.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_pack_frame(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    buf: *mut u8,
    buflen: usize,
) -> c_int {
    let key = FrameKey { traj_id, frame_idx };
    with_handle(id, |h| match h.corpus.pack_frame(key) {
        Ok(bytes) => {
            if buf.is_null() {
                return Ok(bytes.len() as c_int);
            }
            if bytes.len() > buflen {
                set_err(h, "buffer too small");
                return Ok(RKRDB_ERR);
            }
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
            }
            Ok(bytes.len() as c_int)
        }
        Err(e) => {
            set_err(h, e);
            Ok(RKRDB_ERR)
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Pack many frames as one RCSB envelope. `buf == NULL` returns the required size.
/// Rank 0 of the caller comm packs; others never open the env.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_pack_frames(
    id: usize,
    traj_ids: *const u64,
    frame_idxs: *const u32,
    nkeys: u32,
    buf: *mut u8,
    buflen: usize,
) -> c_int {
    if traj_ids.is_null() || frame_idxs.is_null() || nkeys == 0 {
        return RKRDB_NULL;
    }
    let n = nkeys as usize;
    let trajs = unsafe { std::slice::from_raw_parts(traj_ids, n) };
    let frames = unsafe { std::slice::from_raw_parts(frame_idxs, n) };
    let keys: Vec<FrameKey> = trajs
        .iter()
        .zip(frames.iter())
        .map(|(&traj_id, &frame_idx)| FrameKey { traj_id, frame_idx })
        .collect();
    with_handle(id, |h| match h.corpus.pack_frames(&keys) {
        Ok(bytes) => {
            if buf.is_null() {
                return Ok(bytes.len() as c_int);
            }
            if bytes.len() > buflen {
                set_err(h, "buffer too small");
                return Ok(RKRDB_ERR);
            }
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
            }
            Ok(bytes.len() as c_int)
        }
        Err(e) => {
            set_err(h, e);
            Ok(RKRDB_ERR)
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Number of RCSO blobs in an RCSB envelope. No corpus handle.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_unpack_batch_nframes(
    buf: *const u8,
    buflen: usize,
    out_n: *mut u32,
) -> c_int {
    if buf.is_null() || out_n.is_null() {
        return RKRDB_NULL;
    }
    let slice = unsafe { std::slice::from_raw_parts(buf, buflen) };
    match crate::cooked_soa::decode_batch(slice) {
        Ok(parts) => {
            unsafe { *out_n = parts.len() as u32 };
            RKRDB_OK
        }
        Err(_) => RKRDB_ERR,
    }
}

/// Unpack one RCSO item from an RCSB envelope into xyz. No corpus handle.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_unpack_batch_item(
    buf: *const u8,
    buflen: usize,
    index: u32,
    out_xyz: *mut f64,
    capacity_atoms: u32,
    out_natoms: *mut u32,
) -> c_int {
    if buf.is_null() || out_xyz.is_null() || out_natoms.is_null() {
        return RKRDB_NULL;
    }
    let slice = unsafe { std::slice::from_raw_parts(buf, buflen) };
    let parts = match crate::cooked_soa::decode_batch(slice) {
        Ok(p) => p,
        Err(_) => return RKRDB_ERR,
    };
    let item = match parts.get(index as usize) {
        Some(b) => b,
        None => return RKRDB_NOT_FOUND,
    };
    rkrdb_unpack_positions(
        item.as_ptr(),
        item.len(),
        out_xyz,
        capacity_atoms,
        out_natoms,
    )
}

/// Unpack RCSO positions (no corpus handle — MPI worker side).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_unpack_positions(
    buf: *const u8,
    buflen: usize,
    out_xyz: *mut f64,
    capacity_atoms: u32,
    out_natoms: *mut u32,
) -> c_int {
    if buf.is_null() || out_xyz.is_null() || out_natoms.is_null() {
        return RKRDB_NULL;
    }
    let slice = unsafe { std::slice::from_raw_parts(buf, buflen) };
    let cooked = match crate::cooked_soa::CookedSoa::decode(slice) {
        Ok(c) => c,
        Err(_) => return RKRDB_ERR,
    };
    if cooked.natoms > capacity_atoms {
        return RKRDB_ERR;
    }
    unsafe { *out_natoms = cooked.natoms };
    let n = cooked.natoms as usize;
    let dest = unsafe { std::slice::from_raw_parts_mut(out_xyz, n.saturating_mul(3)) };
    for (i, p) in cooked.positions.iter().enumerate() {
        dest[i * 3] = p[0];
        dest[i * 3 + 1] = p[1];
        dest[i * 3 + 2] = p[2];
    }
    RKRDB_OK
}

#[no_mangle]
pub unsafe extern "C" fn rkrdb_close(id: usize) -> c_int {
    let mut g = HANDLES.lock().unwrap();
    if let Some(slot) = g.get_mut(id) {
        *slot = None;
        RKRDB_OK
    } else {
        RKRDB_NULL
    }
}

/// Last error message (thread-safe snapshot into caller buffer). Returns bytes written excluding NUL,
/// or -1 if truncated / null.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_last_error(id: usize, buf: *mut c_char, buflen: usize) -> c_int {
    if buf.is_null() || buflen == 0 {
        return RKRDB_NULL;
    }
    with_handle(id, |h| {
        let bytes = h.last_error.as_bytes();
        let n = (buflen - 1).min(bytes.len());
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, n);
            *buf.add(n) = 0;
        }
        Ok(n as c_int)
    })
    .unwrap_or(RKRDB_NULL)
}

fn parse_units_json(p: *const c_char) -> Result<Option<serde_json::Value>, c_int> {
    if p.is_null() {
        return Ok(None);
    }
    let s = unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|_| RKRDB_ERR)?;
    if s.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(s).map(Some).map_err(|_| RKRDB_ERR)
}

/// `units_json` is optional JSON (`{"length":"A","energy":"ev"}`). NULL stamps nothing.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_append_trajectory_units(
    id: usize,
    traj_id: u64,
    path: *const c_char,
    units_json: *const c_char,
    out_n_frames: *mut u32,
) -> c_int {
    if path.is_null() {
        return RKRDB_NULL;
    }
    let cpath = unsafe { CStr::from_ptr(path) };
    let path = match cpath.to_str() {
        Ok(s) => s,
        Err(_) => return RKRDB_ERR,
    };
    let units = match parse_units_json(units_json) {
        Ok(u) => u,
        Err(c) => return c,
    };
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.append_trajectory_path_units(traj_id, path, units) {
        Ok(n) => {
            if !out_n_frames.is_null() {
                unsafe { *out_n_frames = n };
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rkrdb_append_trajectory(
    id: usize,
    traj_id: u64,
    path: *const c_char,
    out_n_frames: *mut u32,
) -> c_int {
    rkrdb_append_trajectory_units(id, traj_id, path, ptr::null(), out_n_frames)
}

/// Create the trajectory or append frames after the live count.
/// `units_json` is optional; NULL stamps nothing on the new frames.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_extend_trajectory_units(
    id: usize,
    traj_id: u64,
    path: *const c_char,
    units_json: *const c_char,
    out_n_frames: *mut u32,
) -> c_int {
    if path.is_null() {
        return RKRDB_NULL;
    }
    let cpath = unsafe { CStr::from_ptr(path) };
    let path = match cpath.to_str() {
        Ok(s) => s,
        Err(_) => return RKRDB_ERR,
    };
    let units = match parse_units_json(units_json) {
        Ok(u) => u,
        Err(c) => return c,
    };
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.extend_trajectory_path_units(traj_id, path, units) {
        Ok(n) => {
            if !out_n_frames.is_null() {
                unsafe { *out_n_frames = n };
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rkrdb_extend_trajectory(
    id: usize,
    traj_id: u64,
    path: *const c_char,
    out_n_frames: *mut u32,
) -> c_int {
    rkrdb_extend_trajectory_units(id, traj_id, path, ptr::null(), out_n_frames)
}

/// Convert stored numbers and rewrite CON `units` for every frame of `traj_id`.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_set_units(
    id: usize,
    traj_id: u64,
    units_json: *const c_char,
    out_n_frames: *mut u32,
) -> c_int {
    if units_json.is_null() {
        return RKRDB_NULL;
    }
    let units = match parse_units_json(units_json) {
        Ok(Some(u)) => u,
        Ok(None) => return RKRDB_ERR,
        Err(c) => return c,
    };
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.set_trajectory_units(traj_id, units) {
        Ok(n) => {
            if !out_n_frames.is_null() {
                unsafe { *out_n_frames = n };
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

/// Write the frame `units` object as JSON into `buf` (NUL-terminated).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_frame_units(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    buf: *mut c_char,
    buflen: usize,
) -> c_int {
    if buf.is_null() || buflen == 0 {
        return RKRDB_NULL;
    }
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.frame_units(FrameKey { traj_id, frame_idx }) {
        Ok(Some(v)) => {
            let s = v.to_string();
            if s.len() + 1 > buflen {
                set_err_id(id, "frame_units buffer too small");
                return RKRDB_ERR;
            }
            unsafe {
                ptr::copy_nonoverlapping(s.as_ptr(), buf as *mut u8, s.len());
                *buf.add(s.len()) = 0;
            }
            RKRDB_OK
        }
        Ok(None) => RKRDB_NOT_FOUND,
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

/// Select by required symbol (optional) and natoms range (use 0, UINT32_MAX for any).
/// Results stored internally; use rkrdb_result_count / rkrdb_result_key.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_select_basic(
    id: usize,
    traj_id: i64,
    symbol: *const c_char,
    natoms_min: u32,
    natoms_max: u32,
    limit: u32,
) -> c_int {
    with_handle(id, |h| {
        let mut sel = Select::new().natoms_range(natoms_min, natoms_max);
        if traj_id >= 0 {
            sel = sel.trajectory(traj_id as u64);
        }
        if !symbol.is_null() {
            let s = unsafe { CStr::from_ptr(symbol) };
            if let Ok(sym) = s.to_str() {
                if !sym.is_empty() {
                    sel = sel.require_symbol(sym);
                }
            }
        }
        if limit > 0 {
            sel = sel.limit(limit as usize);
        }
        match h.corpus.select(&sel) {
            Ok(keys) => {
                h.last_keys = keys;
                Ok(RKRDB_OK)
            }
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Select by exact xxHash3-128 (16 bytes LE).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_select_hash(id: usize, hash16: *const u8) -> c_int {
    if hash16.is_null() {
        return RKRDB_NULL;
    }
    let mut hb = [0u8; 16];
    unsafe { ptr::copy_nonoverlapping(hash16, hb.as_mut_ptr(), 16) };
    with_handle(id, |h| {
        let sel = Select::new().exact_hash(hb);
        match h.corpus.select(&sel) {
            Ok(keys) => {
                h.last_keys = keys;
                Ok(RKRDB_OK)
            }
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Metadata / section filters. Pass `use_energy_range=0` to ignore energy bounds.
/// Flags: bit0=require_forces, bit1=require_velocities, bit2=require_energy.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_select_meta(
    id: usize,
    traj_id: i64,
    symbol: *const c_char,
    natoms_min: u32,
    natoms_max: u32,
    energy_min: f64,
    energy_max: f64,
    use_energy_range: c_int,
    flags: u32,
    limit: u32,
) -> c_int {
    with_handle(id, |h| {
        let mut sel = Select::new().natoms_range(natoms_min, natoms_max);
        if traj_id >= 0 {
            sel = sel.trajectory(traj_id as u64);
        }
        if !symbol.is_null() {
            let s = unsafe { CStr::from_ptr(symbol) };
            if let Ok(sym) = s.to_str() {
                if !sym.is_empty() {
                    sel = sel.require_symbol(sym);
                }
            }
        }
        if use_energy_range != 0 {
            sel = sel.energy_range(energy_min, energy_max);
        }
        if flags & 1 != 0 {
            sel = sel.require_forces();
        }
        if flags & 2 != 0 {
            sel = sel.require_velocities();
        }
        if flags & 4 != 0 {
            sel = sel.require_energy();
        }
        if limit > 0 {
            sel = sel.limit(limit as usize);
        }
        match h.corpus.select(&sel) {
            Ok(keys) => {
                h.last_keys = keys;
                Ok(RKRDB_OK)
            }
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Rebuild secondary indexes from authoritative frame blobs.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_reindex(id: usize) -> c_int {
    with_handle(id, |h| match h.corpus.reindex() {
        Ok(_) => Ok(RKRDB_OK),
        Err(e) => {
            set_err(h, e);
            Ok(RKRDB_ERR)
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Opt-in cook: derive RCSO into `frames_soa` from CON text in `frames` (CON remains authority).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_cook_frame(id: usize, traj_id: u64, frame_idx: u32) -> c_int {
    with_handle(id, |h| {
        match h
            .corpus
            .cook_frame(crate::keys::FrameKey { traj_id, frame_idx })
        {
            Ok(_) => Ok(RKRDB_OK),
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Drop cooked tier only; CON text and indexes unchanged.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_delete_cooked(id: usize, traj_id: u64, frame_idx: u32) -> c_int {
    with_handle(id, |h| {
        match h
            .corpus
            .delete_cooked_soa(crate::keys::FrameKey { traj_id, frame_idx })
        {
            Ok(()) => Ok(RKRDB_OK),
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Returns 1 if valid RCSO present, 0 if missing/corrupt, negative on error.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_has_valid_cooked(id: usize, traj_id: u64, frame_idx: u32) -> c_int {
    with_handle(id, |h| {
        match h
            .corpus
            .has_valid_cooked_soa(crate::keys::FrameKey { traj_id, frame_idx })
        {
            Ok(true) => Ok(1),
            Ok(false) => Ok(0),
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Prefer cooked positions (no CON parse on hit); else parse CON.
/// Writes `*out_natoms * 3` doubles into `out_xyz` (row-major N×3). `capacity_atoms` is max N.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_get_positions(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    out_xyz: *mut f64,
    capacity_atoms: u32,
    out_natoms: *mut u32,
) -> c_int {
    if out_xyz.is_null() || out_natoms.is_null() {
        return RKRDB_NULL;
    }
    with_handle(id, |h| {
        match h
            .corpus
            .get_positions(crate::keys::FrameKey { traj_id, frame_idx })
        {
            Ok(pos) => {
                let n = pos.len() as u32;
                if n > capacity_atoms {
                    set_err(
                        h,
                        crate::error::Error::Message("positions buffer too small".into()),
                    );
                    return Ok(RKRDB_ERR);
                }
                unsafe {
                    *out_natoms = n;
                    for (i, row) in pos.iter().enumerate() {
                        *out_xyz.add(i * 3) = row[0];
                        *out_xyz.add(i * 3 + 1) = row[1];
                        *out_xyz.add(i * 3 + 2) = row[2];
                    }
                }
                Ok(RKRDB_OK)
            }
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Prefer cooked forces when present; writes N×3 doubles. Sets *out_has_forces 0/1.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_get_forces(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    out_xyz: *mut f64,
    capacity_atoms: u32,
    out_natoms: *mut u32,
    out_has_forces: *mut u8,
) -> c_int {
    if out_xyz.is_null() || out_natoms.is_null() || out_has_forces.is_null() {
        return RKRDB_NULL;
    }
    with_handle(id, |h| {
        match h
            .corpus
            .get_forces(crate::keys::FrameKey { traj_id, frame_idx })
        {
            Ok(None) => {
                unsafe {
                    *out_has_forces = 0;
                    *out_natoms = 0;
                }
                Ok(RKRDB_OK)
            }
            Ok(Some(frc)) => {
                let n = frc.len() as u32;
                if n > capacity_atoms {
                    set_err(
                        h,
                        crate::error::Error::Message("forces buffer too small".into()),
                    );
                    return Ok(RKRDB_ERR);
                }
                unsafe {
                    *out_has_forces = 1;
                    *out_natoms = n;
                    for (i, row) in frc.iter().enumerate() {
                        *out_xyz.add(i * 3) = row[0];
                        *out_xyz.add(i * 3 + 1) = row[1];
                        *out_xyz.add(i * 3 + 2) = row[2];
                    }
                }
                Ok(RKRDB_OK)
            }
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Cook every frame that has CON text (`recook_all`).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_recook_all(id: usize) -> c_int {
    with_handle(id, |h| match h.corpus.recook_all() {
        Ok(_) => Ok(RKRDB_OK),
        Err(e) => {
            set_err(h, e);
            Ok(RKRDB_ERR)
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Canonical composition formula for a stored frame (same as core `index_proj`).
/// Writes into `buf` (NUL-terminated). Returns RKRDB_OK, RKRDB_NOT_FOUND, RKRDB_ERR, or buffer size need as positive?
/// On success returns RKRDB_OK; if buflen too small returns RKRDB_ERR and sets last_error.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_frame_formula(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    buf: *mut c_char,
    buflen: usize,
) -> c_int {
    if buf.is_null() || buflen == 0 {
        return RKRDB_NULL;
    }
    with_handle(id, |h| {
        match h
            .corpus
            .frame_formula(crate::keys::FrameKey { traj_id, frame_idx })
        {
            Ok(s) => {
                let bytes = s.as_bytes();
                if bytes.len() + 1 > buflen {
                    set_err(h, crate::error::Error::Message("buffer too small".into()));
                    return Ok(RKRDB_ERR);
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
                    *buf.add(bytes.len()) = 0;
                }
                Ok(RKRDB_OK)
            }
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Campaign select: composition formula (NUL-terminated, may be null), optional fmax window.
/// `use_fmax_range` non-zero applies fmax_min/max. Flags: bit0 forces, bit1 velocities, bit2 energy.
/// Element constraints: pass `elem_sym` + `elem_count` + `elem_exact` (1=exact, 0=min) for one pair (null skips).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_select_campaign(
    id: usize,
    traj_id: i64,
    symbol: *const c_char,
    natoms_min: u32,
    natoms_max: u32,
    formula: *const c_char,
    energy_min: f64,
    energy_max: f64,
    use_energy_range: c_int,
    fmax_min: f64,
    fmax_max: f64,
    use_fmax_range: c_int,
    elem_sym: *const c_char,
    elem_count: u32,
    elem_exact: c_int,
    flags: u32,
    limit: u32,
) -> c_int {
    with_handle(id, |h| {
        let mut sel = Select::new().natoms_range(natoms_min, natoms_max);
        if traj_id >= 0 {
            sel = sel.trajectory(traj_id as u64);
        }
        if !symbol.is_null() {
            let s = unsafe { CStr::from_ptr(symbol) };
            if let Ok(sym) = s.to_str() {
                if !sym.is_empty() {
                    sel = sel.require_symbol(sym);
                }
            }
        }
        if !formula.is_null() {
            let s = unsafe { CStr::from_ptr(formula) };
            if let Ok(f) = s.to_str() {
                if !f.is_empty() {
                    sel = sel.exact_composition(f);
                }
            }
        }
        if use_energy_range != 0 {
            sel = sel.energy_range(energy_min, energy_max);
        }
        if use_fmax_range != 0 {
            sel = sel.fmax_range(fmax_min, fmax_max);
        }
        if !elem_sym.is_null() {
            let s = unsafe { CStr::from_ptr(elem_sym) };
            if let Ok(sym) = s.to_str() {
                if !sym.is_empty() {
                    if elem_exact != 0 {
                        sel = sel.element_exact(sym, elem_count);
                    } else {
                        sel = sel.element_min(sym, elem_count);
                    }
                }
            }
        }
        if flags & 1 != 0 {
            sel = sel.require_forces();
        }
        if flags & 2 != 0 {
            sel = sel.require_velocities();
        }
        if flags & 4 != 0 {
            sel = sel.require_energy();
        }
        if limit > 0 {
            sel = sel.limit(limit as usize);
        }
        match h.corpus.select(&sel) {
            Ok(keys) => {
                h.last_keys = keys;
                Ok(RKRDB_OK)
            }
            Err(e) => {
                set_err(h, e);
                Ok(RKRDB_ERR)
            }
        }
    })
    .unwrap_or(RKRDB_NULL)
}

#[no_mangle]
pub unsafe extern "C" fn rkrdb_result_count(id: usize) -> c_int {
    with_handle(id, |h| Ok(h.last_keys.len() as c_int)).unwrap_or(RKRDB_NULL)
}

/// Write traj_id and frame_idx for result index `i` (0-based).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_result_key(
    id: usize,
    i: usize,
    out_traj: *mut u64,
    out_frame: *mut u32,
) -> c_int {
    if out_traj.is_null() || out_frame.is_null() {
        return RKRDB_NULL;
    }
    with_handle(id, |h| {
        let k = match h.last_keys.get(i) {
            Some(k) => *k,
            None => return Ok(RKRDB_NOT_FOUND),
        };
        unsafe {
            *out_traj = k.traj_id;
            *out_frame = k.frame_idx;
        }
        Ok(RKRDB_OK)
    })
    .unwrap_or(RKRDB_NULL)
}

/// Hash frame blob at key; writes 16 LE bytes to out_hash16.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_frame_hash(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    out_hash16: *mut u8,
) -> c_int {
    if out_hash16.is_null() {
        return RKRDB_NULL;
    }
    let key = FrameKey { traj_id, frame_idx };
    with_handle(id, |h| match h.corpus.frame_hash(key) {
        Ok(hash) => {
            let b = hash.to_bytes();
            unsafe { ptr::copy_nonoverlapping(b.as_ptr(), out_hash16, 16) };
            Ok(RKRDB_OK)
        }
        Err(e) => {
            set_err(h, e);
            Ok(RKRDB_ERR)
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Copy frame CON text into buf (NUL-terminated). Returns length excluding NUL, or error code.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_get_frame_text(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    buf: *mut c_char,
    buflen: usize,
) -> c_int {
    if buf.is_null() || buflen == 0 {
        return RKRDB_NULL;
    }
    let key = FrameKey { traj_id, frame_idx };
    with_handle(id, |h| match h.corpus.get_frame_text(key) {
        Ok(text) => {
            let bytes = text.as_bytes();
            if bytes.len() + 1 > buflen {
                set_err(h, "buffer too small");
                return Ok(RKRDB_ERR);
            }
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
                *buf.add(bytes.len()) = 0;
            }
            Ok(bytes.len() as c_int)
        }
        Err(e) => {
            set_err(h, e);
            Ok(RKRDB_ERR)
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Parse stored CON text into a readcon-core `RKRConFrame`.
/// Caller frees with `free_rkr_frame` from libreadcon_core. NULL on error.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_get_frame(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
) -> *mut std::ffi::c_void {
    let key = FrameKey { traj_id, frame_idx };
    with_handle(id, |h| match h.corpus.get_frame(key) {
        Ok(frame) => Ok(Box::into_raw(Box::new(frame)) as *mut std::ffi::c_void),
        Err(e) => {
            set_err(h, e);
            Ok(std::ptr::null_mut())
        }
    })
    .unwrap_or(std::ptr::null_mut())
}

/// `nframes` and `natoms` for one trajectory's H5MD arrays.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_h5md_shape(
    id: usize,
    traj_id: u64,
    out_nframes: *mut u32,
    out_natoms: *mut u32,
) -> c_int {
    if out_nframes.is_null() || out_natoms.is_null() {
        return RKRDB_NULL;
    }
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.collect_h5md(traj_id) {
        Ok(a) => {
            unsafe {
                *out_nframes = a.n_frames as u32;
                *out_natoms = a.natoms as u32;
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

/// Row-major `[T][N][3]` dest-Å positions from `collect_h5md`.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_h5md_positions(
    id: usize,
    traj_id: u64,
    out: *mut f64,
    cap: usize,
    out_nframes: *mut u32,
    out_natoms: *mut u32,
) -> c_int {
    if out.is_null() || out_nframes.is_null() || out_natoms.is_null() {
        return RKRDB_NULL;
    }
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.collect_h5md(traj_id) {
        Ok(a) => {
            if a.positions.len() > cap {
                set_err_id(id, "h5md_positions buffer too small");
                return RKRDB_ERR;
            }
            unsafe {
                ptr::copy_nonoverlapping(a.positions.as_ptr(), out, a.positions.len());
                *out_nframes = a.n_frames as u32;
                *out_natoms = a.natoms as u32;
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

unsafe fn h5md_copy_slice(
    id: usize,
    traj_id: u64,
    out: *mut f64,
    cap: usize,
    pick: fn(&crate::export_h5md::H5mdArrays) -> Option<&[f64]>,
    missing: &str,
) -> c_int {
    if out.is_null() {
        return RKRDB_NULL;
    }
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.collect_h5md(traj_id) {
        Ok(a) => {
            let Some(sl) = pick(&a) else {
                set_err_id(id, missing);
                return RKRDB_NOT_FOUND;
            };
            if sl.len() > cap {
                set_err_id(id, "h5md buffer too small");
                return RKRDB_ERR;
            }
            unsafe {
                ptr::copy_nonoverlapping(sl.as_ptr(), out, sl.len());
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rkrdb_h5md_edges(
    id: usize,
    traj_id: u64,
    out: *mut f64,
    cap: usize,
) -> c_int {
    h5md_copy_slice(id, traj_id, out, cap, |a| Some(a.edges.as_slice()), "")
}

#[no_mangle]
pub unsafe extern "C" fn rkrdb_h5md_forces(
    id: usize,
    traj_id: u64,
    out: *mut f64,
    cap: usize,
) -> c_int {
    h5md_copy_slice(id, traj_id, out, cap, |a| a.forces.as_deref(), "no forces")
}

#[no_mangle]
pub unsafe extern "C" fn rkrdb_h5md_velocities(
    id: usize,
    traj_id: u64,
    out: *mut f64,
    cap: usize,
) -> c_int {
    h5md_copy_slice(
        id,
        traj_id,
        out,
        cap,
        |a| a.velocities.as_deref(),
        "no velocities",
    )
}

/// Integer Z, length `N` (`collect_h5md` species_z).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_h5md_species(
    id: usize,
    traj_id: u64,
    out: *mut i32,
    cap: usize,
    out_natoms: *mut u32,
) -> c_int {
    if out.is_null() || out_natoms.is_null() {
        return RKRDB_NULL;
    }
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.collect_h5md(traj_id) {
        Ok(a) => {
            if a.species_z.len() > cap {
                set_err_id(id, "h5md_species buffer too small");
                return RKRDB_ERR;
            }
            unsafe {
                ptr::copy_nonoverlapping(a.species_z.as_ptr(), out, a.species_z.len());
                *out_natoms = a.species_z.len() as u32;
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rkrdb_get_velocities(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    out_xyz: *mut f64,
    capacity_atoms: u32,
    out_natoms: *mut u32,
    out_has_velocities: *mut u8,
) -> c_int {
    if out_xyz.is_null() || out_natoms.is_null() || out_has_velocities.is_null() {
        return RKRDB_NULL;
    }
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.get_velocities(FrameKey { traj_id, frame_idx }) {
        Ok(Some(rows)) => {
            if rows.len() > capacity_atoms as usize {
                set_err_id(id, "velocities buffer too small");
                return RKRDB_ERR;
            }
            unsafe {
                *out_has_velocities = 1;
                *out_natoms = rows.len() as u32;
                for (i, r) in rows.iter().enumerate() {
                    *out_xyz.add(i * 3) = r[0];
                    *out_xyz.add(i * 3 + 1) = r[1];
                    *out_xyz.add(i * 3 + 2) = r[2];
                }
            }
            RKRDB_OK
        }
        Ok(None) => {
            unsafe {
                *out_has_velocities = 0;
                *out_natoms = 0;
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

/// Dest-`ps` times from `collect_h5md` (CON time or `i * timestep`, else index).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_h5md_times(
    id: usize,
    traj_id: u64,
    out: *mut f64,
    cap: usize,
    out_n: *mut u32,
) -> c_int {
    if out.is_null() || out_n.is_null() {
        return RKRDB_NULL;
    }
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.collect_h5md(traj_id) {
        Ok(a) => {
            if a.times.len() > cap {
                set_err_id(id, "h5md_times buffer too small");
                return RKRDB_ERR;
            }
            unsafe {
                ptr::copy_nonoverlapping(a.times.as_ptr(), out, a.times.len());
                *out_n = a.times.len() as u32;
            }
            RKRDB_OK
        }
        Err(e) => {
            set_err_id(id, e);
            RKRDB_ERR
        }
    }
}

/// xxHash3-128 of arbitrary bytes (LE 16 bytes) — for clients hashing off-line blobs.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_xxh3_128(data: *const u8, len: usize, out_hash16: *mut u8) -> c_int {
    if data.is_null() || out_hash16.is_null() {
        return RKRDB_NULL;
    }
    let slice = unsafe { std::slice::from_raw_parts(data, len) };
    let h = hash_frame_bytes(slice);
    let b = h.to_bytes();
    unsafe { ptr::copy_nonoverlapping(b.as_ptr(), out_hash16, 16) };
    RKRDB_OK
}

// silence unused CString in some builds
#[allow(dead_code)]
fn _cs(s: &str) -> Result<CString, c_int> {
    CString::new(s).map_err(|_| RKRDB_ERR)
}

// ContentHash used in find path
#[allow(dead_code)]
fn _ch(b: [u8; 16]) -> ContentHash {
    ContentHash(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test")
            .join(name)
    }

    #[test]
    fn c_abi_append_units_canonical() {
        let dir = tempfile::tempdir().unwrap();
        let path = CString::new(dir.path().to_str().unwrap()).unwrap();
        let con = CString::new(fixture("tiny_cuh2.con").to_str().unwrap()).unwrap();
        let units = CString::new(r#"{"length":"A","energy":"ev","time":"femtosecond"}"#).unwrap();
        let mut id = 0usize;
        let mut n = 0u32;
        unsafe {
            assert_eq!(rkrdb_open(path.as_ptr(), &mut id), RKRDB_OK);
            assert_eq!(
                rkrdb_append_trajectory_units(id, 1, con.as_ptr(), units.as_ptr(), &mut n),
                RKRDB_OK
            );
            assert!(n >= 1);
            let mut buf = vec![0i8; 256];
            assert_eq!(
                rkrdb_frame_units(id, 1, 0, buf.as_mut_ptr(), buf.len()),
                RKRDB_OK
            );
            let s = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
            assert!(s.contains("angstrom"), "{s}");
            assert!(s.contains("eV"), "{s}");
            assert!(s.contains("fs"), "{s}");
            assert!(!s.contains("\"A\""), "{s}");
            let mut times = [0.0f64; 8];
            let mut nt = 0u32;
            assert_eq!(
                rkrdb_h5md_times(id, 1, times.as_mut_ptr(), times.len(), &mut nt),
                RKRDB_OK
            );
            assert!(nt >= 1);
            assert!(
                (times[0] - 0.0).abs() < 1e-12,
                "undeclared header.time dest ps times[0]={}",
                times[0]
            );
            assert_eq!(rkrdb_recook_all(id), RKRDB_OK);
            let need1 = rkrdb_pack_frame(id, 1, 0, std::ptr::null_mut(), 0);
            assert!(need1 > 0, "pack_frame size={need1}");
            let mut one = vec![0u8; need1 as usize];
            let n1 = rkrdb_pack_frame(id, 1, 0, one.as_mut_ptr(), one.len());
            assert_eq!(n1, need1);
            let mut xyz1 = vec![0.0f64; 32];
            let mut nxyz1 = 0u32;
            assert_eq!(
                rkrdb_unpack_positions(one.as_ptr(), n1 as usize, xyz1.as_mut_ptr(), 8, &mut nxyz1),
                RKRDB_OK
            );
            assert!(nxyz1 >= 1);
            assert!(
                (xyz1[0] - 0.6394).abs() < 1e-3,
                "pack_frame dest x0={}",
                xyz1[0]
            );
            let multi = CString::new(fixture("tiny_multi_cuh2.con").to_str().unwrap()).unwrap();
            let mut n2 = 0u32;
            assert_eq!(
                rkrdb_append_trajectory(id, 3, multi.as_ptr(), &mut n2),
                RKRDB_OK
            );
            let tids = [3u64, 3];
            let fids = [0u32, 1];
            let need =
                rkrdb_pack_frames(id, tids.as_ptr(), fids.as_ptr(), 2, std::ptr::null_mut(), 0);
            assert!(need > 0, "pack_frames size={need}");
            let mut pbuf = vec![0u8; need as usize];
            let npack = rkrdb_pack_frames(
                id,
                tids.as_ptr(),
                fids.as_ptr(),
                2,
                pbuf.as_mut_ptr(),
                pbuf.len(),
            );
            assert_eq!(npack, need);
            let mut nfr = 0u32;
            assert_eq!(
                rkrdb_unpack_batch_nframes(pbuf.as_ptr(), npack as usize, &mut nfr),
                RKRDB_OK
            );
            assert_eq!(nfr, 2);
            let mut item = vec![0.0f64; 32];
            let mut nitem = 0u32;
            assert_eq!(
                rkrdb_unpack_batch_item(
                    pbuf.as_ptr(),
                    npack as usize,
                    0,
                    item.as_mut_ptr(),
                    8,
                    &mut nitem
                ),
                RKRDB_OK
            );
            assert!(nitem >= 1);
            assert!((item[0] - 0.6394).abs() < 1e-3, "dest x0={}", item[0]);
            let mut item1 = vec![0.0f64; 32];
            let mut nitem1 = 0u32;
            assert_eq!(
                rkrdb_unpack_batch_item(
                    pbuf.as_ptr(),
                    npack as usize,
                    1,
                    item1.as_mut_ptr(),
                    8,
                    &mut nitem1
                ),
                RKRDB_OK
            );
            assert!(nitem1 >= 1);
            let mut native1 = vec![0.0f64; 32];
            let mut nn1 = 0u32;
            assert_eq!(
                rkrdb_get_positions(id, 3, 1, native1.as_mut_ptr(), 8, &mut nn1),
                RKRDB_OK
            );
            assert_eq!(nitem1, nn1);
            assert!(
                (item1[0] - native1[0]).abs() < 1e-9,
                "item1 x0={} native={}",
                item1[0],
                native1[0]
            );
            assert!((item1[6] - 8.8549).abs() < 1e-4, "frame1 H x={}", item1[6]);
            let mut times3 = [0.0f64; 8];
            let mut nt3 = 0u32;
            assert_eq!(
                rkrdb_h5md_times(id, 3, times3.as_mut_ptr(), times3.len(), &mut nt3),
                RKRDB_OK
            );
            assert!(nt3 >= 2);
            assert!((times3[0] - 0.0).abs() < 1e-12);
            assert!((times3[1] - 1.0).abs() < 1e-12);
            let mut nf = 0u32;
            let mut na = 0u32;
            assert_eq!(rkrdb_h5md_shape(id, 1, &mut nf, &mut na), RKRDB_OK);
            assert!(nf >= 1 && na >= 1);
            let mut pos = vec![0.0f64; (nf as usize) * (na as usize) * 3];
            assert_eq!(
                rkrdb_h5md_positions(id, 1, pos.as_mut_ptr(), pos.len(), &mut nf, &mut na),
                RKRDB_OK
            );
            assert_eq!(pos.len(), (nf as usize) * (na as usize) * 3);
            // stamped A → dest Å; first Cu x on tiny_cuh2.con
            assert!((pos[0] - 0.6394).abs() < 1e-4, "dest A x0={}", pos[0]);
            assert!(pos.iter().any(|&x| x != 0.0));
            let mut xyz = vec![0.0f64; 32];
            let mut npos = 0u32;
            assert_eq!(
                rkrdb_get_positions(id, 1, 0, xyz.as_mut_ptr(), 8, &mut npos),
                RKRDB_OK
            );
            assert!(npos >= 1);
            assert!((xyz[0] - 0.6394).abs() < 1e-4);
            let mut fr0 = vec![0.0f64; 32];
            let mut nf0 = 0u32;
            let mut hf0 = 1u8;
            assert_eq!(
                rkrdb_get_forces(id, 1, 0, fr0.as_mut_ptr(), 8, &mut nf0, &mut hf0),
                RKRDB_OK
            );
            assert_eq!(hf0, 0);
            let mut no_f = [0.0f64; 8];
            assert_eq!(
                rkrdb_h5md_forces(id, 1, no_f.as_mut_ptr(), no_f.len()),
                RKRDB_NOT_FOUND
            );
            let mut no_v = [0.0f64; 8];
            assert_eq!(
                rkrdb_h5md_velocities(id, 1, no_v.as_mut_ptr(), no_v.len()),
                RKRDB_NOT_FOUND
            );
            let mut edges = vec![0.0f64; (nf as usize) * 9];
            assert_eq!(
                rkrdb_h5md_edges(id, 1, edges.as_mut_ptr(), edges.len()),
                RKRDB_OK
            );
            assert!((edges[0] - 15.3456).abs() < 1e-4, "edge a_x={}", edges[0]);
            let mut z = vec![0i32; na as usize];
            let mut nz = 0u32;
            assert_eq!(
                rkrdb_h5md_species(id, 1, z.as_mut_ptr(), z.len(), &mut nz),
                RKRDB_OK
            );
            assert_eq!(nz, na);
            assert!(z.contains(&29), "{z:?}");
            assert!(z.contains(&1), "{z:?}");
            let forces = CString::new(fixture("tiny_cuh2_forces.con").to_str().unwrap()).unwrap();
            let ext_u = CString::new(r#"{"length":"A","energy":"ev"}"#).unwrap();
            let mut n2 = 0u32;
            assert_eq!(
                rkrdb_extend_trajectory_units(id, 1, forces.as_ptr(), ext_u.as_ptr(), &mut n2),
                RKRDB_OK
            );
            assert!(n2 >= 1);
            let mut buf2 = vec![0i8; 256];
            assert_eq!(
                rkrdb_frame_units(id, 1, 1, buf2.as_mut_ptr(), buf2.len()),
                RKRDB_OK
            );
            let s2 = CStr::from_ptr(buf2.as_ptr()).to_str().unwrap();
            assert!(s2.contains("angstrom"), "{s2}");
            let mut frc = vec![0.0f64; 256];
            assert_eq!(
                rkrdb_h5md_forces(id, 1, frc.as_mut_ptr(), frc.len()),
                RKRDB_OK
            );
            let dest_f0 = -1.234567 * 96.485_332;
            assert!(
                frc.iter().any(|&x| (x - dest_f0).abs() < 1e-3),
                "missing dest force ~{dest_f0}, got {:?}",
                &frc[..12]
            );
            let units2 = CString::new(r#"{"length":"nm","energy":"eV"}"#).unwrap();
            let mut nset = 0u32;
            assert_eq!(rkrdb_set_units(id, 1, units2.as_ptr(), &mut nset), RKRDB_OK);
            let mut nf2 = 0u32;
            let mut na2 = 0u32;
            assert_eq!(rkrdb_h5md_shape(id, 1, &mut nf2, &mut na2), RKRDB_OK);
            let mut pos2 = vec![0.0f64; (nf2 as usize) * (na2 as usize) * 3];
            assert_eq!(
                rkrdb_h5md_positions(id, 1, pos2.as_mut_ptr(), pos2.len(), &mut nf2, &mut na2),
                RKRDB_OK
            );
            assert!(
                (pos2[0] - 0.6394).abs() < 1e-4,
                "dest A after set_units x0={}",
                pos2[0]
            );
            rkrdb_close(id);
            let mut idro = 0usize;
            assert_eq!(rkrdb_open_readonly(path.as_ptr(), &mut idro), RKRDB_OK);
            let mut nfr = 0u32;
            let mut nar = 0u32;
            let mut posr = vec![0.0f64; 64];
            assert_eq!(
                rkrdb_h5md_positions(idro, 1, posr.as_mut_ptr(), posr.len(), &mut nfr, &mut nar),
                RKRDB_OK
            );
            assert!(
                (posr[0] - 0.6394).abs() < 1e-4,
                "readonly dest x0={}",
                posr[0]
            );
            assert_eq!(rkrdb_close(idro), RKRDB_OK);
            let velcon = CString::new(fixture("tiny_cuh2.convel").to_str().unwrap()).unwrap();
            let mut id2 = 0usize;
            let mut n3 = 0u32;
            assert_eq!(rkrdb_open(path.as_ptr(), &mut id2), RKRDB_OK);
            assert_eq!(
                rkrdb_append_trajectory(id2, 2, velcon.as_ptr(), &mut n3),
                RKRDB_OK
            );
            let mut vel = vec![0.0f64; 64];
            assert_eq!(
                rkrdb_h5md_velocities(id2, 2, vel.as_mut_ptr(), vel.len()),
                RKRDB_OK
            );
            assert!((vel[0] - 1.234).abs() < 1e-9, "got {}", vel[0]);
            let mut native = vec![0.0f64; 32];
            let mut nv = 0u32;
            let mut has_v = 0u8;
            assert_eq!(
                rkrdb_get_velocities(id2, 2, 0, native.as_mut_ptr(), 8, &mut nv, &mut has_v),
                RKRDB_OK
            );
            assert_eq!(has_v, 1);
            assert!(nv >= 1);
            assert!(
                (native[0] - 0.001234).abs() < 1e-9,
                "native vx0={}",
                native[0]
            );
            let mut tiny = [0.0f64; 1];
            let mut ntiny = 0u32;
            let mut htiny = 0u8;
            assert_eq!(
                rkrdb_get_velocities(id2, 2, 0, tiny.as_mut_ptr(), 0, &mut ntiny, &mut htiny),
                RKRDB_ERR
            );
            let mut ebuf = vec![0i8; 128];
            assert!(rkrdb_last_error(id2, ebuf.as_mut_ptr(), ebuf.len()) >= 0);
            let es = CStr::from_ptr(ebuf.as_ptr()).to_str().unwrap();
            assert!(es.contains("too small"), "{es}");
            let mut none = vec![0.0f64; 8];
            let mut nn = 99u32;
            let mut has_none = 1u8;
            assert_eq!(
                rkrdb_get_velocities(id2, 1, 0, none.as_mut_ptr(), 2, &mut nn, &mut has_none),
                RKRDB_OK
            );
            assert_eq!(has_none, 0);
            assert_eq!(nn, 0);
            assert_eq!(rkrdb_close(id2), RKRDB_OK);
        }
    }
}
