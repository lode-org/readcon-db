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
//! when that frame does not shrink). A duplicate whose first occurrence is
//! in the same batch sends no blob; the owner still has that batch's bytes.
//! A duplicate whose winner was in an earlier batch sends its bytes again,
//! so the owner does not keep every distinct blob until the file ends.
//! The owner then packs each aligned run of 64 frames into one zstd group
//! when that is shorter, and stages the group. xxh3-128 was taken on the UTF-8.
//!
//! - single writer: rank 0 owns the one corpus;
//! - sharded: shard `s = traj_id % n_shards` is owned by rank `s % P`.
//!
//! Each owner receives records in source rank order, which is global frame
//! order, and stages them in that order inside one LMDB write transaction
//! per environment. Frames and trajectory metadata commit together. Dropping
//! the transaction, which is what a failure or a crash does, leaves neither.
//! Keys, hash winners and metadata therefore equal the serial ingest of the
//! same files in the same order, for every rank count.
//!
//! Collective byte counts are checked against `i32::MAX` before they are
//! narrowed. A batch that would overflow is split. Record lengths are `u64`.
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

use crate::corpus::{ConCorpus, IngestSession};
use crate::error::{Error, Result};
use crate::keys::{FrameKey, TrajId};
use crate::mpi_wire::{
    fit_u32, fits_i32, hash_gather_chunk_frames, mpi_i32, prefix_i32, push_record, read_bytes,
    read_u64, record_bytes, send_stored_bytes,
};
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
        if i >= words.len() {
            return Err(Error::Message("truncated frame-offset broadcast".into()));
        }
        let k = usize::try_from(words[i])
            .map_err(|_| Error::Message("frame-offset count exceeds usize".into()))?;
        let end = i
            .checked_add(1)
            .and_then(|s| s.checked_add(k))
            .ok_or_else(|| Error::Message("frame-offset table overflows".into()))?;
        if end > words.len() {
            return Err(Error::Message("truncated frame-offset broadcast".into()));
        }
        let offs = words[i + 1..end].to_vec();
        i = end;
        if offs.is_empty() {
            return Err(Error::Message("frame-offset table is empty".into()));
        }
        let tab = FileTab {
            traj_id: *t,
            path: p.clone(),
            offs,
            g0,
        };
        g0 = g0
            .checked_add(tab.n())
            .ok_or_else(|| Error::Message("frame count overflows".into()))?;
        tabs.push(tab);
    }
    if i != words.len() {
        return Err(Error::Message("trailing frame-offset bytes".into()));
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
        if hi < lo {
            return Err(Error::Message(format!(
                "{}: frame offsets go backwards",
                t.path.display()
            )));
        }
        let span = usize::try_from(hi - lo).map_err(|_| {
            Error::Message(format!(
                "{}: frame byte span {} exceeds usize",
                t.path.display(),
                hi - lo
            ))
        })?;
        let mut f = std::fs::File::open(&t.path)?;
        f.seek(SeekFrom::Start(lo))?;
        let mut buf = vec![0u8; span];
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
                frame_idx: fit_u32(i)?,
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

/// Map headroom per input byte reserved before an MPI ingest.
const INPUT_MAP_FACTOR: usize = 3;

/// Where a trajectory's frames are written: (owner rank, env id).
type Route<'a> = &'a dyn Fn(TrajId) -> (usize, u32);

struct Held {
    fk: FrameKey,
    enc: Vec<u8>,
    stored: Vec<u8>,
}

fn pack_held(mut rows: Vec<Held>) -> Result<Vec<Held>> {
    let texts: Vec<(u32, String)> = rows
        .iter()
        .map(|r| Ok((r.fk.frame_idx, crate::frame_codec::load_frame(&r.stored)?)))
        .collect::<Result<_>>()?;
    let view: Vec<(u32, &str)> = texts.iter().map(|(i, s)| (*i, s.as_str())).collect();
    let packed = crate::frame_codec::pack_aligned(&view)?;
    for (row, stored) in rows.iter_mut().zip(packed) {
        row.stored = stored;
    }
    Ok(rows)
}

/// Emit every aligned group that is fully present, in buffer order.
/// A short tail stays buffered until its trajectory's last frame has arrived.
fn drain_ready(buf: &mut Vec<Held>, n_of: &HashMap<TrajId, u32>) -> Result<Vec<Held>> {
    let mut ready = Vec::new();
    loop {
        if buf.is_empty() {
            break;
        }
        let fk = buf[0].fk;
        let n = *n_of
            .get(&fk.traj_id)
            .ok_or_else(|| Error::Message(format!("missing traj {}", fk.traj_id)))?;
        let start = fk.frame_idx / crate::frame_codec::GROUP_LEN * crate::frame_codec::GROUP_LEN;
        if fk.frame_idx != start {
            return Err(Error::Message(
                "frame group does not start on an aligned index".into(),
            ));
        }
        let end = (start + crate::frame_codec::GROUP_LEN).min(n);
        let need = (end - start) as usize;
        if buf.len() < need {
            break;
        }
        let last = buf[need - 1].fk;
        if last.traj_id != fk.traj_id || last.frame_idx + 1 != end {
            break;
        }
        let group: Vec<Held> = buf.drain(..need).collect();
        ready.extend(pack_held(group)?);
    }
    Ok(ready)
}

fn flush_ready(
    sessions: &mut HashMap<u32, IngestSession<'_>>,
    bufs: &mut HashMap<u32, Vec<Held>>,
    id: u32,
    n_of: &HashMap<TrajId, u32>,
) -> Result<()> {
    let ready = match bufs.get_mut(&id) {
        Some(buf) => drain_ready(buf, n_of)?,
        None => return Ok(()),
    };
    if ready.is_empty() {
        return Ok(());
    }
    let rows = ready.into_iter().map(|r| (r.fk, r.enc, r.stored)).collect();
    sessions
        .get_mut(&id)
        .ok_or_else(|| Error::Message(format!("missing env {id}")))?
        .put_encoded(rows)
}

fn rank_span(n: usize, size: usize, rank: usize, g0: usize, g1: usize) -> std::ops::Range<usize> {
    let mine = block_range(n, size, rank);
    mine.start.max(g0)..mine.end.min(g1)
}

fn gather_hashes<C: Communicator>(
    comm: &C,
    n: usize,
    local: &Local,
    rank: usize,
    size: usize,
) -> Result<Vec<u8>> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let total = n
        .checked_mul(16)
        .ok_or_else(|| Error::Message(format!("hash exchange of {n} frames overflows")))?;
    let mut all_h = vec![0u8; total];
    let chunk = hash_gather_chunk_frames().max(1);
    let mut g0 = 0usize;
    while g0 < n {
        let g1 = (g0 + chunk).min(n);
        let counts = (0..size)
            .map(|r| {
                let bytes = rank_span(n, size, r, g0, g1)
                    .len()
                    .checked_mul(16)
                    .ok_or_else(|| Error::Message("hash count overflows".into()))?;
                mpi_i32(bytes as u64)
            })
            .collect::<Result<Vec<_>>>()?;
        let displs = prefix_i32(&counts)?;
        let mine = rank_span(n, size, rank, g0, g1);
        // Capacity 1 keeps the empty send pointer live. Open MPI rejects a
        // dangling buffer even when the count is zero.
        let mut my = Vec::with_capacity(mine.len().saturating_mul(16).saturating_add(1));
        for g in mine {
            my.extend_from_slice(&local.hashes[g - local.first]);
        }
        let mut part = PartitionMut::new(&mut all_h[g0 * 16..g1 * 16], &counts[..], &displs[..]);
        comm.all_gather_varcount_into(&my[..], &mut part);
        g0 = g1;
    }
    Ok(all_h)
}

fn plan_counts(
    local: &Local,
    mine: std::ops::Range<usize>,
    b0: usize,
    b1: usize,
    size: usize,
    env_of: &[(usize, u32)],
    first_of: &[usize],
) -> Result<Vec<u64>> {
    let mut scounts = vec![0u64; size];
    for g in mine.start.max(b0)..mine.end.min(b1) {
        let i = g - local.first;
        let n = record_bytes(
            local.enc[i].len(),
            local.stored[i].len(),
            send_stored_bytes(first_of[g], g, b0),
        );
        let dest = env_of[g].0;
        scounts[dest] = scounts[dest]
            .checked_add(n)
            .ok_or_else(|| Error::Message("MPI message length overflows u64".into()))?;
    }
    Ok(scounts)
}

fn build_messages(
    local: &Local,
    mine: std::ops::Range<usize>,
    b0: usize,
    b1: usize,
    size: usize,
    env_of: &[(usize, u32)],
    first_of: &[usize],
) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new(); size];
    for g in mine.start.max(b0)..mine.end.min(b1) {
        let i = g - local.first;
        let blob: &[u8] = if send_stored_bytes(first_of[g], g, b0) {
            &local.stored[i]
        } else {
            &[]
        };
        push_record(&mut out[env_of[g].0], g as u64, &local.enc[i], blob);
    }
    out
}

/// Exchange frames in `[b0, b1)` and return the rank-local receive buffer.
/// `b1` may be shorter than requested when a full batch would exceed `i32::MAX`.
fn exchange_batch<C: Communicator>(
    comm: &C,
    local: &Local,
    mine: std::ops::Range<usize>,
    b0: usize,
    batch: usize,
    n: usize,
    size: usize,
    env_of: &[(usize, u32)],
    first_of: &[usize],
) -> Result<(usize, Vec<u8>)> {
    let mut b1 = (b0 + batch).min(n);
    loop {
        let planned = plan_counts(local, mine.clone(), b0, b1, size, env_of, first_of);
        if !agree(comm, planned.is_ok()) {
            return match planned {
                Err(e) => Err(e),
                Ok(_) => Err(Error::Message("MPI ingest failed on another rank".into())),
            };
        }
        let scounts = planned.expect("plan succeeded on every rank");
        let mut rcounts = vec![0u64; size];
        comm.all_to_all_into(&scounts[..], &mut rcounts[..]);
        let fit = fits_i32(&scounts) && fits_i32(&rcounts);
        if !agree(comm, fit) {
            if b1 - b0 <= 1 {
                let biggest = scounts
                    .iter()
                    .chain(rcounts.iter())
                    .copied()
                    .max()
                    .unwrap_or(0);
                return Err(Error::Message(format!(
                    "MPI message of {biggest} bytes exceeds i32::MAX"
                )));
            }
            b1 = b0 + (b1 - b0) / 2;
            continue;
        }
        let out = build_messages(local, mine.clone(), b0, b1, size, env_of, first_of);
        let lens: Vec<u64> = out.iter().map(|m| m.len() as u64).collect();
        if !agree(comm, lens == scounts) {
            return Err(Error::Message(
                "MPI batch length does not match the planned count".into(),
            ));
        }
        let packed = (|| -> Result<_> {
            let sc_i32 = scounts
                .iter()
                .copied()
                .map(mpi_i32)
                .collect::<Result<Vec<_>>>()?;
            let rc_i32 = rcounts
                .iter()
                .copied()
                .map(mpi_i32)
                .collect::<Result<Vec<_>>>()?;
            let sd = prefix_i32(&sc_i32)?;
            let rd = prefix_i32(&rc_i32)?;
            Ok((sc_i32, rc_i32, sd, rd))
        })();
        if !agree(comm, packed.is_ok()) {
            return match packed {
                Err(e) => Err(e),
                Ok(_) => Err(Error::Message("MPI ingest failed on another rank".into())),
            };
        }
        let (sc_i32, rc_i32, sd, rd) = packed.expect("counts fit in i32");
        // `fits_i32` already required both totals to be <= i32::MAX.
        let send_len = usize::try_from(scounts.iter().sum::<u64>()).expect("send length fits");
        let mut send = Vec::with_capacity(1 + send_len);
        for m in &out {
            send.extend_from_slice(m);
        }
        let rtot = usize::try_from(rcounts.iter().sum::<u64>()).expect("receive length fits");
        let mut recv = vec![0u8; rtot + 1];
        recv.truncate(rtot);
        {
            let sp = mpi::datatype::Partition::new(&send[..], &sc_i32[..], &sd[..]);
            let mut rp = PartitionMut::new(&mut recv[..], &rc_i32[..], &rd[..]);
            comm.all_to_all_varcount_into(&sp, &mut rp);
        }
        return Ok((b1, recv));
    }
}

fn decode_batch(
    recv: &[u8],
    b0: usize,
    b1: usize,
    first_of: &[usize],
    tabs: &[FileTab],
    env_of: &[(usize, u32)],
) -> Result<Vec<(u32, Held)>> {
    let mut reuse = HashSet::new();
    for g in b0..b1 {
        let winner = first_of[g];
        if winner != g && winner >= b0 {
            reuse.insert(winner);
        }
    }
    // Winners needed again in this batch only. The map dies with the batch.
    let mut kept: HashMap<usize, Vec<u8>> = HashMap::new();
    let mut rows = Vec::new();
    let mut p = recv;
    while !p.is_empty() {
        let g = usize::try_from(read_u64(&mut p)?)
            .map_err(|_| Error::Message("frame index exceeds usize".into()))?;
        if g >= first_of.len() {
            return Err(Error::Message(format!("frame {g} outside ingest")));
        }
        let enc = read_bytes(&mut p)?.to_vec();
        let raw = read_bytes(&mut p)?;
        let winner = first_of[g];
        let stored = if winner == g {
            let bytes = raw.to_vec();
            if reuse.contains(&g) {
                kept.insert(g, bytes.clone());
            }
            bytes
        } else if winner < b0 {
            raw.to_vec()
        } else {
            kept.get(&winner)
                .cloned()
                .ok_or_else(|| Error::Message(format!("missing dedup payload for frame {g}")))?
        };
        let t = &tabs[file_of(tabs, g)];
        rows.push((
            env_of[g].1,
            Held {
                fk: FrameKey {
                    traj_id: t.traj_id,
                    frame_idx: fit_u32(g - t.g0)?,
                },
                enc,
                stored,
            },
        ));
    }
    Ok(rows)
}

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
    let counts = collective(
        comm,
        tabs.iter()
            .map(|t| fit_u32(t.n()))
            .collect::<Result<Vec<_>>>(),
    )?;

    // Owners open their envs. The existence check happens inside the write
    // transaction below, so a second writer cannot pass it first.
    let mut envs: HashMap<u32, ConCorpus> = HashMap::new();
    let opened = (|| -> Result<()> {
        for t in &tabs {
            let (owner, env) = route(t.traj_id);
            if owner == rank {
                if let std::collections::hash_map::Entry::Vacant(v) = envs.entry(env) {
                    v.insert(sink(env)?);
                }
            }
        }
        Ok(())
    })();
    collective(comm, opened)?;

    // Size each writer's map from the input so large ingests cannot hit
    // MDB_MAP_FULL: frame text, index keys and B-tree overhead stay below
    // INPUT_MAP_FACTOR times the CON bytes routed to that env.
    // The resize must finish before the ingest transaction opens.
    let mut env_bytes: HashMap<u32, usize> = HashMap::new();
    let measured = (|| -> Result<()> {
        for t in &tabs {
            let (owner, env) = route(t.traj_id);
            if owner == rank {
                let bytes = usize::try_from(*t.offs.last().unwrap_or(&0)).map_err(|_| {
                    Error::Message(format!("{} is longer than usize", t.path.display()))
                })?;
                let slot = env_bytes.entry(env).or_default();
                *slot = slot
                    .checked_add(bytes)
                    .ok_or_else(|| Error::Message(format!("input bytes for env {env} overflow")))?;
            }
        }
        Ok(())
    })();
    collective(comm, measured)?;
    let grown = env_bytes.iter().try_for_each(|(env, b)| {
        envs[env]
            .reserve_map(b.saturating_mul(INPUT_MAP_FACTOR))
            .map(|_| ())
    });
    collective(comm, grown)?;

    let mut sessions: HashMap<u32, IngestSession<'_>> = HashMap::new();
    let begun = (|| -> Result<()> {
        for (id, corpus) in &envs {
            sessions.insert(*id, IngestSession::begin(corpus)?);
        }
        for t in &tabs {
            let (owner, env) = route(t.traj_id);
            if owner == rank {
                sessions
                    .get_mut(&env)
                    .ok_or_else(|| Error::Message(format!("missing env {env}")))?
                    .claim(t.traj_id)?;
            }
        }
        Ok(())
    })();
    collective(comm, begun)?;

    let mine = block_range(n, size, rank);
    let local = collective(comm, parse_local(&tabs, mine.clone()))?;
    let all_h = collective(comm, gather_hashes(comm, n, &local, rank, size))?;

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

    let n_of: HashMap<TrajId, u32> = tabs
        .iter()
        .zip(counts.iter())
        .map(|(t, n_frames)| (t.traj_id, *n_frames))
        .collect();
    let mut bufs: HashMap<u32, Vec<Held>> = HashMap::new();
    let mut b0 = 0usize;
    while b0 < n {
        let exchanged = exchange_batch(
            comm,
            &local,
            mine.clone(),
            b0,
            batch,
            n,
            size,
            &env_of,
            &first_of,
        );
        let (b1, recv) = collective(comm, exchanged)?;
        let decoded = decode_batch(&recv, b0, b1, &first_of, &tabs, &env_of);
        for (id, held) in collective(comm, decoded)? {
            bufs.entry(id).or_default().push(held);
        }
        let ids: Vec<u32> = bufs.keys().copied().collect();
        let put = ids
            .iter()
            .try_for_each(|id| flush_ready(&mut sessions, &mut bufs, *id, &n_of));
        collective(comm, put)?;
        b0 = b1;
    }
    let ids: Vec<u32> = bufs.keys().copied().collect();
    let tail = ids.iter().try_for_each(|id| {
        flush_ready(&mut sessions, &mut bufs, *id, &n_of)?;
        if bufs.get(id).is_some_and(|b| !b.is_empty()) {
            return Err(Error::Message(
                "MPI ingest left an unfinished frame group".into(),
            ));
        }
        Ok(())
    });
    collective(comm, tail)?;
    let meta = tabs
        .iter()
        .zip(counts.iter())
        .try_for_each(|(t, n_frames)| {
            let (owner, env) = route(t.traj_id);
            if owner == rank {
                sessions
                    .get_mut(&env)
                    .ok_or_else(|| Error::Message(format!("missing env {env}")))?
                    .put_traj_meta(t.traj_id, *n_frames, t.path.display().to_string())
            } else {
                Ok(())
            }
        });
    collective(comm, meta)?;
    // Every rank reached here, so every owner can commit. A failure before
    // this point dropped `sessions` and aborted the uncommitted transaction.
    let committed = sessions
        .into_iter()
        .try_for_each(|(_, session)| session.commit());
    collective(comm, committed)?;
    Ok(counts)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_ingest_commits_nothing_and_leaves_an_earlier_traj() {
        let universe = mpi::initialize().expect("MPI init");
        let comm = universe.world();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let good = root.join("resources/test/tiny_cuh2.con");
        // One directory, created on rank 0, so every rank reads the same file.
        let hold = if comm.rank() == 0 {
            let dir = tempfile::tempdir().unwrap();
            let mut text = std::fs::read_to_string(&good).unwrap();
            text.push_str("\nthis is not a frame\n");
            std::fs::write(dir.path().join("bad.con"), text).unwrap();
            Some(dir)
        } else {
            None
        };
        let dir_bytes = if comm.rank() == 0 {
            hold.as_ref()
                .unwrap()
                .path()
                .as_os_str()
                .as_encoded_bytes()
                .to_vec()
        } else {
            Vec::new()
        };
        let dir = PathBuf::from(
            std::str::from_utf8(&bcast_bytes(&comm, dir_bytes)).expect("dir path is utf-8"),
        );
        let bad = dir.join("bad.con");

        let n = append_trajectory_path(&comm, &dir, 1, &good, 1).unwrap();
        assert!(n > 0);

        // Reusing traj 1 must fail without deleting the frames committed above.
        let err = append_trajectory_paths(&comm, &dir, &[(1, good.clone()), (2, bad.clone())], 1)
            .unwrap_err();
        assert!(!err.to_string().is_empty(), "{err}");

        // A parse error on any rank aborts before commit, so traj 2 is absent.
        let err = append_trajectory_path(&comm, &dir, 2, &bad, 1).unwrap_err();
        assert!(!err.to_string().is_empty(), "{err}");
        if comm.rank() == 0 {
            let db = ConCorpus::open(&dir).unwrap();
            assert_eq!(db.traj_meta(1).unwrap().unwrap().n_frames, n);
            assert!(db.traj_meta(2).unwrap().is_none());
            assert!(db.list_frame_keys().unwrap().iter().all(|k| k.traj_id == 1));
        }
        let _ = hold;
    }
}
