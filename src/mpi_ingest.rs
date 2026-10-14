//! Parallel CON ingest over a caller-supplied MPI communicator.
//!
//! The frames of all input files, concatenated in argument order, form one
//! global sequence. Every rank parses, hashes and derives index keys for a
//! contiguous block of it. Only the 16-byte content hashes are exchanged
//! first (`MPI_Allgatherv`), so a frame whose bytes already occurred at a
//! lower global index in the same LMDB env sends no blob.
//!
//! Records then move to the rank that owns the destination env with
//! `MPI_Alltoallv`, in batches of global frames. The payload of a first
//! occurrence is the tagged zstd frame of the CON text (or the raw UTF-8
//! when that frame does not shrink). xxh3-128 was taken on the UTF-8.
//! A later duplicate of those bytes in the same env receives a copy.
//!
//! - single writer: rank 0 owns the one corpus;
//! - sharded: shard `s = traj_id % n_shards` is owned by rank `s % P`.
//!
//! Each owner receives records in source rank order, which is global frame
//! order, and inserts them in that order. Keys, hash winners and trajectory
//! metadata therefore equal the serial ingest of the same files in the same
//! order, for every rank count.
//!
//! The library never calls `MPI_Init` and only uses the communicator it is
//! given.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use mpi::collective::SystemOperation;
use mpi::datatype::PartitionMut;
use mpi::traits::*;
use readcon_core::iterators::{frame_start_offsets, ConFrameIterator};

use crate::corpus::ConCorpus;
use crate::error::{Error, Result};
use crate::keys::{FrameKey, TrajId};
use crate::shard::ShardedConCorpus;

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
pub(crate) fn collective_pub<C: Communicator, T>(comm: &C, r: Result<T>) -> Result<T> {
    collective(comm, r)
}

fn collective<C: Communicator, T>(comm: &C, r: Result<T>) -> Result<T> {
    let ok = agree(comm, r.is_ok());
    match (r, ok) {
        (Ok(v), true) => Ok(v),
        (Err(e), _) => Err(Error::Message(format!("rank {}: {e}", comm.rank()))),
        (Ok(_), false) => Err(Error::Message("MPI ingest failed on another rank".into())),
    }
}

/// Per-file frame table broadcast from rank 0.
struct FileTab {
    traj_id: TrajId,
    path: PathBuf,
    /// Frame start offsets plus the file length.
    offs: Vec<u64>,
    /// Global index of this file's first frame.
    g0: usize,
}

impl FileTab {
    fn n(&self) -> usize {
        self.offs.len() - 1
    }
}

fn scan_files<C: Communicator>(comm: &C, files: &[(TrajId, PathBuf)]) -> Result<Vec<FileTab>> {
    let root = if comm.rank() == 0 {
        (|| -> std::result::Result<Vec<u8>, String> {
            let mut out = Vec::new();
            for (_, p) in files {
                let text =
                    std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
                let offs = frame_start_offsets(&text);
                out.extend_from_slice(&(offs.len() as u64 + 1).to_le_bytes());
                for o in offs {
                    out.extend_from_slice(&(o as u64).to_le_bytes());
                }
                out.extend_from_slice(&(text.len() as u64).to_le_bytes());
            }
            Ok(out)
        })()
    } else {
        Ok(Vec::new())
    };
    let words: Vec<u64> = bcast_result(comm, root)?
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect();
    let mut tabs = Vec::with_capacity(files.len());
    let (mut i, mut g0) = (0usize, 0usize);
    for (t, p) in files {
        let k = words[i] as usize;
        let offs = words[i + 1..i + 1 + k].to_vec();
        i += 1 + k;
        let tab = FileTab {
            traj_id: *t,
            path: p.clone(),
            offs,
            g0,
        };
        g0 += tab.n();
        tabs.push(tab);
    }
    Ok(tabs)
}

struct Local {
    first: usize,
    hashes: Vec<[u8; 16]>,
    enc: Vec<Vec<u8>>,
    /// Tagged zstd bytes, or raw UTF-8 when the tagged frame does not shrink.
    stored: Vec<Vec<u8>>,
}

fn take<'a>(k: usize, p: &mut &'a [u8]) -> &'a [u8] {
    let (a, b) = p.split_at(k);
    *p = b;
    a
}

fn file_of(tabs: &[FileTab], g: usize) -> usize {
    tabs.partition_point(|t| t.g0 + t.n() <= g)
}

fn parse_local(tabs: &[FileTab], range: std::ops::Range<usize>) -> Result<Local> {
    let mut out = Local {
        first: range.start,
        hashes: vec![],
        enc: vec![],
        stored: vec![],
    };
    let mut g = range.start;
    while g < range.end {
        let t = &tabs[file_of(tabs, g)];
        let (a, b) = (g - t.g0, (range.end - t.g0).min(t.n()));
        let (lo, hi) = (t.offs[a], t.offs[b]);
        let mut f = std::fs::File::open(&t.path)?;
        f.seek(SeekFrom::Start(lo))?;
        let mut buf = vec![0u8; (hi - lo) as usize];
        f.read_exact(&mut buf)?;
        let text = std::str::from_utf8(&buf).map_err(|e| Error::Message(format!("utf8: {e}")))?;
        for i in a..b {
            let s = &text[(t.offs[i] - lo) as usize..(t.offs[i + 1] - lo) as usize];
            let (frame, blob) = match ConFrameIterator::new(s).next_with_raw_span(s) {
                Some(Ok(x)) => x,
                Some(Err(e)) => return Err(Error::Parse(e.to_string())),
                None => {
                    return Err(Error::Parse(format!(
                        "{}: frame {i} is empty",
                        t.path.display()
                    )))
                }
            };
            let fk = FrameKey {
                traj_id: t.traj_id,
                frame_idx: i as u32,
            };
            let (h, enc, stored) = ConCorpus::encode_frame_keys(fk, &frame, blob)?;
            out.hashes.push(h);
            out.enc.push(enc);
            out.stored.push(stored);
        }
        g = t.g0 + b;
    }
    Ok(out)
}

fn prefix(counts: &[i32]) -> Vec<i32> {
    counts
        .iter()
        .scan(0, |a, &c| {
            let d = *a;
            *a += c;
            Some(d)
        })
        .collect()
}

/// Map headroom per input byte reserved before an MPI ingest.
const INPUT_MAP_FACTOR: usize = 3;

/// Where a trajectory's frames are written: (owner rank, env id).
type Route<'a> = &'a dyn Fn(TrajId) -> (usize, u32);

fn engine<C: Communicator>(
    comm: &C,
    files: &[(TrajId, PathBuf)],
    route: Route<'_>,
    sink: &mut dyn FnMut(u32) -> Result<ConCorpus>,
    batch: usize,
) -> Result<Vec<u32>> {
    let (rank, size) = (comm.rank() as usize, comm.size() as usize);
    let batch = batch.max(1);
    let mut seen_t = HashSet::new();
    if !files.iter().all(|(t, _)| seen_t.insert(*t)) {
        return Err(Error::Message("duplicate traj_id in MPI ingest".into()));
    }
    let tabs = scan_files(comm, files)?;
    let n: usize = tabs.iter().map(FileTab::n).sum();

    // Owners open their envs and check that no trajectory exists yet.
    let mut envs: HashMap<u32, ConCorpus> = HashMap::new();
    let opened = (|| -> Result<()> {
        for t in &tabs {
            let (owner, env) = route(t.traj_id);
            if owner == rank {
                if let std::collections::hash_map::Entry::Vacant(v) = envs.entry(env) {
                    v.insert(sink(env)?);
                }
                envs[&env].check_traj_free(t.traj_id)?;
            }
        }
        Ok(())
    })();
    collective(comm, opened)?;

    // Size each writer's map from the input so large ingests cannot hit
    // MDB_MAP_FULL: frame text, index keys and B-tree overhead stay below
    // INPUT_MAP_FACTOR times the CON bytes routed to that env.
    let mut env_bytes: HashMap<u32, usize> = HashMap::new();
    for t in &tabs {
        let (owner, env) = route(t.traj_id);
        if owner == rank {
            *env_bytes.entry(env).or_default() += *t.offs.last().unwrap_or(&0) as usize;
        }
    }
    let grown = env_bytes.iter().try_for_each(|(env, b)| {
        envs[env]
            .reserve_map(b.saturating_mul(INPUT_MAP_FACTOR))
            .map(|_| ())
    });
    collective(comm, grown)?;

    let mine = block_range(n, size, rank);
    let local = collective(comm, parse_local(&tabs, mine.clone()))?;

    // Hash-only exchange in rank order = global frame order.
    let mut my_h: Vec<u8> = Vec::with_capacity(1 + local.hashes.len() * 16);
    my_h.extend(local.hashes.iter().flatten());
    let counts: Vec<i32> = (0..size)
        .map(|r| (block_range(n, size, r).len() * 16) as i32)
        .collect();
    let displs = prefix(&counts);
    let mut all_h = vec![0u8; n * 16 + 1];
    all_h.truncate(n * 16);
    {
        let mut part = PartitionMut::new(&mut all_h[..], &counts[..], &displs[..]);
        comm.all_gather_varcount_into(&my_h[..], &mut part);
    }
    // env_of[g] and first_of[g] = lowest global index with equal bytes in the same env.
    let mut env_of = Vec::with_capacity(n);
    for t in &tabs {
        let e = route(t.traj_id);
        env_of.extend(std::iter::repeat_n(e, t.n()));
    }
    let mut seen: HashMap<(u32, &[u8]), usize> = HashMap::with_capacity(n);
    let first_of: Vec<usize> = (0..n)
        .map(|g| {
            *seen
                .entry((env_of[g].1, &all_h[g * 16..g * 16 + 16]))
                .or_insert(g)
        })
        .collect();
    let referenced: HashSet<usize> = (0..n)
        .filter(|&g| first_of[g] != g)
        .map(|g| first_of[g])
        .collect();

    let mut kept: HashMap<usize, Vec<u8>> = HashMap::new();
    let mut b0 = 0usize;
    while b0 < n {
        let b1 = (b0 + batch).min(n);
        // One message per destination rank, records in global order.
        let mut out: Vec<Vec<u8>> = vec![Vec::new(); size];
        for g in mine.start.max(b0)..mine.end.min(b1) {
            let i = g - local.first;
            let m = &mut out[env_of[g].0];
            m.extend_from_slice(&(g as u64).to_le_bytes());
            m.extend_from_slice(&(local.enc[i].len() as u32).to_le_bytes());
            m.extend_from_slice(&local.enc[i]);
            let stored: &[u8] = if first_of[g] == g {
                &local.stored[i]
            } else {
                &[]
            };
            m.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            m.extend_from_slice(stored);
        }
        let scounts: Vec<i32> = out.iter().map(|m| m.len() as i32).collect();
        let mut rcounts = vec![0i32; size];
        comm.all_to_all_into(&scounts[..], &mut rcounts[..]);
        let mut send: Vec<u8> =
            Vec::with_capacity(1 + scounts.iter().map(|&c| c as usize).sum::<usize>());
        for m in &out {
            send.extend_from_slice(m);
        }
        let rtot: usize = rcounts.iter().map(|&c| c as usize).sum();
        let mut recv = vec![0u8; rtot + 1];
        recv.truncate(rtot);
        {
            let sd = prefix(&scounts);
            let rd = prefix(&rcounts);
            let sp = mpi::datatype::Partition::new(&send[..], &scounts[..], &sd[..]);
            let mut rp = PartitionMut::new(&mut recv[..], &rcounts[..], &rd[..]);
            comm.all_to_all_varcount_into(&sp, &mut rp);
        }
        // Decode in source order (= global order) and insert per env.
        let mut rows: Vec<(u32, FrameKey, &[u8], Vec<u8>)> = Vec::new();
        let mut p = &recv[..];
        while !p.is_empty() {
            let g = u64::from_le_bytes(take(8, &mut p).try_into().unwrap()) as usize;
            let el = u32::from_le_bytes(take(4, &mut p).try_into().unwrap()) as usize;
            let enc = take(el, &mut p);
            let bl = u32::from_le_bytes(take(4, &mut p).try_into().unwrap()) as usize;
            let raw = take(bl, &mut p);
            let stored = if first_of[g] == g {
                let s = raw.to_vec();
                if referenced.contains(&g) {
                    kept.insert(g, s.clone());
                }
                s
            } else {
                kept.get(&first_of[g]).cloned().unwrap_or_default()
            };
            let t = &tabs[file_of(&tabs, g)];
            rows.push((
                env_of[g].1,
                FrameKey {
                    traj_id: t.traj_id,
                    frame_idx: (g - t.g0) as u32,
                },
                enc,
                stored,
            ));
        }
        let mut ids: Vec<u32> = rows.iter().map(|r| r.0).collect();
        ids.dedup();
        ids.sort_unstable();
        ids.dedup();
        let put = ids.iter().try_for_each(|id| {
            let sel: Vec<(FrameKey, &[u8], Vec<u8>)> = rows
                .iter()
                .filter(|r| r.0 == *id)
                .map(|r| (r.1, r.2, r.3.clone()))
                .collect();
            envs[id].put_encoded(sel)
        });
        collective(comm, put)?;
        b0 = b1;
    }
    let meta = tabs.iter().try_for_each(|t| {
        let (owner, env) = route(t.traj_id);
        if owner == rank {
            envs[&env].put_traj_meta(t.traj_id, t.n() as u32, t.path.display().to_string())
        } else {
            Ok(())
        }
    });
    collective(comm, meta)?;
    Ok(tabs.iter().map(|t| t.n() as u32).collect())
}

/// Ingest the CON file at `path` (visible on every rank) as `traj_id`.
///
/// Collective over `comm`. Rank 0 passes the corpus directory and is the
/// only LMDB writer; other ranks may pass any value. Returns the frame count.
pub fn append_trajectory_path<C: Communicator>(
    comm: &C,
    corpus_dir: impl AsRef<Path>,
    traj_id: TrajId,
    path: impl AsRef<Path>,
    batch_frames: usize,
) -> Result<u32> {
    let files = [(traj_id, path.as_ref().to_path_buf())];
    Ok(append_trajectory_paths(comm, corpus_dir, &files, batch_frames)?[0])
}

/// Single-writer ingest of several files, in order, into the corpus at
/// `corpus_dir` (opened on rank 0 only). Returns frame counts per file.
pub fn append_trajectory_paths<C: Communicator>(
    comm: &C,
    corpus_dir: impl AsRef<Path>,
    files: &[(TrajId, PathBuf)],
    batch_frames: usize,
) -> Result<Vec<u32>> {
    let dir = corpus_dir.as_ref().to_path_buf();
    engine(
        comm,
        files,
        &|_| (0, 0),
        &mut |_| ConCorpus::open(&dir),
        batch_frames,
    )
}

/// Sharded multi-writer ingest: shard `traj_id % n_shards` is written by
/// rank `shard % P` under `root`. Equals `ShardedConCorpus::append_trajectory_path`
/// called for each file in order. Returns frame counts per file.
pub fn append_trajectory_paths_sharded<C: Communicator>(
    comm: &C,
    root: impl AsRef<Path>,
    n_shards: u32,
    files: &[(TrajId, PathBuf)],
    batch_frames: usize,
) -> Result<Vec<u32>> {
    let root = root.as_ref().to_path_buf();
    // Rank 0 creates or reads the manifest; everyone uses its shard count.
    let r = if comm.rank() == 0 {
        ShardedConCorpus::open(&root, n_shards)
            .map(|s| s.n_shards().to_le_bytes().to_vec())
            .map_err(|e| e.to_string())
    } else {
        Ok(Vec::new())
    };
    let ns = u32::from_le_bytes(bcast_result(comm, r)?[..4].try_into().unwrap());
    let size = comm.size() as usize;
    let route = move |t: TrajId| {
        let s = ShardedConCorpus::shard_for_traj(t, ns);
        (s as usize % size, s)
    };
    engine(
        comm,
        files,
        &route,
        &mut |s| ShardedConCorpus::open_shard(&root, s),
        batch_frames,
    )
}

/// C ABI: MPI ingest over a Fortran `MPI_Fint` communicator handle.
///
/// `n_shards == 0` writes one corpus at `dir` from rank 0; otherwise `dir`
/// is a sharded root. `traj_ids` and `paths` hold `n_files` entries in
/// ingest order. On success every rank writes the frame count of each file
/// to `out_counts` (if non-null) and returns 0. Failures are collective.
/// The header `readcon-db-mpi.h` wraps this for `MPI_Comm`.
///
/// # Safety
/// MPI must be initialized, `comm_f` must name a live intra-communicator,
/// and the pointers must be valid for `n_files` entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rkrdb_mpi_ingest_paths_f(
    comm_f: mpi::ffi::RSMPI_Fint,
    dir: *const std::ffi::c_char,
    n_shards: u32,
    n_files: usize,
    traj_ids: *const u64,
    paths: *const *const std::ffi::c_char,
    batch_frames: usize,
    out_counts: *mut u32,
) -> i32 {
    use mpi::raw::FromRaw;
    use mpi::topology::SimpleCommunicator;
    if dir.is_null() || (n_files > 0 && (traj_ids.is_null() || paths.is_null())) {
        return -1;
    }
    let mut dup: mpi::ffi::MPI_Comm = unsafe { std::mem::zeroed() };
    if unsafe { mpi::ffi::MPI_Comm_dup(mpi::ffi::RSMPI_Comm_f2c(comm_f), &mut dup) } != 0 {
        return -2;
    }
    // Library traffic stays on a private duplicate, freed on drop.
    let comm = unsafe { SimpleCommunicator::from_raw(dup) };
    let s = |p: *const std::ffi::c_char| {
        unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned()
    };
    let dir = s(dir);
    let files: Vec<(TrajId, PathBuf)> = (0..n_files)
        .map(|i| unsafe { (*traj_ids.add(i), PathBuf::from(s(*paths.add(i)))) })
        .collect();
    let r = if n_shards == 0 {
        append_trajectory_paths(&comm, &dir, &files, batch_frames)
    } else {
        append_trajectory_paths_sharded(&comm, &dir, n_shards, &files, batch_frames)
    };
    match r {
        Ok(c) => {
            if !out_counts.is_null() {
                for (i, x) in c.into_iter().enumerate() {
                    unsafe { *out_counts.add(i) = x };
                }
            }
            0
        }
        Err(e) => {
            if comm.rank() == 0 {
                eprintln!("readcon-db mpi ingest: {e}");
            }
            -3
        }
    }
}
