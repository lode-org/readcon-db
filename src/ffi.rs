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

#[no_mangle]
pub unsafe extern "C" fn rkrdb_append_trajectory(
    id: usize,
    traj_id: u64,
    path: *const c_char,
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
    // Prepare+commit on Arc corpus **outside** handle mutex (concurrent writers on distinct handles).
    let corpus = match corpus_arc(id) {
        Ok(c) => c,
        Err(c) => return c,
    };
    match corpus.append_trajectory_path(traj_id, path) {
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
