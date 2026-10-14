//! Read-side MPI helpers over a caller-supplied communicator.
//!
//! - [`scatter_packed_frames`]: rank 0 opens the corpus read-only, packs
//!   each rank's block of the selection as an RCSO batch and sends it with
//!   `MPI_Scatterv`. Other ranks never open LMDB.
//! - [`export_extxyz_collective`]: every rank opens the corpus read-only
//!   (shared file system), formats its block of the selection, and the
//!   blocks are written at `MPI_Exscan` offsets with
//!   `MPI_File_write_at_all`, or gathered to rank 0 without MPI-IO. The
//!   file bytes equal `ConCorpus::export_extxyz` for every rank count.
//!
//! The selection (`keys`) is read on rank 0 and broadcast, so other ranks
//! may pass an empty slice. Blocks follow [`crate::mpi_ingest::block_range`].

use std::path::Path;

use mpi::collective::SystemOperation;
use mpi::datatype::{Partition, PartitionMut};
use mpi::ffi;
use mpi::traits::*;

use crate::corpus::ConCorpus;
use crate::error::{Error, Result};
use crate::export_xyz::write_frame_extxyz;
use crate::keys::FrameKey;
use crate::mpi_ingest::{block_range, collective_pub as collective};

fn bcast_keys<C: Communicator>(comm: &C, keys: &[FrameKey]) -> Vec<FrameKey> {
    let root = comm.process_at_rank(0);
    let mut n = keys.len() as u64;
    root.broadcast_into(&mut n);
    let mut buf: Vec<u8> = if comm.rank() == 0 {
        keys.iter().flat_map(|k| k.to_bytes()).collect()
    } else {
        vec![0u8; n as usize * 12]
    };
    if n > 0 {
        root.broadcast_into(&mut buf[..]);
    }
    buf.chunks_exact(12)
        .map(|c| FrameKey {
            traj_id: u64::from_be_bytes(c[..8].try_into().unwrap()),
            frame_idx: u32::from_be_bytes(c[8..].try_into().unwrap()),
        })
        .collect()
}

fn prefix(c: &[i32]) -> Vec<i32> {
    c.iter()
        .scan(0, |a, &x| {
            let d = *a;
            *a += x;
            Some(d)
        })
        .collect()
}

/// This rank's block of `keys` (taken from rank 0) as one RCSO batch, plus
/// the global index range of that block.
pub fn scatter_packed_frames<C: Communicator>(
    comm: &C,
    corpus_dir: impl AsRef<Path>,
    keys: &[FrameKey],
) -> Result<(std::ops::Range<usize>, Vec<u8>)> {
    let keys = bcast_keys(comm, keys);
    let (rank, size) = (comm.rank() as usize, comm.size() as usize);
    let n = keys.len();
    let packed = if rank == 0 {
        (|| -> Result<Vec<Vec<u8>>> {
            let c = ConCorpus::open_readonly(corpus_dir.as_ref())?;
            (0..size)
                .map(|r| c.pack_frames(&keys[block_range(n, size, r)]))
                .collect()
        })()
    } else {
        Ok(Vec::new())
    };
    let packed = collective(comm, packed)?;
    let root = comm.process_at_rank(0);
    let mut len = 0i32;
    let mut mine;
    if rank == 0 {
        let counts: Vec<i32> = packed.iter().map(|b| b.len() as i32).collect();
        root.scatter_into_root(&counts[..], &mut len);
        let all: Vec<u8> = packed.concat();
        mine = vec![0u8; len as usize + 1];
        mine.truncate(len as usize);
        let d = prefix(&counts);
        root.scatter_varcount_into_root(
            &Partition::new(&all[..], &counts[..], &d[..]),
            &mut mine[..],
        );
    } else {
        root.scatter_into(&mut len);
        mine = vec![0u8; len as usize + 1];
        mine.truncate(len as usize);
        root.scatter_varcount_into(&mut mine[..]);
    }
    Ok((block_range(n, size, rank), mine))
}

/// How [`export_extxyz_collective`] writes the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// `MPI_File_write_at_all` at `MPI_Exscan` offsets.
    MpiIo,
    /// Gather every block to rank 0, which writes the file.
    Gather,
}

fn rc(r: i32, what: &str) -> Result<()> {
    if r == 0 {
        Ok(())
    } else {
        Err(Error::Message(format!("{what} returned {r}")))
    }
}

/// Collective extended-XYZ export of `keys` (taken from rank 0) to `path`.
/// Refuses an existing `path` like the serial export. Returns frames written.
pub fn export_extxyz_collective<C: Communicator>(
    comm: &C,
    corpus_dir: impl AsRef<Path>,
    keys: &[FrameKey],
    path: impl AsRef<Path>,
    energy_key: &str,
    mode: WriteMode,
) -> Result<usize> {
    let path = path.as_ref();
    let keys = bcast_keys(comm, keys);
    let (rank, size) = (comm.rank() as usize, comm.size() as usize);
    let exists = if rank == 0 && path.exists() {
        Err(Error::Message(format!(
            "export_extxyz dest exists: {}",
            path.display()
        )))
    } else {
        Ok(())
    };
    collective(comm, exists)?;
    let bytes = (|| -> Result<Vec<u8>> {
        let c = ConCorpus::open_readonly(corpus_dir.as_ref())?;
        let mut b = Vec::with_capacity(1);
        for k in &keys[block_range(keys.len(), size, rank)] {
            write_frame_extxyz(&mut b, &c.get_frame(*k)?, energy_key)?;
        }
        Ok(b)
    })();
    let bytes = collective(comm, bytes)?;
    let len = bytes.len() as u64;
    let mut off = 0u64;
    comm.exclusive_scan_into(&len, &mut off, SystemOperation::sum());
    if rank == 0 {
        off = 0;
    }
    let mut total = 0u64;
    comm.all_reduce_into(&len, &mut total, SystemOperation::sum());
    match mode {
        WriteMode::MpiIo => write_mpiio(comm, path, &bytes, off, total)?,
        WriteMode::Gather => write_gather(comm, path, &bytes)?,
    }
    Ok(keys.len())
}

fn write_gather<C: Communicator>(comm: &C, path: &Path, bytes: &[u8]) -> Result<()> {
    let root = comm.process_at_rank(0);
    let len = bytes.len() as i32;
    if comm.rank() == 0 {
        let mut lens = vec![0i32; comm.size() as usize];
        root.gather_into_root(&len, &mut lens[..]);
        let tot: usize = lens.iter().map(|&x| x as usize).sum();
        let mut all = vec![0u8; tot + 1];
        all.truncate(tot);
        let d = prefix(&lens);
        root.gather_varcount_into_root(
            bytes,
            &mut PartitionMut::new(&mut all[..], &lens[..], &d[..]),
        );
        let w = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, &all))
            .map_err(Error::from);
        collective(comm, w)
    } else {
        root.gather_into(&len);
        root.gather_varcount_into(bytes);
        collective(comm, Ok(()))
    }
}

/// Bytes per `MPI_File_write_at_all` call (stays below the i32 count limit).
const CHUNK: usize = 1 << 30;

fn write_mpiio<C: Communicator>(
    comm: &C,
    path: &Path,
    bytes: &[u8],
    off: u64,
    total: u64,
) -> Result<()> {
    let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|e| Error::Message(e.to_string()));
    let cpath = collective(comm, cpath)?;
    let mut fh: ffi::MPI_File = unsafe { ffi::RSMPI_FILE_NULL };
    let amode = (ffi::MPI_MODE_CREATE | ffi::MPI_MODE_WRONLY | ffi::MPI_MODE_EXCL) as i32;
    let r = unsafe {
        ffi::MPI_File_open(
            comm.as_raw(),
            cpath.as_ptr(),
            amode,
            ffi::RSMPI_INFO_NULL,
            &mut fh,
        )
    };
    collective(comm, rc(r, "MPI_File_open"))?;
    let res = (|| -> Result<()> {
        let r = unsafe { ffi::MPI_File_set_size(fh, total as ffi::MPI_Offset) };
        collective(comm, rc(r, "MPI_File_set_size"))?;
        let mine = bytes.len().div_ceil(CHUNK) as u64;
        let mut rounds = 0u64;
        comm.all_reduce_into(&mine, &mut rounds, SystemOperation::max());
        let keep = [0u8; 1];
        let mut res = Ok(());
        for k in 0..rounds as usize {
            let lo = (k * CHUNK).min(bytes.len());
            let hi = ((k + 1) * CHUNK).min(bytes.len());
            // Empty rounds still pass a live pointer.
            let ptr = if hi > lo {
                bytes[lo..].as_ptr()
            } else {
                keep.as_ptr()
            };
            let r = unsafe {
                ffi::MPI_File_write_at_all(
                    fh,
                    (off + lo as u64) as ffi::MPI_Offset,
                    ptr as *const std::ffi::c_void,
                    (hi - lo) as i32,
                    ffi::RSMPI_UINT8_T,
                    ffi::RSMPI_STATUS_IGNORE,
                )
            };
            if res.is_ok() {
                res = rc(r, "MPI_File_write_at_all");
            }
        }
        collective(comm, res)
    })();
    let r = unsafe { ffi::MPI_File_close(&mut fh) };
    res?;
    collective(comm, rc(r, "MPI_File_close"))
}
