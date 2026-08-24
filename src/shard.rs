//! HPC multi-writer: one LMDB env **per shard** so writers do not serialize on a single
//! write_txn. Route `traj_id % n_shards` to a shard directory.
//!
//! One writer owns each `shard_id` across the job (`traj_id % n_shards`). If many
//! ranks share a shard id, each node keeps a private tree, `drain`s to a unique dest,
//! then `join_drained_roots`. Global select fans out across shards.
//!
//! This is **not** multi-writer inside one LMDB env (impossible). It is **partitioned writers**,
//! the standard embedded pattern for high write concurrency on one filesystem.

use std::path::{Path, PathBuf};

use readcon_core::types::ConFrame;

use crate::corpus::ConCorpus;
use crate::error::{Error, Result};
use crate::keys::{FrameKey, TrajId};
use crate::select::Select;

/// Default shard count for HPC campaign roots (power of two aids routing).
pub const DEFAULT_N_SHARDS: u32 = 64;

/// Manifest file in the corpus root describing shard layout.
const MANIFEST: &str = "shards.json";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ShardManifest {
    pub n_shards: u32,
    pub version: u32,
}

/// Multi-shard campaign corpus: `root/shard_XXXX/` each holds an independent `ConCorpus`.
pub struct ShardedConCorpus {
    root: PathBuf,
    n_shards: u32,
    /// Lazily opened shards (only those touched). Avoid opening all 10^6 writers' shards in one process.
    shards: Vec<Option<ConCorpus>>,
}

impl ShardedConCorpus {
    /// Open a sharded root that already has `shards.json`. Does not mkdir.
    pub fn open_existing(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if !root.join(MANIFEST).is_file() {
            return Err(Error::Message(format!(
                "missing shards.json: {}",
                root.display()
            )));
        }
        Self::open(root, 1)
    }

    /// Create or open a sharded root. If manifest missing, writes one with `n_shards`.
    pub fn open(root: impl AsRef<Path>, n_shards: u32) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        let manifest_path = root.join(MANIFEST);
        let n_shards = if manifest_path.is_file() {
            let s = std::fs::read_to_string(&manifest_path)?;
            let m: ShardManifest = serde_json::from_str(&s)?;
            m.n_shards
        } else {
            if n_shards == 0 {
                return Err(Error::Message("n_shards must be >= 1".into()));
            }
            let m = ShardManifest {
                n_shards,
                version: 1,
            };
            std::fs::write(&manifest_path, serde_json::to_string_pretty(&m)?)?;
            n_shards
        };
        let mut shards = Vec::with_capacity(n_shards as usize);
        shards.resize_with(n_shards as usize, || None);
        Ok(Self {
            root,
            n_shards,
            shards,
        })
    }

    pub fn n_shards(&self) -> u32 {
        self.n_shards
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    #[inline]
    pub fn shard_for_traj(traj_id: TrajId, n_shards: u32) -> u32 {
        (traj_id % u64::from(n_shards)) as u32
    }

    fn shard_path(&self, shard_id: u32) -> PathBuf {
        self.root.join(format!("shard_{shard_id:04}"))
    }

    fn shard_has_data(&self, shard_id: u32) -> bool {
        self.shard_path(shard_id).join("data.mdb").is_file()
    }

    /// Open one shard env (creates dir). Safe for many processes to open **different** shards.
    pub fn shard_mut(&mut self, shard_id: u32) -> Result<&ConCorpus> {
        if shard_id >= self.n_shards {
            return Err(Error::Message(format!(
                "shard_id {shard_id} >= n_shards {}",
                self.n_shards
            )));
        }
        let i = shard_id as usize;
        if self.shards[i].is_none() {
            let p = self.shard_path(shard_id);
            self.shards[i] = Some(ConCorpus::open(p)?);
        }
        Ok(self.shards[i].as_ref().unwrap())
    }

    /// Open only the shard for `traj_id` (HPC rank typically owns one shard).
    pub fn open_shard_for_traj(
        root: impl AsRef<Path>,
        traj_id: TrajId,
    ) -> Result<(u32, ConCorpus)> {
        let root = root.as_ref();
        let manifest_path = root.join(MANIFEST);
        let n_shards = if manifest_path.is_file() {
            let m: ShardManifest = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
            m.n_shards
        } else {
            let _ = Self::open(root, DEFAULT_N_SHARDS)?;
            DEFAULT_N_SHARDS
        };
        let sid = Self::shard_for_traj(traj_id, n_shards);
        let corpus = ConCorpus::open(root.join(format!("shard_{sid:04}")))?;
        Ok((sid, corpus))
    }

    /// Open a **single** shard by id. That rank must be the only writer of
    /// `shard_id` in the job, or drain to a unique dest and `join_drained_roots`.
    pub fn open_shard(root: impl AsRef<Path>, shard_id: u32) -> Result<ConCorpus> {
        let root = root.as_ref();
        let manifest_path = root.join(MANIFEST);
        let n_shards = if manifest_path.is_file() {
            let m: ShardManifest = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
            m.n_shards
        } else {
            // Ensure manifest exists for readers.
            let _ = Self::open(root, DEFAULT_N_SHARDS)?;
            DEFAULT_N_SHARDS
        };
        if shard_id >= n_shards {
            return Err(Error::Message(format!(
                "shard_id {shard_id} >= n_shards {n_shards}"
            )));
        }
        ConCorpus::open(root.join(format!("shard_{shard_id:04}")))
    }

    pub fn append_trajectory_path(
        &mut self,
        traj_id: TrajId,
        file: impl AsRef<Path>,
    ) -> Result<u32> {
        self.append_trajectory_path_units(traj_id, file, None)
    }

    pub fn append_trajectory_path_units(
        &mut self,
        traj_id: TrajId,
        file: impl AsRef<Path>,
        units: Option<serde_json::Value>,
    ) -> Result<u32> {
        let sid = Self::shard_for_traj(traj_id, self.n_shards);
        let c = self.shard_mut(sid)?;
        c.append_trajectory_path_units(traj_id, file, units)
    }

    pub fn append_trajectory_str(
        &mut self,
        traj_id: TrajId,
        contents: &str,
        source: impl Into<String>,
    ) -> Result<u32> {
        let sid = Self::shard_for_traj(traj_id, self.n_shards);
        let c = self.shard_mut(sid)?;
        c.append_trajectory_str(traj_id, contents, source)
    }

    pub fn append_trajectory_frames(
        &mut self,
        traj_id: TrajId,
        frames: &[ConFrame],
        source: impl Into<String>,
    ) -> Result<u32> {
        let sid = Self::shard_for_traj(traj_id, self.n_shards);
        let c = self.shard_mut(sid)?;
        c.append_trajectory_frames(traj_id, frames, source)
    }

    /// Fan-out select across shards that already exist (does not mint empty envs).
    pub fn select(&mut self, sel: &Select) -> Result<Vec<FrameKey>> {
        let mut out = Vec::new();
        for sid in 0..self.n_shards {
            if !self.shard_has_data(sid) {
                continue;
            }
            let c = self.shard_mut(sid)?;
            out.extend(c.select(sel)?);
        }
        out.sort();
        if let Some(lim) = sel.limit {
            out.truncate(lim);
        }
        Ok(out)
    }

    pub fn get_frame_text(&mut self, key: FrameKey) -> Result<String> {
        let sid = Self::shard_for_traj(key.traj_id, self.n_shards);
        if !self.shard_has_data(sid) {
            return Err(Error::Message(format!(
                "shard_{sid:04} is not a corpus directory"
            )));
        }
        self.shard_mut(sid)?.get_frame_text(key)
    }

    pub fn reindex_all(&mut self) -> Result<u32> {
        let mut n = 0u32;
        for sid in 0..self.n_shards {
            if self.shard_has_data(sid) {
                n += self.shard_mut(sid)?.reindex()?;
            }
        }
        Ok(n)
    }

    /// Compact-copy each present shard onto `dst` (data.mdb only, no lockfile).
    /// Refuses a dest shard that already exists so two node-local trees that
    /// share a shard id cannot last-writer-wins.
    pub fn drain_to(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<u32> {
        let src = src.as_ref();
        let dst = dst.as_ref();
        let man = src.join(MANIFEST);
        if !man.is_file() {
            return Err(Error::Message("drain: missing shards.json".into()));
        }
        let dest_was_new = !dst.exists();
        std::fs::create_dir_all(dst)?;
        let dest_man = dst.join(MANIFEST);
        if dest_man.is_file() {
            let existing: ShardManifest =
                serde_json::from_str(&std::fs::read_to_string(&dest_man)?)?;
            let incoming: ShardManifest = serde_json::from_str(&std::fs::read_to_string(&man)?)?;
            if existing.n_shards != incoming.n_shards {
                return Err(Error::Message(
                    "drain: dest shards.json n_shards does not match src".into(),
                ));
            }
        }
        let m: ShardManifest = serde_json::from_str(&std::fs::read_to_string(&man)?)?;
        for i in 0..m.n_shards {
            let name = format!("shard_{i:04}");
            if src.join(&name).join("data.mdb").is_file()
                && dst.join(&name).join("data.mdb").is_file()
            {
                return Err(Error::Message(format!(
                    "drain: dest {name} exists; refuse overwrite. Drain each node to a unique dest, then join-drained."
                )));
            }
        }
        let copied_manifest = !dest_man.is_file();
        if copied_manifest {
            std::fs::copy(&man, &dest_man)?;
        }
        let mut created = Vec::new();
        let written = (|| -> Result<u32> {
            let mut n = 0u32;
            for i in 0..m.n_shards {
                let name = format!("shard_{i:04}");
                let from = src.join(&name);
                if !from.join("data.mdb").is_file() {
                    continue;
                }
                let to = dst.join(&name);
                let ro = ConCorpus::open_readonly(&from)?;
                ro.snapshot_to(&to)?;
                ro.close();
                created.push(to);
                n += 1;
            }
            Ok(n)
        })();
        if written.is_err() {
            if dest_was_new {
                let _ = std::fs::remove_dir_all(dst);
            } else {
                for p in &created {
                    let _ = std::fs::remove_dir_all(p);
                }
                if copied_manifest {
                    let _ = std::fs::remove_file(&dest_man);
                }
            }
        }
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test")
            .join(name)
    }

    #[test]
    fn parallel_writers_different_shards() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("hpc");
        // 8 shards, 8 threads each write traj_id == shard so zero writer contention across envs.
        let n_shards = 8u32;
        ShardedConCorpus::open(&root, n_shards).unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        let root = Arc::new(root);
        let mut joins = Vec::new();
        for sid in 0..n_shards {
            let root = Arc::clone(&root);
            let text = text.clone();
            joins.push(thread::spawn(move || {
                // Each writer opens **only its shard** (HPC rank pattern).
                let db = ShardedConCorpus::open_shard(root.as_path(), sid).unwrap();
                let traj = u64::from(sid); // maps to this shard
                db.append_trajectory_str(traj, &text, format!("shard{sid}"))
                    .unwrap()
            }));
        }
        let mut ns = Vec::new();
        for j in joins {
            ns.push(j.join().unwrap());
        }
        assert!(ns.iter().all(|&n| n >= 1));
        let mut fan = ShardedConCorpus::open(root.as_path(), n_shards).unwrap();
        let keys = fan.select(&Select::new().require_symbol("Cu")).unwrap();
        drop(fan);
        let drained = dir.path().join("pfs");
        let ncopy = ShardedConCorpus::drain_to(root.as_path(), &drained).unwrap();
        assert_eq!(ncopy, n_shards);
        assert!(drained.join("shards.json").is_file());
        assert!(drained.join("shard_0000").join("data.mdb").is_file());
        assert!(!drained.join("shard_0000").join("lock.mdb").is_file());
        let dest_sz = std::fs::metadata(drained.join("shard_0000").join("data.mdb"))
            .unwrap()
            .len();
        assert!(
            dest_sz < 64 * 1024 * 1024,
            "compact snapshot must not materialize the 2 GiB map, got {dest_sz}"
        );
        assert!(ShardedConCorpus::drain_to(root.as_path(), &drained).is_err());
        assert!(drained.join("shards.json").is_file());
        assert_eq!(keys.len(), 8);
        let joined = dir.path().join("joined");
        let mut drained_root = ShardedConCorpus::open(&drained, n_shards).unwrap();
        let njoin = drained_root.join_to_single_env(&joined).unwrap();
        assert!(njoin >= n_shards);
        let single = ConCorpus::open(&joined).unwrap();
        let jk = single.select(&Select::new().require_symbol("Cu")).unwrap();
        assert_eq!(jk.len(), keys.len());
    }

    #[test]
    fn open_shard_for_traj_writes_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fresh");
        let (sid, db) = ShardedConCorpus::open_shard_for_traj(&root, 0).unwrap();
        assert_eq!(sid, 0);
        drop(db);
        assert!(root.join("shards.json").is_file());
    }

    #[test]
    fn join_drained_duplicate_traj_does_not_create_dest() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        ShardedConCorpus::open(&a, 1).unwrap();
        ShardedConCorpus::open(&b, 1).unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        ShardedConCorpus::open_shard(&a, 0)
            .unwrap()
            .append_trajectory_str(7, &text, "a")
            .unwrap();
        ShardedConCorpus::open_shard(&b, 0)
            .unwrap()
            .append_trajectory_str(7, &text, "b")
            .unwrap();
        let dest = dir.path().join("out");
        let err = join_drained_roots(&[a, b], &dest).unwrap_err();
        assert!(err.to_string().contains("traj_id"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn join_drained_roots_missing_source_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let dest = dir.path().join("out");
        let err = join_drained_roots(&[missing], &dest).unwrap_err();
        assert!(err.to_string().contains("missing shards.json"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn drain_refuse_does_not_write_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("hpc");
        ShardedConCorpus::open(&root, 2).unwrap();
        let db = ShardedConCorpus::open_shard(&root, 0).unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        db.append_trajectory_str(0, &text, "s0").unwrap();
        drop(db);
        let dest = dir.path().join("pfs");
        std::fs::create_dir_all(dest.join("shard_0000")).unwrap();
        std::fs::write(dest.join("shard_0000").join("data.mdb"), b"x").unwrap();
        let err = ShardedConCorpus::drain_to(&root, &dest).unwrap_err();
        assert!(err.to_string().contains("refuse overwrite"), "{err}");
        assert!(
            !dest.join("shards.json").is_file(),
            "refuse must not leave dest shards.json"
        );
    }

    #[test]
    fn traj_routing_stable() {
        assert_eq!(ShardedConCorpus::shard_for_traj(0, 64), 0);
        assert_eq!(ShardedConCorpus::shard_for_traj(65, 64), 1);
    }

    /// Strong-scaling HPC story: concurrent writers on distinct shards, then
    /// fan-out select agrees with a **single-env** corpus that ingested the
    /// same trajectory texts (ground truth membership).
    #[test]
    fn multi_shard_writers_select_matches_single_env_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("hpc_scale");
        let baseline = dir.path().join("single_env");
        let n_shards = 4u32;
        ShardedConCorpus::open(&root, n_shards).unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        let root_a = Arc::new(root.clone());
        let mut joins = Vec::new();
        for sid in 0..n_shards {
            let root = Arc::clone(&root_a);
            let text = text.clone();
            joins.push(thread::spawn(move || {
                let db = ShardedConCorpus::open_shard(root.as_path(), sid).unwrap();
                let traj = u64::from(sid);
                db.append_trajectory_str(traj, &text, format!("s{sid}"))
                    .unwrap()
            }));
        }
        let mut frames_per_traj = Vec::new();
        for j in joins {
            frames_per_traj.push(j.join().unwrap());
        }
        assert!(frames_per_traj.iter().all(|&n| n >= 1));

        // Single-env ground truth: same traj_ids and CON text.
        let single = ConCorpus::open(&baseline).unwrap();
        for sid in 0..n_shards {
            let traj = u64::from(sid);
            let n = single
                .append_trajectory_str(traj, &text, format!("s{sid}"))
                .unwrap();
            assert_eq!(n, frames_per_traj[sid as usize]);
        }

        let mut fan = ShardedConCorpus::open(&root, n_shards).unwrap();
        let sharded_keys = fan.select(&Select::new().require_symbol("Cu")).unwrap();
        let base_keys = single.select(&Select::new().require_symbol("Cu")).unwrap();
        assert_eq!(sharded_keys.len(), base_keys.len());
        let mut sk: Vec<_> = sharded_keys
            .iter()
            .map(|k| (k.traj_id, k.frame_idx))
            .collect();
        let mut bk: Vec<_> = base_keys.iter().map(|k| (k.traj_id, k.frame_idx)).collect();
        sk.sort_unstable();
        bk.sort_unstable();
        assert_eq!(sk, bk, "fan-out select must match single-env membership");

        // Spot-check text blobs agree for each key.
        for (tid, fidx) in &sk {
            let key = crate::keys::FrameKey {
                traj_id: *tid,
                frame_idx: *fidx,
            };
            let a = fan.get_frame_text(key).unwrap();
            let b = single.get_frame_text(key).unwrap();
            assert_eq!(a, b);
        }
    }
}

/// Exportable corpus layout kinds for analysis handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorpusExportKind {
    /// Full sharded root (`shards.json` + `shard_XXXX/`).
    ShardedLmdb,
    /// Single-env LMDB directory (one `ConCorpus::open` path).
    SingleEnvLmdb,
    /// Filtered extXYZ for external tools (non-LMDB).
    ExtXyz,
}

impl CorpusExportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ShardedLmdb => "sharded-lmdb",
            Self::SingleEnvLmdb => "single-env-lmdb",
            Self::ExtXyz => "extxyz",
        }
    }
}

impl ShardedConCorpus {
    /// **Join:** copy all frames from every shard into a **new single-env** corpus at `dst`
    /// (traj_id preserved; collision if same traj_id appears on two shards is an error).
    /// Secondary indexes built via normal append/prepare on each blob. Reversible with
    /// [`Self::split_single_to_sharded`] using the same `n_shards` and traj routing.
    pub fn join_to_single_env(&mut self, dst: impl AsRef<Path>) -> Result<u32> {
        let dst = dst.as_ref();
        if dst.exists() {
            return Err(Error::Message(format!(
                "join dest exists: {}",
                dst.display()
            )));
        }
        let mut preview = std::collections::BTreeMap::new();
        for sid in 0..self.n_shards {
            if !self.shard_has_data(sid) {
                continue;
            }
            for fk in self.shard_mut(sid)?.list_frame_keys()? {
                preview_traj(&mut preview, fk.traj_id, u64::from(sid))?;
            }
        }
        let n = (|| -> Result<u32> {
            let out = ConCorpus::open(dst)?;
            let mut seen_traj = std::collections::BTreeSet::new();
            let n = append_sharded_into(self, &out, &mut seen_traj)?;
            out.close();
            Ok(n)
        })();
        rollback_new_dest(dst, n)
    }

    /// Join into a temp single-env, export extxyz, then remove the temp dest.
    pub fn export_extxyz(
        &mut self,
        sel: &Select,
        out: impl AsRef<Path>,
        energy_key: &str,
    ) -> Result<u32> {
        let joined = std::env::temp_dir().join(format!(
            "readcon_db_join_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        if joined.exists() {
            return Err(Error::Message(
                "export_extxyz: temp join dest exists".into(),
            ));
        }
        struct RemoveOnDrop(PathBuf);
        impl Drop for RemoveOnDrop {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _guard = RemoveOnDrop(joined.clone());
        self.join_to_single_env(&joined)?;
        let db = ConCorpus::open_readonly(&joined)?;
        let keys = db.select(sel)?;
        let n = db.export_extxyz(&keys, out, energy_key)?;
        db.close();
        Ok(n as u32)
    }

    /// **Split:** read a **single-env** corpus and write a new sharded root at `dst_root`
    /// with `n_shards` (rewrites manifest). Traj_id preserved; routing is `traj_id % n_shards`.
    pub fn split_single_to_sharded(
        single: &ConCorpus,
        dst_root: impl AsRef<Path>,
        n_shards: u32,
    ) -> Result<u32> {
        if n_shards == 0 {
            return Err(Error::Message("n_shards must be >= 1".into()));
        }
        let dst_root = dst_root.as_ref();
        if dst_root.exists() {
            return Err(Error::Message(format!(
                "split dest exists: {}",
                dst_root.display()
            )));
        }
        let n = (|| -> Result<u32> {
            let mut sharded = ShardedConCorpus::open(dst_root, n_shards)?;
            let keys = single.list_frame_keys()?;
            let mut by_traj: std::collections::BTreeMap<u64, Vec<FrameKey>> =
                std::collections::BTreeMap::new();
            for fk in keys {
                by_traj.entry(fk.traj_id).or_default().push(fk);
            }
            let mut n = 0u32;
            for (tid, mut fks) in by_traj {
                fks.sort();
                let mut concat = String::new();
                for fk in &fks {
                    concat.push_str(&single.get_frame_text(*fk)?);
                }
                let nf = sharded.append_trajectory_str(tid, &concat, "split-from-single")?;
                n += nf;
            }
            Ok(n)
        })();
        rollback_new_dest(dst_root, n)
    }
}

fn append_sharded_into(
    sh: &mut ShardedConCorpus,
    out: &ConCorpus,
    seen_traj: &mut std::collections::BTreeSet<u64>,
) -> Result<u32> {
    let mut n = 0u32;
    for sid in 0..sh.n_shards {
        if !sh.shard_has_data(sid) {
            continue;
        }
        let shard = sh.shard_mut(sid)?;
        let keys = shard.list_frame_keys()?;
        let mut by_traj: std::collections::BTreeMap<u64, Vec<FrameKey>> =
            std::collections::BTreeMap::new();
        for fk in keys {
            by_traj.entry(fk.traj_id).or_default().push(fk);
        }
        for (tid, mut fks) in by_traj {
            if !seen_traj.insert(tid) {
                return Err(Error::Message(format!(
                    "traj_id {tid} appears in multiple shards or join sources"
                )));
            }
            fks.sort();
            let mut concat = String::new();
            for fk in &fks {
                concat.push_str(&shard.get_frame_text(*fk)?);
            }
            n += out.append_trajectory_str(tid, &concat, format!("join-from-shard-{sid}"))?;
        }
    }
    Ok(n)
}

/// Join several drained sharded roots (unique dests after refuse-overwrite)
/// into one single-env corpus. Traj ids must be unique across sources.
pub fn join_drained_roots(sources: &[PathBuf], dst: impl AsRef<Path>) -> Result<u32> {
    if sources.is_empty() {
        return Err(Error::Message("join-drained: no sources".into()));
    }
    let dst = dst.as_ref();
    if dst.exists() {
        return Err(Error::Message(format!(
            "join-drained dest exists: {}",
            dst.display()
        )));
    }
    for src in sources {
        if !src.join(MANIFEST).is_file() {
            return Err(Error::Message(format!(
                "join-drained: missing shards.json: {}",
                src.display()
            )));
        }
    }
    {
        let mut preview = std::collections::BTreeMap::new();
        for (si, src) in sources.iter().enumerate() {
            let mut sh = ShardedConCorpus::open_existing(src)?;
            for sid in 0..sh.n_shards {
                if !sh.shard_has_data(sid) {
                    continue;
                }
                let shard = sh.shard_mut(sid)?;
                let owner = ((si as u64) << 32) | u64::from(sid);
                for fk in shard.list_frame_keys()? {
                    preview_traj(&mut preview, fk.traj_id, owner)?;
                }
            }
        }
    }
    let n = (|| -> Result<u32> {
        let out = ConCorpus::open(dst)?;
        let mut n = 0u32;
        let mut seen = std::collections::BTreeSet::new();
        for src in sources {
            let mut sh = ShardedConCorpus::open_existing(src)?;
            n += append_sharded_into(&mut sh, &out, &mut seen)?;
        }
        out.close();
        Ok(n)
    })();
    rollback_new_dest(dst, n)
}

/// Join any set of **single-env** corpus directories into one destination (traj_id must be unique).
pub fn join_corpus_dirs(sources: &[PathBuf], dst: impl AsRef<Path>) -> Result<u32> {
    let dst = dst.as_ref();
    if dst.exists() {
        return Err(Error::Message(format!(
            "join dest exists: {}",
            dst.display()
        )));
    }
    for src in sources {
        if !src.join("data.mdb").is_file() {
            return Err(Error::Message(format!(
                "join: missing data.mdb: {}",
                src.display()
            )));
        }
    }
    {
        let mut preview = std::collections::BTreeMap::new();
        for (si, src) in sources.iter().enumerate() {
            let c = ConCorpus::open_readonly(src)?;
            for fk in c.list_frame_keys()? {
                preview_traj(&mut preview, fk.traj_id, si as u64)?;
            }
            c.close();
        }
    }
    let n = (|| -> Result<u32> {
        let out = ConCorpus::open(dst)?;
        let mut n = 0u32;
        let mut seen = std::collections::BTreeSet::new();
        for src in sources {
            let c = ConCorpus::open_readonly(src)?;
            let keys = c.list_frame_keys()?;
            let mut by_traj: std::collections::BTreeMap<u64, Vec<FrameKey>> =
                std::collections::BTreeMap::new();
            for fk in keys {
                by_traj.entry(fk.traj_id).or_default().push(fk);
            }
            for (tid, mut fks) in by_traj {
                if !seen.insert(tid) {
                    return Err(Error::Message(format!(
                        "duplicate traj_id {tid} across join sources"
                    )));
                }
                fks.sort();
                let mut concat = String::new();
                for fk in &fks {
                    concat.push_str(&c.get_frame_text(*fk)?);
                }
                n += out.append_trajectory_str(tid, &concat, src.display().to_string())?;
            }
            c.close();
        }
        out.close();
        Ok(n)
    })();
    rollback_new_dest(dst, n)
}

fn rollback_new_dest<T>(dst: &Path, r: Result<T>) -> Result<T> {
    if r.is_err() {
        let _ = std::fs::remove_dir_all(dst);
    }
    r
}

/// Record `traj_id` under `owner`. Extra frames of the same traj on the
/// same owner are fine; the same traj on a different owner is a collision.
fn preview_traj(
    preview: &mut std::collections::BTreeMap<u64, u64>,
    traj_id: u64,
    owner: u64,
) -> Result<()> {
    match preview.entry(traj_id) {
        std::collections::btree_map::Entry::Occupied(e) if *e.get() != owner => {
            Err(Error::Message(format!(
                "traj_id {traj_id} appears in multiple shards or join sources"
            )))
        }
        std::collections::btree_map::Entry::Occupied(_) => Ok(()),
        std::collections::btree_map::Entry::Vacant(e) => {
            e.insert(owner);
            Ok(())
        }
    }
}

/// Open a single-env corpus for analysis export. Refuses a sharded root so
/// `ConCorpus::open` cannot mint `data.mdb` next to `shards.json`.
pub fn open_single_env_for_export(src: impl AsRef<Path>) -> Result<ConCorpus> {
    let src = src.as_ref();
    if src.join(MANIFEST).is_file() {
        return Err(Error::Message(
            "compact-export-extxyz: sharded root needs --sharded".into(),
        ));
    }
    ConCorpus::open_readonly(src)
}

#[cfg(test)]
mod compaction_tests {
    use super::*;
    use crate::keys::FrameKey;
    use crate::select::Select;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test")
            .join(name)
    }

    #[test]
    fn join_split_reversible_membership() {
        let dir = tempfile::tempdir().unwrap();
        let sharded_root = dir.path().join("sharded");
        let con_text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        {
            let mut s = ShardedConCorpus::open(&sharded_root, 4).unwrap();
            for tid in [0u64, 1, 2, 3] {
                s.append_trajectory_str(tid, &con_text, "t").unwrap();
            }
        }
        let mut s = ShardedConCorpus::open(&sharded_root, 4).unwrap();
        let before = s.select(&Select::new()).unwrap();
        assert_eq!(before.len(), 4);

        let joined = dir.path().join("joined");
        let n = s.join_to_single_env(&joined).unwrap();
        assert_eq!(n, 4);
        assert!(s.join_to_single_env(&joined).is_err());
        let joined_c = ConCorpus::open(&joined).unwrap();
        let mid = joined_c.select(&Select::new()).unwrap();
        assert_eq!(mid.len(), 4);

        let split_root = dir.path().join("split_again");
        let n2 = ShardedConCorpus::split_single_to_sharded(&joined_c, &split_root, 4).unwrap();
        assert_eq!(n2, 4);
        let mut s2 = ShardedConCorpus::open(&split_root, 4).unwrap();
        let after = s2.select(&Select::new()).unwrap();
        assert_eq!(after.len(), before.len());
        // same traj set
        let mut bt: Vec<_> = before.iter().map(|k| k.traj_id).collect();
        let mut at: Vec<_> = after.iter().map(|k| k.traj_id).collect();
        bt.sort();
        at.sort();
        assert_eq!(bt, at);
    }

    #[test]
    fn join_drained_roots_keeps_disjoint_trajs_same_shard() {
        let dir = tempfile::tempdir().unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        let n_shards = 2u32;
        let node_a = dir.path().join("node_a");
        let node_b = dir.path().join("node_b");
        ShardedConCorpus::open(&node_a, n_shards).unwrap();
        ShardedConCorpus::open(&node_b, n_shards).unwrap();
        // traj 0 and traj 2 both route to shard_0000
        ShardedConCorpus::open_shard(&node_a, 0)
            .unwrap()
            .append_trajectory_str(0, &text, "a")
            .unwrap();
        ShardedConCorpus::open_shard(&node_b, 0)
            .unwrap()
            .append_trajectory_str(2, &text, "b")
            .unwrap();
        let dest_a = dir.path().join("dest_a");
        let dest_b = dir.path().join("dest_b");
        assert_eq!(ShardedConCorpus::drain_to(&node_a, &dest_a).unwrap(), 1);
        assert_eq!(ShardedConCorpus::drain_to(&node_b, &dest_b).unwrap(), 1);
        assert!(ShardedConCorpus::drain_to(&node_a, &dest_a).is_err());
        let joined = dir.path().join("joined");
        let n = join_drained_roots(&[dest_a.clone(), dest_b.clone()], &joined).unwrap();
        assert!(n >= 2);
        let db = ConCorpus::open(&joined).unwrap();
        let keys = db.select(&Select::new()).unwrap();
        let mut tids: Vec<u64> = keys.iter().map(|k| k.traj_id).collect();
        tids.sort_unstable();
        tids.dedup();
        assert_eq!(tids, vec![0, 2]);
        assert!(join_drained_roots(&[dest_a, dest_b], &joined).is_err());
    }

    #[test]
    fn join_drained_roots_refuses_existing_dest() {
        let dir = tempfile::tempdir().unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        let node = dir.path().join("node");
        ShardedConCorpus::open(&node, 2).unwrap();
        ShardedConCorpus::open_shard(&node, 0)
            .unwrap()
            .append_trajectory_str(0, &text, "a")
            .unwrap();
        let dest = dir.path().join("dest");
        assert_eq!(ShardedConCorpus::drain_to(&node, &dest).unwrap(), 1);
        let joined = dir.path().join("joined");
        join_drained_roots(&[dest.clone()], &joined).unwrap();
        let err = join_drained_roots(&[dest], &joined).unwrap_err();
        assert!(
            err.to_string().contains("join-drained dest exists"),
            "{err}"
        );
    }

    #[test]
    fn export_kinds_documented() {
        assert_eq!(CorpusExportKind::ShardedLmdb.as_str(), "sharded-lmdb");
        assert_eq!(CorpusExportKind::SingleEnvLmdb.as_str(), "single-env-lmdb");
        assert_eq!(CorpusExportKind::ExtXyz.as_str(), "extxyz");
    }

    #[test]
    fn select_skips_empty_shard_dir_without_data() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sharded");
        let mut s = ShardedConCorpus::open(&root, 4).unwrap();
        std::fs::create_dir_all(root.join("shard_0001")).unwrap();
        let keys = s.select(&Select::new()).unwrap();
        assert!(keys.is_empty());
        assert!(!root.join("shard_0001").join("data.mdb").exists());
        let err = s
            .get_frame_text(FrameKey {
                traj_id: 1,
                frame_idx: 0,
            })
            .unwrap_err();
        assert!(
            err.to_string().contains("is not a corpus directory"),
            "{err}"
        );
        assert!(!root.join("shard_0001").join("data.mdb").exists());
    }

    #[test]
    fn select_skips_missing_shard_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sharded");
        let mut s = ShardedConCorpus::open(&root, 8).unwrap();
        s.append_trajectory_path(0, fixture("tiny_cuh2.con"))
            .unwrap();
        drop(s);
        let mut s = ShardedConCorpus::open_existing(&root).unwrap();
        let keys = s.select(&Select::new()).unwrap();
        assert_eq!(keys.len(), 1);
        let present: Vec<u32> = (0..8)
            .filter(|&i| root.join(format!("shard_{i:04}")).is_dir())
            .collect();
        assert_eq!(present, vec![0]);
    }

    #[test]
    fn get_frame_text_missing_shard_does_not_mint() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sharded");
        let mut s = ShardedConCorpus::open(&root, 4).unwrap();
        let err = s
            .get_frame_text(FrameKey {
                traj_id: 1,
                frame_idx: 0,
            })
            .unwrap_err();
        assert!(
            err.to_string().contains("is not a corpus directory"),
            "{err}"
        );
        assert!(!root.join("shard_0001").exists());
    }

    #[test]
    fn join_drained_roots_keeps_multi_frame_traj() {
        let dir = tempfile::tempdir().unwrap();
        let text = std::fs::read_to_string(fixture("tiny_multi_cuh2.con")).unwrap();
        let node = dir.path().join("node");
        ShardedConCorpus::open(&node, 2).unwrap();
        ShardedConCorpus::open_shard(&node, 0)
            .unwrap()
            .append_trajectory_str(0, &text, "a")
            .unwrap();
        let dest = dir.path().join("dest");
        assert_eq!(ShardedConCorpus::drain_to(&node, &dest).unwrap(), 1);
        let joined = dir.path().join("joined");
        let n = join_drained_roots(&[dest], &joined).unwrap();
        assert!(n >= 2, "joined frames={n}");
    }

    #[test]
    fn join_corpus_dirs_keeps_multi_frame_traj() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        ConCorpus::open(&a)
            .unwrap()
            .append_trajectory_path(1, fixture("tiny_multi_cuh2.con"))
            .unwrap();
        let dest = dir.path().join("out");
        let n = join_corpus_dirs(&[a], &dest).unwrap();
        assert!(n >= 2, "joined frames={n}");
    }

    #[test]
    fn drain_to_failure_removes_shards_created_this_call() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        ShardedConCorpus::open(&src, 2).unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        ShardedConCorpus::open_shard(&src, 0)
            .unwrap()
            .append_trajectory_str(0, &text, "a")
            .unwrap();
        ShardedConCorpus::open_shard(&src, 1)
            .unwrap()
            .append_trajectory_str(1, &text, "b")
            .unwrap();
        let dest = dir.path().join("dest");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("shard_0001"), b"not-a-dir").unwrap();
        assert!(ShardedConCorpus::drain_to(&src, &dest).is_err());
        assert!(!dest.join("shard_0000").exists());
    }

    #[test]
    fn rollback_new_dest_removes_dir() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("x"), b"y").unwrap();
        let err = rollback_new_dest::<u32>(&dest, Err(Error::Message("boom".into()))).unwrap_err();
        assert!(err.to_string().contains("boom"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn join_to_single_env_keeps_multi_frame_traj() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sharded");
        let mut s = ShardedConCorpus::open(&root, 2).unwrap();
        s.append_trajectory_path(0, fixture("tiny_multi_cuh2.con"))
            .unwrap();
        let dest = dir.path().join("out");
        let n = s.join_to_single_env(&dest).unwrap();
        assert!(n >= 2, "joined frames={n}");
        let db = ConCorpus::open(&dest).unwrap();
        let keys = db.select(&Select::new()).unwrap();
        assert!(keys.len() >= 2);
        assert!(keys.iter().all(|k| k.traj_id == 0));
    }

    #[test]
    fn join_to_single_env_duplicate_traj_does_not_create_dest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sharded");
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        let mut s = ShardedConCorpus::open(&root, 2).unwrap();
        s.shard_mut(0)
            .unwrap()
            .append_trajectory_str(0, &text, "a")
            .unwrap();
        s.shard_mut(1)
            .unwrap()
            .append_trajectory_str(0, &text, "b")
            .unwrap();
        let dest = dir.path().join("out");
        let err = s.join_to_single_env(&dest).unwrap_err();
        assert!(err.to_string().contains("traj_id"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn join_corpus_dirs_duplicate_does_not_create_dest() {
        let dir = tempfile::tempdir().unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        ConCorpus::open(&a)
            .unwrap()
            .append_trajectory_str(1, &text, "a")
            .unwrap();
        ConCorpus::open(&b)
            .unwrap()
            .append_trajectory_str(1, &text, "b")
            .unwrap();
        let dest = dir.path().join("out");
        let err = join_corpus_dirs(&[a, b], &dest).unwrap_err();
        assert!(
            err.to_string().contains("traj_id") || err.to_string().contains("duplicate"),
            "{err}"
        );
        assert!(!dest.exists());
    }

    #[test]
    fn join_corpus_dirs_missing_mdb_does_not_create_dest() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        std::fs::create_dir_all(&missing).unwrap();
        let dest = dir.path().join("out");
        let err = join_corpus_dirs(&[missing], &dest).unwrap_err();
        assert!(err.to_string().contains("missing data.mdb"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn open_single_env_for_export_refuses_sharded_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sharded");
        ShardedConCorpus::open(&root, 2).unwrap();
        assert!(!root.join("data.mdb").exists());
        match open_single_env_for_export(&root) {
            Ok(_) => panic!("expected sharded-root refuse"),
            Err(err) => assert!(err.to_string().contains("--sharded"), "{err}"),
        }
        assert!(!root.join("data.mdb").exists());
    }

    #[test]
    fn export_extxyz_removes_temp_join() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sharded");
        let mut s = ShardedConCorpus::open(&root, 2).unwrap();
        s.append_trajectory_path(0, fixture("tiny_cuh2.con"))
            .unwrap();
        let out = dir.path().join("out.xyz");
        let n = s.export_extxyz(&Select::new(), &out, "energy").unwrap();
        assert!(n >= 1);
        assert!(out.is_file());
        let prefix = format!("readcon_db_join_{}_", std::process::id());
        let leftovers: Vec<_> = std::env::temp_dir()
            .read_dir()
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn open_existing_error_is_missing_shards_json() {
        let dir = tempfile::tempdir().unwrap();
        match ShardedConCorpus::open_existing(dir.path().join("nope")) {
            Ok(_) => panic!("expected missing shards.json"),
            Err(err) => {
                assert!(err.to_string().starts_with("missing shards.json:"), "{err}");
                assert!(!err.to_string().contains("join-drained"));
            }
        }
    }
}
