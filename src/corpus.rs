use std::collections::BTreeSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use heed::types::{Bytes, Str, Unit};
use heed::{Database, Env, EnvOpenOptions};
use readcon_core::iterators::ConFrameIterator;
use readcon_core::types::ConFrame;
use readcon_core::writer::ConFrameWriter;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::export_xyz::write_frame_extxyz;
use crate::keys::{
    hash_frame_bytes, natoms_key, symbol_key, symbol_prefix, ContentHash, FrameKey, TrajId,
};
use crate::select::Select;

const MAP_SIZE: usize = 2 * 1024 * 1024 * 1024;
const MAX_DBS: u32 = 16;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrajMeta {
    pub n_frames: u32,
    pub source: String,
}

pub struct ConCorpus {
    path: PathBuf,
    env: Env,
    frames: Database<Bytes, Str>,
    traj_meta: Database<Bytes, Str>,
    idx_natoms: Database<Bytes, Unit>,
    idx_symbol: Database<Bytes, Unit>,
    frame_by_hash: Database<Bytes, Bytes>,
    hash_by_frame: Database<Bytes, Bytes>,
}

impl ConCorpus {
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
        let frame_by_hash = env.create_database(&mut wtxn, Some("frame_by_hash"))?;
        let hash_by_frame = env.create_database(&mut wtxn, Some("hash_by_frame"))?;
        wtxn.commit()?;

        Ok(Self {
            path,
            env,
            frames,
            traj_meta,
            idx_natoms,
            idx_symbol,
            frame_by_hash,
            hash_by_frame,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

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
            let hash = hash_frame_bytes(blob.as_bytes());

            let fk = FrameKey {
                traj_id,
                frame_idx,
            };
            let fk_b = fk.to_bytes();
            self.frames.put(&mut wtxn, &fk_b[..], blob.as_str())?;

            let hb = hash.to_bytes();
            self.hash_by_frame.put(&mut wtxn, &fk_b[..], &hb[..])?;
            if self.frame_by_hash.get(&wtxn, &hb[..])?.is_none() {
                self.frame_by_hash.put(&mut wtxn, &hb[..], &fk_b[..])?;
            }

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

    pub fn get_frame_text(&self, key: FrameKey) -> Result<String> {
        let rtxn = self.env.read_txn()?;
        let fk_b = key.to_bytes();
        match self.frames.get(&rtxn, &fk_b[..])? {
            Some(s) => Ok(s.to_owned()),
            None => Err(Error::MissingFrame(key)),
        }
    }

    pub fn get_frame(&self, key: FrameKey) -> Result<ConFrame> {
        let text = self.get_frame_text(key)?;
        let mut it = ConFrameIterator::new(&text);
        let fr = it
            .next()
            .ok_or_else(|| Error::Parse("empty blob".into()))?
            .map_err(|e| Error::Parse(e.to_string()))?;
        Ok(fr)
    }

    pub fn frame_hash(&self, key: FrameKey) -> Result<ContentHash> {
        let rtxn = self.env.read_txn()?;
        let fk_b = key.to_bytes();
        match self.hash_by_frame.get(&rtxn, &fk_b[..])? {
            Some(b) => ContentHash::from_bytes(b).ok_or_else(|| Error::Message("bad hash".into())),
            None => {
                let text = self.get_frame_text(key)?;
                Ok(hash_frame_bytes(text.as_bytes()))
            }
        }
    }

    pub fn find_by_hash(&self, hash: ContentHash) -> Result<Option<FrameKey>> {
        let rtxn = self.env.read_txn()?;
        let hb = hash.to_bytes();
        match self.frame_by_hash.get(&rtxn, &hb[..])? {
            Some(b) => Ok(FrameKey::from_bytes(b)),
            None => Ok(None),
        }
    }

    pub fn hash_con_text(text: &str) -> Result<ContentHash> {
        let mut it = ConFrameIterator::new(text);
        let frame = it
            .next()
            .ok_or_else(|| Error::Parse("no frame".into()))?
            .map_err(|e| Error::Parse(e.to_string()))?;
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = ConFrameWriter::new(&mut buf);
            w.write_frame(&frame)
                .map_err(|e| Error::Parse(format!("re-serialize: {e}")))?;
        }
        Ok(hash_frame_bytes(&buf.into_inner()))
    }

    pub fn select(&self, sel: &Select) -> Result<Vec<FrameKey>> {
        let rtxn = self.env.read_txn()?;
        let mut sets: Vec<BTreeSet<FrameKey>> = Vec::new();

        if let Some(h) = sel.content_hash {
            let mut s = BTreeSet::new();
            if let Some(b) = self.frame_by_hash.get(&rtxn, &h[..])? {
                if let Some(fk) = FrameKey::from_bytes(b) {
                    if sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                        s.insert(fk);
                    }
                }
            }
            sets.push(s);
        }

        if sel.natoms_min.is_some() || sel.natoms_max.is_some() {
            let lo = sel.natoms_min.unwrap_or(0);
            let hi = sel.natoms_max.unwrap_or(u32::MAX);
            let mut s = BTreeSet::new();
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

    /// Decode selected frames to ASE/metatrain-oriented extended XYZ.
    pub fn export_extxyz(
        &self,
        keys: &[FrameKey],
        path: impl AsRef<Path>,
        energy_key: &str,
    ) -> Result<usize> {
        use std::fs::File;
        use std::io::BufWriter;
        let mut w = BufWriter::new(File::create(path)?);
        let mut n = 0usize;
        for k in keys {
            let fr = self.get_frame(*k)?;
            write_frame_extxyz(&mut w, &fr, energy_key)?;
            n += 1;
        }
        Ok(n)
    }

    /// Ingest `*.con` / `*.convel` files in a directory (non-recursive), traj ids from `start`.
    pub fn ingest_directory(
        &self,
        dir: impl AsRef<Path>,
        start_traj_id: TrajId,
    ) -> Result<Vec<(TrajId, u32, String)>> {
        let mut out = Vec::new();
        let mut tid = start_traj_id;
        let mut paths: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                name.ends_with(".con") || name.ends_with(".convel")
            })
            .collect();
        paths.sort();
        for p in paths {
            let n = self.append_trajectory_path(tid, &p)?;
            out.push((tid, n, p.display().to_string()));
            tid += 1;
        }
        Ok(out)
    }

    /// Keys that are the representative for their content hash (dedup set).
    pub fn unique_frame_keys(&self, sel: &Select) -> Result<Vec<FrameKey>> {
        let keys = self.select(sel)?;
        let mut uniq = Vec::new();
        for k in keys {
            let h = self.frame_hash(k)?;
            if self.find_by_hash(h)? == Some(k) {
                uniq.push(k);
            }
        }
        Ok(uniq)
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

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../readcon-core/resources/test")
    }

    #[test]
    fn ingest_hash_dedup_and_select() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        let n = db
            .append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        assert!(n >= 1);
        let k0 = FrameKey {
            traj_id: 1,
            frame_idx: 0,
        };
        let h = db.frame_hash(k0).unwrap();
        assert_eq!(db.find_by_hash(h).unwrap(), Some(k0));

        let by_hash = db
            .select(&Select::new().exact_hash(h.to_bytes()))
            .unwrap();
        assert_eq!(by_hash, vec![k0]);

        let cu = db
            .select(&Select::new().require_symbol("Cu").trajectory(1))
            .unwrap();
        assert!(!cu.is_empty());

        let n2 = db
            .append_trajectory_path(2, fixture("tiny_multi_cuh2.con"))
            .unwrap();
        assert!(n2 >= 2);
    }

    #[test]
    fn workflow_metatrain_extxyz_export() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path().join("corpus")).unwrap();
        // simulate multi-trajectory campaign: ingest test suite CONs
        let ingested = db.ingest_directory(fixtures_dir(), 1).unwrap();
        assert!(ingested.len() >= 3);

        // ML-style filter: frames containing Cu, bounded size
        let keys = db
            .select(
                &Select::new()
                    .require_symbol("Cu")
                    .natoms_range(1, 10_000)
                    .limit(50),
            )
            .unwrap();
        assert!(!keys.is_empty());

        // dedup for training set (exact geometry match)
        let uniq = db
            .unique_frame_keys(&Select::new().require_symbol("Cu"))
            .unwrap();
        assert!(!uniq.is_empty());
        assert!(uniq.len() <= keys.len() + 100);

        let xyz = dir.path().join("train_subset.xyz");
        let n = db.export_extxyz(&uniq, &xyz, "energy").unwrap();
        assert_eq!(n, uniq.len());
        let text = std::fs::read_to_string(&xyz).unwrap();
        assert!(text.contains("Lattice="));
        assert!(text.contains("Properties="));
        // at least one Cu line
        assert!(text.lines().any(|l| l.trim_start().starts_with("Cu ")));
    }

    #[test]
    fn workflow_dedup_identical_ingest() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        let f = fixture("tiny_cuh2.con");
        db.append_trajectory_path(1, &f).unwrap();
        // second traj same file content → same hashes, different keys
        db.append_trajectory_path(2, &f).unwrap();
        let k1 = FrameKey {
            traj_id: 1,
            frame_idx: 0,
        };
        let k2 = FrameKey {
            traj_id: 2,
            frame_idx: 0,
        };
        let h1 = db.frame_hash(k1).unwrap();
        let h2 = db.frame_hash(k2).unwrap();
        assert_eq!(h1, h2);
        // representative is first ingested
        assert_eq!(db.find_by_hash(h1).unwrap(), Some(k1));
        let uniq = db.unique_frame_keys(&Select::new()).unwrap();
        assert!(uniq.contains(&k1));
        assert!(!uniq.contains(&k2));
    }
}
