//! HPC multi-writer: one LMDB env **per shard** so writers do not serialize on a single
//! write_txn. Route `traj_id % n_shards` (or explicit `writer_id`) to a shard directory.
//!
//! Millions of ranks: assign `shard = rank % n_shards` or use traj_id space partitioned by
//! site; each rank opens **only its shard** for append. Global select fans out across shards.
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
        self.root
            .join(format!("shard_{shard_id:04}"))
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
    pub fn open_shard_for_traj(root: impl AsRef<Path>, traj_id: TrajId) -> Result<(u32, ConCorpus)> {
        let root = root.as_ref();
        let manifest_path = root.join(MANIFEST);
        let n_shards = if manifest_path.is_file() {
            let m: ShardManifest = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
            m.n_shards
        } else {
            DEFAULT_N_SHARDS
        };
        let sid = Self::shard_for_traj(traj_id, n_shards);
        let corpus = ConCorpus::open(root.join(format!("shard_{sid:04}")))?;
        Ok((sid, corpus))
    }

    /// Open a **single** shard by id (rank `r` uses `open_shard(root, r % n)`).
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
        let sid = Self::shard_for_traj(traj_id, self.n_shards);
        let c = self.shard_mut(sid)?;
        c.append_trajectory_path(traj_id, file)
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

    /// Fan-out select across all shards (read-only; opens missing shards).
    pub fn select(&mut self, sel: &Select) -> Result<Vec<FrameKey>> {
        let mut out = Vec::new();
        for sid in 0..self.n_shards {
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
        self.shard_mut(sid)?.get_frame_text(key)
    }

    pub fn reindex_all(&mut self) -> Result<u32> {
        let mut n = 0u32;
        for sid in 0..self.n_shards {
            if self.shard_path(sid).is_dir() {
                n += self.shard_mut(sid)?.reindex()?;
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../readcon-core/resources/test")
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
        assert_eq!(keys.len(), 8);
    }

    #[test]
    fn traj_routing_stable() {
        assert_eq!(ShardedConCorpus::shard_for_traj(0, 64), 0);
        assert_eq!(ShardedConCorpus::shard_for_traj(65, 64), 1);
    }
}
