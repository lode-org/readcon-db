//! Parallel CON ingest over a caller-supplied MPI communicator.
//!
//! Every rank parses, hashes and derives index keys for a contiguous block
//! of frames. Only the 16-byte content hashes are exchanged first
//! (`MPI_Allgatherv`), so a frame whose bytes already occurred at a lower
//! global index sends no blob. Rank 0 is the single LMDB writer and inserts
//! frames in global order, in batches gathered with `MPI_Gatherv`, so keys,
//! hash winners and metadata equal the serial `append_trajectory_path`.
//!
//! The library never calls `MPI_Init` and only uses the communicator it is
//! given. Rank 0 must pass the open corpus; other ranks pass `None`.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use mpi::collective::SystemOperation;
use mpi::datatype::PartitionMut;
use mpi::traits::*;
use readcon_core::iterators::{frame_start_offsets, ConFrameIterator};

use crate::corpus::ConCorpus;
use crate::error::{Error, Result};
use crate::keys::{FrameKey, TrajId};

/// Frames per gather batch on the writer rank (bounds rank 0 memory).
pub const DEFAULT_BATCH_FRAMES: usize = 4096;

/// Contiguous block of `n` items for `rank` of `size`; the remainder goes to
/// low ranks. Same partition as `readcon_core::distrib::block_range`.
pub fn block_range(n: usize, size: usize, rank: usize) -> std::ops::Range<usize> {
    let base = n / size;
    let rem = n % size;
    let start = rank * base + rank.min(rem);
    start..start + base + usize::from(rank < rem)
}

fn agree<C: Communicator>(comm: &C, ok: bool) -> bool {
    let mut all = 0i32;
    comm.all_reduce_into(&i32::from(ok), &mut all, SystemOperation::min());
    all == 1
}

fn bcast_bytes<C: Communicator>(comm: &C, data: Vec<u8>) -> Vec<u8> {
    let root = comm.process_at_rank(0);
    let mut n = data.len() as u64;
    root.broadcast_into(&mut n);
    let mut buf = if comm.rank() == 0 {
        data
    } else {
        vec![0u8; n as usize]
    };
    if n > 0 {
        root.broadcast_into(&mut buf[..]);
    }
    buf
}

/// Rank 0 result (payload or message) broadcast to all ranks.
fn bcast_result<C: Communicator>(
    comm: &C,
    r: std::result::Result<Vec<u8>, String>,
) -> Result<Vec<u8>> {
    let msg = if comm.rank() == 0 {
        match r {
            Ok(p) => [&[1u8][..], &p].concat(),
            Err(e) => [&[0u8][..], e.as_bytes()].concat(),
        }
    } else {
        Vec::new()
    };
    let got = bcast_bytes(comm, msg);
    match got.split_first() {
        Some((1, rest)) => Ok(rest.to_vec()),
        Some((_, rest)) => Err(Error::Message(String::from_utf8_lossy(rest).into_owned())),
        None => Err(Error::Message("empty broadcast".into())),
    }
}

/// Fail on every rank if `r` failed on any rank.
fn collective<C: Communicator, T>(comm: &C, r: Result<T>) -> Result<T> {
    let ok = agree(comm, r.is_ok());
    match (r, ok) {
        (Ok(v), true) => Ok(v),
        (Err(e), _) => Err(Error::Message(format!("rank {}: {e}", comm.rank()))),
        (Ok(_), false) => Err(Error::Message("MPI ingest failed on another rank".into())),
    }
}

struct Local {
    first: usize,
    hashes: Vec<[u8; 16]>,
    enc: Vec<Vec<u8>>,
    blobs: Vec<String>,
}

fn take<'a>(k: usize, p: &mut &'a [u8]) -> &'a [u8] {
    let (a, b) = p.split_at(k);
    *p = b;
    a
}

fn parse_local(
    traj_id: TrajId,
    path: &Path,
    offs: &[u64],
    first: usize,
    end: usize,
) -> Result<Local> {
    let mut out = Local {
        first,
        hashes: vec![],
        enc: vec![],
        blobs: vec![],
    };
    if first == end {
        return Ok(out);
    }
    let (lo, hi) = (offs[first], offs[end]);
    let mut f = std::fs::File::open(path)?;
    f.seek(SeekFrom::Start(lo))?;
    let mut buf = vec![0u8; (hi - lo) as usize];
    f.read_exact(&mut buf)?;
    let text = std::str::from_utf8(&buf).map_err(|e| Error::Message(format!("utf8: {e}")))?;
    for g in first..end {
        let s = &text[(offs[g] - lo) as usize..(offs[g + 1] - lo) as usize];
        let (frame, blob) = match ConFrameIterator::new(s).next_with_raw_span(s) {
            Some(Ok(x)) => x,
            Some(Err(e)) => return Err(Error::Parse(e.to_string())),
            None => return Err(Error::Parse(format!("frame {g}: empty slice"))),
        };
        let fk = FrameKey {
            traj_id,
            frame_idx: g as u32,
        };
        let (h, enc) = ConCorpus::encode_frame_keys(fk, &frame, blob);
        out.hashes.push(h);
        out.enc.push(enc);
        out.blobs.push(blob.to_owned());
    }
    Ok(out)
}

/// Ingest the CON file at `path` (visible on every rank) as `traj_id`.
///
/// Collective over `comm`. Rank 0 passes `Some(corpus)` and is the only
/// LMDB writer. Returns the frame count on every rank.
pub fn append_trajectory_path<C: Communicator>(
    comm: &C,
    corpus: Option<&ConCorpus>,
    traj_id: TrajId,
    path: impl AsRef<Path>,
    batch_frames: usize,
) -> Result<u32> {
    let path = path.as_ref();
    let (rank, size) = (comm.rank() as usize, comm.size() as usize);
    let batch = batch_frames.max(1);

    // Rank 0: metadata guard plus frame offsets (u64, last = file length).
    let root = if rank == 0 {
        (|| -> std::result::Result<Vec<u8>, String> {
            let c = corpus.ok_or("rank 0 must pass the corpus")?;
            c.check_traj_free(traj_id).map_err(|e| e.to_string())?;
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let mut offs: Vec<u64> = frame_start_offsets(&text)
                .into_iter()
                .map(|o| o as u64)
                .collect();
            offs.push(text.len() as u64);
            Ok(offs.iter().flat_map(|x| x.to_le_bytes()).collect())
        })()
    } else {
        Ok(Vec::new())
    };
    let offs: Vec<u64> = bcast_result(comm, root)?
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect();
    let n = offs.len().saturating_sub(1);
    let mine = block_range(n, size, rank);

    let local = collective(
        comm,
        parse_local(traj_id, path, &offs, mine.start, mine.end),
    )?;

    // Hash-only exchange in rank order = global frame order.
    let mut my_h: Vec<u8> = Vec::with_capacity(1 + local.hashes.len() * 16);
    my_h.extend(local.hashes.iter().flatten());
    let counts: Vec<i32> = (0..size)
        .map(|r| (block_range(n, size, r).len() * 16) as i32)
        .collect();
    let displs: Vec<i32> = counts
        .iter()
        .scan(0, |a, &c| {
            let d = *a;
            *a += c;
            Some(d)
        })
        .collect();
    let mut all_h = vec![0u8; n * 16 + 1];
    all_h.truncate(n * 16);
    {
        let mut part = PartitionMut::new(&mut all_h[..], &counts[..], &displs[..]);
        comm.all_gather_varcount_into(&my_h[..], &mut part);
    }
    // first[g] = lowest global index with the same bytes.
    let mut seen: HashMap<&[u8], usize> = HashMap::with_capacity(n);
    let first_of: Vec<usize> = (0..n)
        .map(|g| *seen.entry(&all_h[g * 16..g * 16 + 16]).or_insert(g))
        .collect();
    let referenced: HashSet<usize> = (0..n)
        .filter(|&g| first_of[g] != g)
        .map(|g| first_of[g])
        .collect();

    let mut kept: HashMap<usize, String> = HashMap::new();
    let mut nb = 0usize;
    #[allow(clippy::needless_range_loop)]
    while nb * batch < n || (n == 0 && nb == 0) {
        let (b0, b1) = (nb * batch, ((nb + 1) * batch).min(n));
        nb += 1;
        // This rank's records for frames in [b0, b1).
        // A real allocation even when empty: Open MPI rejects dangling buffers.
        let mut msg = Vec::with_capacity(1);
        for g in mine.start.max(b0)..mine.end.min(b1) {
            let i = g - local.first;
            msg.extend_from_slice(&(local.enc[i].len() as u32).to_le_bytes());
            msg.extend_from_slice(&local.enc[i]);
            let blob: &[u8] = if first_of[g] == g {
                local.blobs[i].as_bytes()
            } else {
                &[]
            };
            msg.extend_from_slice(&(blob.len() as u32).to_le_bytes());
            msg.extend_from_slice(blob);
        }
        let len = msg.len() as i32;
        let rp = comm.process_at_rank(0);
        let mut ok = true;
        if rank == 0 {
            let mut lens = vec![0i32; size];
            rp.gather_into_root(&len, &mut lens[..]);
            let d: Vec<i32> = lens
                .iter()
                .scan(0, |a, &c| {
                    let x = *a;
                    *a += c;
                    Some(x)
                })
                .collect();
            let tot = lens.iter().map(|&x| x as usize).sum();
            let mut store = vec![0u8; tot + 1];
            store.truncate(tot);
            {
                let mut part = PartitionMut::new(&mut store[..], &lens[..], &d[..]);
                rp.gather_varcount_into_root(&msg[..], &mut part);
            }
            let mut rows = Vec::with_capacity(b1 - b0);
            let mut p = &store[..];
            for g in b0..b1 {
                let el = u32::from_le_bytes(take(4, &mut p).try_into().unwrap()) as usize;
                let enc = take(el, &mut p);
                let bl = u32::from_le_bytes(take(4, &mut p).try_into().unwrap()) as usize;
                let blob = if first_of[g] == g {
                    let s = String::from_utf8(take(bl, &mut p).to_vec()).unwrap_or_default();
                    if referenced.contains(&g) {
                        kept.insert(g, s.clone());
                    }
                    s
                } else {
                    kept.get(&first_of[g]).cloned().unwrap_or_default()
                };
                rows.push((
                    FrameKey {
                        traj_id,
                        frame_idx: g as u32,
                    },
                    enc,
                    blob,
                ));
            }
            let r = corpus.unwrap().put_encoded(rows);
            ok = r.is_ok();
            if let Err(e) = r {
                eprintln!("readcon-db mpi ingest: {e}");
            }
        } else {
            rp.gather_into(&len);
            rp.gather_varcount_into(&msg[..]);
        }
        if !agree(comm, ok) {
            return Err(Error::Message("LMDB insert failed on rank 0".into()));
        }
        if n == 0 {
            break;
        }
    }
    let meta = if rank == 0 {
        corpus
            .unwrap()
            .put_traj_meta(traj_id, n as u32, path.display().to_string())
    } else {
        Ok(())
    };
    collective(comm, meta)?;

    Ok(n as u32)
}
