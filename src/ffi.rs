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
/// Returns byte count, or error. Rank 0 of that comm calls this; other
/// ranks never open the env.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_pack_frame(
    id: usize,
    traj_id: u64,
    frame_idx: u32,
    buf: *mut u8,
    buflen: usize,
) -> c_int {
    if buf.is_null() || buflen == 0 {
        return RKRDB_NULL;
    }
    let key = FrameKey {
        traj_id,
        frame_idx,
    };
    with_handle(id, |h| match h.corpus.pack_frame(key) {
        Ok(bytes) => {
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
        .map(|(&traj_id, &frame_idx)| FrameKey {
            traj_id,
            frame_idx,
        })
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
    let s = unsafe { CStr::from_ptr(p) }.to_str().map_err(|_| RKRDB_ERR)?;
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
    match corpus.frame_units(FrameKey {
        traj_id,
        frame_idx,
    }) {
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
        match h.corpus.cook_frame(crate::keys::FrameKey {
            traj_id,
            frame_idx,
        }) {
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
        match h.corpus.delete_cooked_soa(crate::keys::FrameKey {
            traj_id,
            frame_idx,
        }) {
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
        match h.corpus.has_valid_cooked_soa(crate::keys::FrameKey {
            traj_id,
            frame_idx,
        }) {
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
        match h.corpus.get_positions(crate::keys::FrameKey {
            traj_id,
            frame_idx,
        }) {
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
        match h.corpus.get_forces(crate::keys::FrameKey {
            traj_id,
            frame_idx,
        }) {
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
        match h.corpus.frame_formula(crate::keys::FrameKey {
            traj_id,
            frame_idx,
        }) {
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
    let key = FrameKey {
        traj_id,
        frame_idx,
    };
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
    let key = FrameKey {
        traj_id,
        frame_idx,
    };
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
    let key = FrameKey {
        traj_id,
        frame_idx,
    };
    with_handle(id, |h| match h.corpus.get_frame(key) {
        Ok(frame) => Ok(Box::into_raw(Box::new(frame)) as *mut std::ffi::c_void),
        Err(e) => {
            set_err(h, e);
            Ok(std::ptr::null_mut())
        }
    })
    .unwrap_or(std::ptr::null_mut())
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
            assert_eq!(rkrdb_close(id), RKRDB_OK);
        }
    }
}
