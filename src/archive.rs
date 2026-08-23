//! Asynchronous observation archive over a [`ConCorpus`].
//!
//! Active-learning consumers (GP surrogates pruning their training
//! sets) need a durable record of every configuration an expensive
//! oracle evaluated: the active set is a view, the archive is the
//! ledger. This module owns that contract once, behind a C ABI, so
//! every consumer shares one implementation instead of re-deriving
//! frame serialization, threading, and read-back ordering.
//!
//! Writes follow the async-logger shape: [`ObservationArchive::append`]
//! copies the row into a channel and returns; a dedicated writer
//! thread drains the channel and commits each row as a single-frame
//! trajectory built natively with [`ConFrameBuilder`] (no temporary
//! files, no serialization on the caller thread). Disk-side failures
//! are counted, never raised into the caller. Readers synchronize with
//! [`ObservationArchive::flush`]; LMDB's MVCC makes the read snapshot
//! safe against the concurrent writer.
//!
//! Atom ordering: CON groups atoms by type, so stored frames are a
//! permutation of the caller's order. [`ObservationArchive::fetch`]
//! inverts that permutation through the stored `atom_id`s and returns
//! rows in the caller's original order — the property a training-set
//! consumer actually needs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use readcon_core::helpers::atomic_number_to_symbol;
use readcon_core::types::ConFrameBuilder;

use crate::corpus::ConCorpus;
use crate::error::{Error, Result};
use crate::keys::FrameKey;

struct Row {
    positions: Vec<f64>,
    forces: Vec<f64>,
    energy: f64,
}

enum Msg {
    Row(Row),
    Flush(Sender<()>),
}

/// Fixed-composition archive of oracle evaluations.
pub struct ObservationArchive {
    corpus: Arc<ConCorpus>,
    tx: Option<Sender<Msg>>,
    worker: Option<JoinHandle<()>>,
    committed: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    /// Rows accepted by append (existing + enqueued), ahead of
    /// `committed` while the writer drains. Readers key refreshes on
    /// this so an in-flight row still triggers a flush + refetch.
    appended: AtomicU64,
    natoms: usize,
    dir: PathBuf,
}

impl ObservationArchive {
    /// Open (or create) the archive at `dir` for one fixed composition.
    /// `z` is the per-atom atomic-number list in caller order; `cell`
    /// the orthorhombic box lengths. The corpus lives at
    /// `dir/observations.rdb`; trajectory ids continue across restarts.
    pub fn open(dir: impl AsRef<Path>, z: Vec<u32>, cell: [f64; 3]) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let corpus = Arc::new(ConCorpus::open(dir.join("observations.rdb"))?);
        let existing = corpus.list_frame_keys()?.len() as u64;
        let committed = Arc::new(AtomicU64::new(existing));
        let dropped = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel::<Msg>();
        let natoms = z.len();

        let worker_corpus = Arc::clone(&corpus);
        let worker_committed = Arc::clone(&committed);
        let worker_dropped = Arc::clone(&dropped);
        let worker = std::thread::Builder::new()
            .name("rkrdb-archive".into())
            .spawn(move || {
                while let Ok(msg) = rx.recv() {
                    match msg {
                        Msg::Row(row) => {
                            let traj = worker_committed.load(Ordering::Relaxed);
                            match commit_row(&worker_corpus, traj, &z, cell, &row) {
                                Ok(()) => {
                                    worker_committed.store(traj + 1, Ordering::Relaxed);
                                }
                                Err(_) => {
                                    worker_dropped.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        Msg::Flush(ack) => {
                            let _ = ack.send(());
                        }
                    }
                }
            })
            .map_err(|e| Error::Message(format!("archive worker spawn failed: {e}")))?;

        Ok(Self {
            corpus,
            tx: Some(tx),
            worker: Some(worker),
            committed,
            dropped,
            appended: AtomicU64::new(existing),
            natoms,
            dir,
        })
    }

    /// Enqueue one evaluation. `positions` and `forces` are flat
    /// `3 * natoms` buffers in caller order. Returns `false` only when
    /// the row cannot be enqueued (wrong length, archive closing);
    /// disk-side failures are counted in [`Self::dropped`] instead.
    pub fn append(&self, positions: &[f64], forces: &[f64], energy: f64) -> bool {
        if positions.len() != 3 * self.natoms || forces.len() != 3 * self.natoms {
            return false;
        }
        let Some(tx) = self.tx.as_ref() else {
            return false;
        };
        let sent = tx
            .send(Msg::Row(Row {
                positions: positions.to_vec(),
                forces: forces.to_vec(),
                energy,
            }))
            .is_ok();
        if sent {
            self.appended.fetch_add(1, Ordering::Relaxed);
        }
        sent
    }

    /// Block until every previously enqueued row is committed (or
    /// counted dropped). Call before reading the corpus back.
    pub fn flush(&self) {
        let Some(tx) = self.tx.as_ref() else { return };
        let (ack_tx, ack_rx) = mpsc::channel();
        if tx.send(Msg::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv();
        }
    }

    /// Rows committed to the corpus (including prior runs).
    pub fn committed(&self) -> u64 {
        self.committed.load(Ordering::Relaxed)
    }

    /// Rows accepted by append, including any still in the writer
    /// queue (and prior runs). `appended - dropped` equals `committed`
    /// once flushed.
    pub fn appended(&self) -> u64 {
        self.appended.load(Ordering::Relaxed)
    }

    /// Rows the writer could not persist.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Atoms per row.
    pub fn natoms(&self) -> usize {
        self.natoms
    }

    /// Archive directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Fetch committed row `index` in the caller's original atom
    /// order (the stored type-grouped permutation is inverted through
    /// the frame's `atom_id`s). Flush first for a complete snapshot.
    pub fn fetch(&self, index: u64) -> Result<(Vec<f64>, Vec<f64>, f64)> {
        let frame = self.corpus.get_frame(FrameKey {
            traj_id: index,
            frame_idx: 0,
        })?;
        let n = self.natoms;
        if frame.atom_ids.len() != n {
            return Err(Error::Message(format!(
                "archive row {index}: expected {n} atoms, found {}",
                frame.atom_ids.len()
            )));
        }
        if frame.positions.nrows() != n || frame.forces.nrows() != n {
            return Err(Error::Parse(format!(
                "archive row {index}: expected {n} position and force rows, \
                 found {} and {}",
                frame.positions.nrows(),
                frame.forces.nrows()
            )));
        }
        let mut positions = vec![0.0; 3 * n];
        let mut forces = vec![0.0; 3 * n];
        for (stored, &atom_id) in frame.atom_ids.iter().enumerate() {
            let orig = atom_id as usize;
            if orig >= n {
                return Err(Error::Parse(format!(
                    "archive row {index}: atom id {orig} out of range"
                )));
            }
            positions[3 * orig..3 * orig + 3]
                .copy_from_slice(&frame.positions.as_f64_row(stored));
            forces[3 * orig..3 * orig + 3]
                .copy_from_slice(&frame.forces.as_f64_row(stored));
        }
        let energy = frame
            .header
            .energy()
            .ok_or_else(|| Error::Message(format!("archive row {index} has no energy")))?;
        Ok((positions, forces, energy))
    }
}

impl Drop for ObservationArchive {
    fn drop(&mut self) {
        // Close the channel so the worker drains and exits, then join.
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn commit_row(
    corpus: &ConCorpus,
    traj: u64,
    z: &[u32],
    cell: [f64; 3],
    row: &Row,
) -> Result<()> {
    let mut builder = ConFrameBuilder::new(cell, [90.0, 90.0, 90.0]);
    builder.prebox_header("observation archive");
    builder.set_energy(row.energy);
    for (i, &zi) in z.iter().enumerate() {
        builder.add_atom(
            atomic_number_to_symbol(u64::from(zi)),
            row.positions[3 * i],
            row.positions[3 * i + 1],
            row.positions[3 * i + 2],
            [false; 3],
            i as u64,
            f64::from(zi),
        );
    }
    builder
        .set_forces_from_flat(&row.forces)
        .map_err(|e| Error::Message(format!("archive forces: {e}")))?;
    let frame = builder.build();
    // 17 significant digits: the ledger must round-trip f64 exactly.
    corpus.append_trajectory_frames_with_precision(traj, &[frame], "observation-archive", 17)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// C ABI
// ---------------------------------------------------------------------------

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

use crate::ffi::{RKRDB_ERR, RKRDB_NOT_FOUND, RKRDB_NULL, RKRDB_OK};

static ARCHIVES: Mutex<Vec<Option<Box<ObservationArchive>>>> = Mutex::new(Vec::new());

fn push_archive(a: ObservationArchive) -> usize {
    let mut g = ARCHIVES.lock().unwrap();
    for (i, slot) in g.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(Box::new(a));
            return i;
        }
    }
    g.push(Some(Box::new(a)));
    g.len() - 1
}

fn with_archive<F, T>(id: usize, f: F) -> std::result::Result<T, c_int>
where
    F: FnOnce(&ObservationArchive) -> std::result::Result<T, c_int>,
{
    let g = ARCHIVES.lock().unwrap();
    let slot = g.get(id).ok_or(RKRDB_NULL)?;
    let a = slot.as_ref().ok_or(RKRDB_NULL)?;
    f(a)
}

/// Open (or create) an observation archive at `dir` for a fixed
/// composition: `z` is the per-atom atomic-number list in caller
/// order (`natoms` entries), `cell3` the three orthorhombic box
/// lengths. Writes the opaque handle to `out_id`.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_archive_open(
    dir: *const c_char,
    z: *const u32,
    natoms: u32,
    cell3: *const f64,
    out_id: *mut usize,
) -> c_int {
    if dir.is_null() || z.is_null() || cell3.is_null() || out_id.is_null() || natoms == 0 {
        return RKRDB_NULL;
    }
    let cdir = unsafe { CStr::from_ptr(dir) };
    let Ok(dir) = cdir.to_str() else {
        return RKRDB_ERR;
    };
    let z = unsafe { std::slice::from_raw_parts(z, natoms as usize) }.to_vec();
    let cell = unsafe { [*cell3, *cell3.add(1), *cell3.add(2)] };
    match ObservationArchive::open(dir, z, cell) {
        Ok(archive) => {
            unsafe { *out_id = push_archive(archive) };
            RKRDB_OK
        }
        Err(_) => RKRDB_ERR,
    }
}

/// Enqueue one evaluation: flat `3 * natoms` position and force
/// buffers in caller order plus the total energy. Non-blocking; disk
/// failures are counted via `rkrdb_archive_dropped`.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_archive_append(
    id: usize,
    positions: *const f64,
    forces: *const f64,
    energy: f64,
) -> c_int {
    if positions.is_null() || forces.is_null() {
        return RKRDB_NULL;
    }
    with_archive(id, |a| {
        let n3 = 3 * a.natoms();
        let pos = unsafe { std::slice::from_raw_parts(positions, n3) };
        let frc = unsafe { std::slice::from_raw_parts(forces, n3) };
        Ok(if a.append(pos, frc, energy) {
            RKRDB_OK
        } else {
            RKRDB_ERR
        })
    })
    .unwrap_or(RKRDB_NULL)
}

/// Block until every enqueued row is committed or counted dropped.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_archive_flush(id: usize) -> c_int {
    with_archive(id, |a| {
        a.flush();
        Ok(RKRDB_OK)
    })
    .unwrap_or(RKRDB_NULL)
}

/// Rows committed to the corpus (including prior runs).
#[no_mangle]
pub unsafe extern "C" fn rkrdb_archive_count(id: usize, out_count: *mut u64) -> c_int {
    if out_count.is_null() {
        return RKRDB_NULL;
    }
    with_archive(id, |a| {
        unsafe { *out_count = a.committed() };
        Ok(RKRDB_OK)
    })
    .unwrap_or(RKRDB_NULL)
}

/// Rows accepted by append, including any still queued (prior runs
/// included). Key cache refreshes on this; fetch bounds on
/// rkrdb_archive_count after a flush.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_archive_appended(id: usize, out_count: *mut u64) -> c_int {
    if out_count.is_null() {
        return RKRDB_NULL;
    }
    with_archive(id, |a| {
        unsafe { *out_count = a.appended() };
        Ok(RKRDB_OK)
    })
    .unwrap_or(RKRDB_NULL)
}

/// Rows the writer thread could not persist.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_archive_dropped(id: usize, out_count: *mut u64) -> c_int {
    if out_count.is_null() {
        return RKRDB_NULL;
    }
    with_archive(id, |a| {
        unsafe { *out_count = a.dropped() };
        Ok(RKRDB_OK)
    })
    .unwrap_or(RKRDB_NULL)
}

/// Fetch committed row `index` in the caller's original atom order.
/// `positions` and `forces` receive `3 * natoms` doubles; `out_energy`
/// the total energy. Flush first for a complete snapshot.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_archive_fetch(
    id: usize,
    index: u64,
    positions: *mut f64,
    forces: *mut f64,
    capacity_atoms: u32,
    out_energy: *mut f64,
) -> c_int {
    if positions.is_null() || forces.is_null() || out_energy.is_null() {
        return RKRDB_NULL;
    }
    with_archive(id, |a| {
        if (capacity_atoms as usize) < a.natoms() {
            return Ok(RKRDB_ERR);
        }
        match a.fetch(index) {
            Ok((pos, frc, energy)) => {
                unsafe {
                    std::ptr::copy_nonoverlapping(pos.as_ptr(), positions, pos.len());
                    std::ptr::copy_nonoverlapping(frc.as_ptr(), forces, frc.len());
                    *out_energy = energy;
                }
                Ok(RKRDB_OK)
            }
            Err(Error::MissingFrame(_)) => Ok(RKRDB_NOT_FOUND),
            Err(_) => Ok(RKRDB_ERR),
        }
    })
    .unwrap_or(RKRDB_NULL)
}

/// Drain the writer, close the corpus, release the handle.
#[no_mangle]
pub unsafe extern "C" fn rkrdb_archive_close(id: usize) -> c_int {
    let mut g = ARCHIVES.lock().unwrap();
    match g.get_mut(id) {
        Some(slot) => {
            *slot = None;
            RKRDB_OK
        }
        None => RKRDB_NULL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_caller_order_and_energy() {
        let dir = tempfile::tempdir().unwrap();
        // Deliberately type-interleaved composition so the CON
        // type-grouping permutes storage order: H, O, H.
        let z = vec![1_u32, 8, 1];
        let cell = [25.0, 25.0, 25.0];
        let archive = ObservationArchive::open(dir.path(), z.clone(), cell).unwrap();

        let pos: Vec<f64> = (0..9).map(|i| i as f64 * 0.1).collect();
        let frc: Vec<f64> = (0..9).map(|i| -(i as f64) * 0.01).collect();
        assert!(archive.append(&pos, &frc, -76.4));
        assert!(archive.append(&frc, &pos, -75.9));
        // The refresh key sees in-flight rows before any flush.
        assert_eq!(archive.appended(), 2);
        archive.flush();
        assert_eq!(archive.appended(), 2);
        assert_eq!(archive.committed(), 2);
        assert_eq!(archive.dropped(), 0);

        let (p0, f0, e0) = archive.fetch(0).unwrap();
        assert_eq!(p0, pos);
        assert_eq!(f0, frc);
        assert!((e0 - (-76.4)).abs() < 1e-12);
        let (p1, _f1, e1) = archive.fetch(1).unwrap();
        assert_eq!(p1, frc);
        assert!((e1 - (-75.9)).abs() < 1e-12);
    }

    #[test]
    fn restart_continues_trajectory_ids() {
        let dir = tempfile::tempdir().unwrap();
        let z = vec![6_u32, 1];
        let cell = [10.0, 10.0, 10.0];
        let pos = vec![0.0; 6];
        let frc = vec![0.0; 6];
        {
            let archive = ObservationArchive::open(dir.path(), z.clone(), cell).unwrap();
            assert!(archive.append(&pos, &frc, 1.0));
            archive.flush();
            assert_eq!(archive.committed(), 1);
        }
        {
            let archive = ObservationArchive::open(dir.path(), z, cell).unwrap();
            assert_eq!(archive.committed(), 1);
            assert!(archive.append(&pos, &frc, 2.0));
            archive.flush();
            assert_eq!(archive.committed(), 2);
            let (_, _, e) = archive.fetch(1).unwrap();
            assert!((e - 2.0).abs() < 1e-12);
        }
    }

    #[test]
    fn append_rejects_wrong_length() {
        let dir = tempfile::tempdir().unwrap();
        let archive =
            ObservationArchive::open(dir.path(), vec![1, 1], [5.0, 5.0, 5.0]).unwrap();
        assert!(!archive.append(&[0.0; 3], &[0.0; 6], 0.0));
    }
}
