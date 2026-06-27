//! Embedded CON/convel corpus store on LMDB via Heed.
//!
//! - **frames**: `(traj_id, frame_idx) →` raw frame text (UTF-8 CON fragment)
//! - **traj_meta**: `traj_id →` JSON `{ n_frames, source }`
//! - **idx_natoms**: `(n_atoms, traj_id, frame_idx) →` empty (range scans)
//! - **idx_symbol**: `(symbol, traj_id, frame_idx) →` empty (equality / multi-symbol ∩)
//!
//! Selection is explicit Rust filters over secondary indexes — no SQL.
//! Decode always uses [`readcon_core::iterators::ConFrameIterator`] so CON
//! semantics stay owned by readcon-core.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use heed::types::{Bytes, Str, Unit};
use heed::{Database, Env, EnvOpenOptions};
use readcon_core::iterators::ConFrameIterator;
use readcon_core::types::ConFrame;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type TrajId = u64;
pub type FrameIdx = u32;

const MAP_SIZE: usize = 1024 * 1024 * 1024; // 1 GiB default map; grow via reopen if needed
const MAX_DBS: u32 = 16;

#[derive(Error, Debug)]
pub enum Error {
    #[error("heed: {0}")]
    Heed(#[from] heed::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse frame: {0}")]
    Parse(String),
    #[error("missing frame {0:?}")]
    MissingFrame(FrameKey),
    #[error("trajectory {0} already exists (n_frames={1}); use a new traj_id")]
    TrajExists(TrajId, u32),
    #[error("utf-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameKey {
    pub traj_id: TrajId,
    pub frame_idx: FrameIdx,
}

impl FrameKey {
    pub fn to_bytes(self) -> [u8; 12] {
        let mut out = [0u8; 12];
        out[..8].copy_from_slice(&self.traj_id.to_be_bytes());
        out[8..].copy_from_slice(&self.frame_idx.to_be_bytes());
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != 12 {
            return None;
        }
        let mut t = [0u8; 8];
        let mut f = [0u8; 4];
        t.copy_from_slice(&b[..8]);
        f.copy_from_slice(&b[8..]);
        Some(Self {
            traj_id: u64::from_be_bytes(t),
            frame_idx: u32::from_be_bytes(f),
        })
    }
}

/// Secondary index key: n_atoms (BE u32) || FrameKey bytes
fn natoms_key(n_atoms: u32, fk: FrameKey) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(&n_atoms.to_be_bytes());
    out[4..].copy_from_slice(&fk.to_bytes());
    out
}

fn natoms_key_prefix(n_atoms: u32) -> [u8; 4] {
    n_atoms.to_be_bytes()
}

/// symbol (utf-8) || 0xff || FrameKey — 0xff cannot appear in ASCII element symbols
fn symbol_key(symbol: &str, fk: FrameKey) -> Vec<u8> {
    let mut v = symbol.as_bytes().to_vec();
    v.push(0xff);
    v.extend_from_slice(&fk.to_bytes());
    v
}

fn symbol_prefix(symbol: &str) -> Vec<u8> {
    let mut v = symbol.as_bytes().to_vec();
    v.push(0xff);
    v
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrajMeta {
    pub n_frames: u32,
    pub source: String,
}

#[derive(Clone, Debug, Default)]
pub struct Select {
    pub traj_id: Option<TrajId>,
    pub natoms_min: Option<u32>,
    pub natoms_max: Option<u32>,
    pub symbols_all: Vec<String>,
    pub limit: Option<usize>,
}

impl Select {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn trajectory(mut self, id: TrajId) -> Self {
        self.traj_id = Some(id);
        self
    }
    pub fn natoms_range(mut self, min: u32, max: u32) -> Self {
        self.natoms_min = Some(min);
        self.natoms_max = Some(max);
        self
    }
    pub fn require_symbol(mut self, s: impl Into<String>) -> Self {
        self.symbols_all.push(s.into());
        self
    }
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }
}

pub struct ConCorpus {
    path: PathBuf,
    env: Env,
    frames: Database<Bytes, Str>,
    traj_meta: Database<Bytes, Str>,
    idx_natoms: Database<Bytes, Unit>,
    idx_symbol: Database<Bytes, Unit>,
}

impl ConCorpus {
    /// Open or create an LMDB environment at `path` (directory).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(&path)?;
        let mut opts = EnvOpenOptions::new();
        opts.map_size(MAP_SIZE);
        opts.max_dbs(MAX_DBS);
        let env = unsafe { opts.open(&path)? };

        let mut wtxn = env.write_txn()?;
        let frames = env.create_database(&mut wtxn, Some("frames"))?;
        let traj_meta = env.create_database(&mut wtxn, Some("traj_meta"))?;
        let idx_natoms = env.create_database(&mut wtxn, Some("idx_natoms"))?;
        let idx_symbol = env.create_database(&mut wtxn, Some("idx_symbol"))?;
        wtxn.commit()?;

        Ok(Self {
            path,
            env,
            frames,
            traj_meta,
            idx_natoms,
            idx_symbol,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Ingest a multi-frame CON/convel file as trajectory `traj_id`.
    /// Stores each frame's text blob and updates secondary indexes.
    pub fn append_trajectory_path(&self, traj_id: TrajId, file: impl AsRef<Path>) -> Result<u32> {
        let text = fs::read_to_string(file.as_ref())?;
        self.append_trajectory_str(traj_id, &text, file.as_ref().display().to_string())
    }

    pub fn append_trajectory_str(
        &self,
        traj_id: TrajId,
        file_contents: &str,
        source: impl Into<String>,
    ) -> Result<u32> {
        let source = source.into();
        let mut wtxn = self.env.write_txn()?;

        let tid_key = traj_id.to_be_bytes();
        if let Some(existing) = self.traj_meta.get(&wtxn, &tid_key[..])? {
            let meta: TrajMeta = serde_json::from_str(existing)?;
            return Err(Error::TrajExists(traj_id, meta.n_frames));
        }

        let mut frame_idx: u32 = 0;
        // Split by iterating and re-serializing is hard; store slices by scanning offsets.
        // ConFrameIterator yields frames; we need original text slices.
        // Use line-based re-parse: collect frames via iterator and write canonical via writer would diverge.
        // Practical approach: walk iterator and for each successful frame, write the frame using
        // positions from a manual scan with ConFrameIterator on substrings is complex.
        // Simpler: split multi-frame by re-reading — use iterator and store Display from writer.
        use readcon_core::writer::ConFrameWriter;
        use std::io::Cursor;

        for item in ConFrameIterator::new(file_contents) {
            let frame = item.map_err(|e| Error::Parse(e.to_string()))?;
            let mut buf = Cursor::new(Vec::new());
            {
                let mut w = ConFrameWriter::new(&mut buf);
                w.write_frame(&frame)
                    .map_err(|e| Error::Parse(format!("re-serialize: {e}")))?;
            }
            let blob = String::from_utf8(buf.into_inner())
                .map_err(|e| Error::Parse(format!("utf8: {e}")))?;

            let fk = FrameKey {
                traj_id,
                frame_idx,
            };
            let fk_b = fk.to_bytes();
            self.frames.put(&mut wtxn, &fk_b[..], blob.as_str())?;

            let n_atoms = frame.atom_data.len() as u32;
            let nk = natoms_key(n_atoms, fk);
            self.idx_natoms.put(&mut wtxn, &nk[..], &())?;

            let mut syms = BTreeSet::new();
            for a in &frame.atom_data {
                if !a.symbol.is_empty() {
                    syms.insert(a.symbol.to_string());
                }
            }
            for s in &syms {
                let sk = symbol_key(s, fk);
                self.idx_symbol.put(&mut wtxn, &sk[..], &())?;
            }

            frame_idx += 1;
        }

        let meta = TrajMeta {
            n_frames: frame_idx,
            source,
        };
        let meta_s = serde_json::to_string(&meta)?;
        self.traj_meta
            .put(&mut wtxn, &tid_key[..], meta_s.as_str())?;
        wtxn.commit()?;
        Ok(frame_idx)
    }

    pub fn traj_meta(&self, traj_id: TrajId) -> Result<Option<TrajMeta>> {
        let rtxn = self.env.read_txn()?;
        let tid_key = traj_id.to_be_bytes();
        match self.traj_meta.get(&rtxn, &tid_key[..])? {
            None => Ok(None),
            Some(s) => Ok(Some(serde_json::from_str(s)?)),
        }
    }

    /// Raw CON text for one frame (as re-serialized at ingest).
    pub fn get_frame_text(&self, key: FrameKey) -> Result<String> {
        let rtxn = self.env.read_txn()?;
        let fk_b = key.to_bytes();
        match self.frames.get(&rtxn, &fk_b[..])? {
            Some(s) => Ok(s.to_owned()),
            None => Err(Error::MissingFrame(key)),
        }
    }

    /// Decode one frame via readcon-core.
    pub fn get_frame(&self, key: FrameKey) -> Result<ConFrame> {
        let text = self.get_frame_text(key)?;
        let mut it = ConFrameIterator::new(&text);
        let fr = it
            .next()
            .ok_or_else(|| Error::Parse("empty blob".into()))?
            .map_err(|e| Error::Parse(e.to_string()))?;
        Ok(fr)
    }

    /// Non-SQL selection using secondary indexes (intersection).
    pub fn select(&self, sel: &Select) -> Result<Vec<FrameKey>> {
        let rtxn = self.env.read_txn()?;
        let mut sets: Vec<BTreeSet<FrameKey>> = Vec::new();

        if sel.natoms_min.is_some() || sel.natoms_max.is_some() {
            let lo = sel.natoms_min.unwrap_or(0);
            let hi = sel.natoms_max.unwrap_or(u32::MAX);
            let mut s = BTreeSet::new();
            // Full index scan with BE-ordered keys: stop once n_atoms > hi.
            let mut iter = self.idx_natoms.iter(&rtxn)?;
            while let Some(Ok((k, _))) = iter.next() {
                if k.len() < 16 {
                    continue;
                }
                let mut nb = [0u8; 4];
                nb.copy_from_slice(&k[..4]);
                let n = u32::from_be_bytes(nb);
                if n > hi {
                    break;
                }
                if n >= lo {
                    if let Some(fk) = FrameKey::from_bytes(&k[4..16]) {
                        if sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                            s.insert(fk);
                        }
                    }
                }
            }
            sets.push(s);
        }

        for sym in &sel.symbols_all {
            let mut s = BTreeSet::new();
            let pref = symbol_prefix(sym);
            let mut iter = self.idx_symbol.prefix_iter(&rtxn, &pref)?;
            while let Some(Ok((k, _))) = iter.next() {
                if k.len() < 12 {
                    continue;
                }
                let fk_bytes = &k[k.len() - 12..];
                if let Some(fk) = FrameKey::from_bytes(fk_bytes) {
                    if sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                        s.insert(fk);
                    }
                }
            }
            sets.push(s);
        }

        // traj-only or full scan of frames for this traj
        if sets.is_empty() {
            let mut s = BTreeSet::new();
            let mut iter = self.frames.iter(&rtxn)?;
            while let Some(Ok((k, _))) = iter.next() {
                if let Some(fk) = FrameKey::from_bytes(k) {
                    if sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                        s.insert(fk);
                    }
                }
            }
            sets.push(s);
        } else if let Some(tid) = sel.traj_id {
            // filter all sets by traj (already applied in loops, but if only symbols...)
            for set in &mut sets {
                set.retain(|fk| fk.traj_id == tid);
            }
        }

        let mut acc = sets.remove(0);
        for s in sets {
            acc = acc.intersection(&s).copied().collect();
        }

        let mut out: Vec<FrameKey> = acc.into_iter().collect();
        out.sort();
        if let Some(lim) = sel.limit {
            out.truncate(lim);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../readcon-core/resources/test")
            .join(name)
    }

    #[test]
    fn frame_key_order() {
        let a = FrameKey {
            traj_id: 1,
            frame_idx: 2,
        };
        let b = FrameKey {
            traj_id: 1,
            frame_idx: 10,
        };
        assert!(a.to_bytes() < b.to_bytes());
        assert_eq!(FrameKey::from_bytes(&a.to_bytes()).unwrap(), a);
    }

    #[test]
    fn ingest_select_natoms_and_symbol() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        let n = db
            .append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        assert!(n >= 1);
        let meta = db.traj_meta(1).unwrap().unwrap();
        assert_eq!(meta.n_frames, n);

        let keys = db
            .select(&Select::new().trajectory(1).natoms_range(1, 10_000))
            .unwrap();
        assert_eq!(keys.len() as u32, n);

        let cu = db
            .select(&Select::new().require_symbol("Cu").trajectory(1))
            .unwrap();
        assert!(!cu.is_empty());

        let fr = db.get_frame(FrameKey {
            traj_id: 1,
            frame_idx: 0,
        })
        .unwrap();
        assert!(!fr.atom_data.is_empty());

        // multi-frame file
        let n2 = db
            .append_trajectory_path(2, fixture("tiny_multi_cuh2.con"))
            .unwrap();
        assert!(n2 >= 2);
        let all2 = db.select(&Select::new().trajectory(2)).unwrap();
        assert_eq!(all2.len() as u32, n2);
    }
}
