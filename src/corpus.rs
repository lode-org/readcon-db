use std::collections::BTreeSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use heed::types::{Bytes, Str, Unit};
use heed::{Database, Env, EnvOpenOptions, RwTxn};
use readcon_core::iterators::ConFrameIterator;
use readcon_core::types::ConFrame;
use readcon_core::writer::ConFrameWriter;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::export_xyz::write_frame_extxyz;
use crate::keys::{
    composition_formula, elem_count_key, elem_count_symbol_prefix, energy_bin_key, flag_key,
    fmax_bin_key, formula_key, formula_prefix, hash_frame_bytes, natoms_key, ordered_f64_bits,
    parse_elem_count_key, species_counts_from_symbols, symbol_key, symbol_prefix, ContentHash,
    FrameKey, TrajId, FLAG_HAS_ENERGY, FLAG_HAS_FORCES, FLAG_HAS_VELOCITIES,
};
use crate::select::Select;

const MAP_SIZE: usize = 2 * 1024 * 1024 * 1024;
const MAX_DBS: u32 = 32;

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
    idx_energy: Database<Bytes, Unit>,
    idx_flags: Database<Bytes, Unit>,
    /// Per-element atom counts: symbol ‖ 0xff ‖ BE count ‖ FrameKey
    idx_elem_count: Database<Bytes, Unit>,
    /// Canonical formula ‖ 0xff ‖ FrameKey
    idx_formula: Database<Bytes, Unit>,
    /// Ordered max force magnitude (forces present only)
    idx_fmax: Database<Bytes, Unit>,
    frame_by_hash: Database<Bytes, Bytes>,
    hash_by_frame: Database<Bytes, Bytes>,
}

fn frame_has_forces(frame: &ConFrame) -> bool {
    frame
        .header
        .sections
        .iter()
        .any(|s| s.eq_ignore_ascii_case("forces"))
        || frame.atom_data.iter().any(|a| a.force.is_some())
}

fn frame_has_velocities(frame: &ConFrame) -> bool {
    frame
        .header
        .sections
        .iter()
        .any(|s| s.eq_ignore_ascii_case("velocities"))
        || frame.atom_data.iter().any(|a| a.velocity.is_some())
}

fn frame_energy(frame: &ConFrame) -> Option<f64> {
    frame
        .header
        .energy()
        .filter(|e| e.is_finite())
        .or_else(|| {
            frame
                .header
                .metadata
                .get("energy")
                .and_then(|v| v.as_f64())
                .filter(|e| e.is_finite())
        })
}

/// Euclidean max ||F_i|| over atoms with force data; None if no forces.
pub fn frame_fmax(frame: &ConFrame) -> Option<f64> {
    let mut m = None;
    for a in &frame.atom_data {
        if let Some(f) = a.force {
            let mag = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
            if mag.is_finite() {
                m = Some(m.map_or(mag, |cur: f64| cur.max(mag)));
            }
        }
    }
    m
}

fn frame_species(frame: &ConFrame) -> Vec<(String, u32)> {
    species_counts_from_symbols(frame.atom_data.iter().map(|a| a.symbol.as_ref()))
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
        let idx_energy = env.create_database(&mut wtxn, Some("idx_energy"))?;
        let idx_flags = env.create_database(&mut wtxn, Some("idx_flags"))?;
        let idx_elem_count = env.create_database(&mut wtxn, Some("idx_elem_count"))?;
        let idx_formula = env.create_database(&mut wtxn, Some("idx_formula"))?;
        let idx_fmax = env.create_database(&mut wtxn, Some("idx_fmax"))?;
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
            idx_energy,
            idx_flags,
            idx_elem_count,
            idx_formula,
            idx_fmax,
            frame_by_hash,
            hash_by_frame,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn index_frame(&self, wtxn: &mut RwTxn, fk: FrameKey, frame: &ConFrame, blob: &str) -> Result<()> {
        let fk_b = fk.to_bytes();
        self.frames.put(wtxn, &fk_b[..], blob)?;

        let hash = hash_frame_bytes(blob.as_bytes());
        let hb = hash.to_bytes();
        self.hash_by_frame.put(wtxn, &fk_b[..], &hb[..])?;
        if self.frame_by_hash.get(wtxn, &hb[..])?.is_none() {
            self.frame_by_hash.put(wtxn, &hb[..], &fk_b[..])?;
        }

        let n_atoms = frame.atom_data.len() as u32;
        let nk = natoms_key(n_atoms, fk);
        self.idx_natoms.put(wtxn, &nk[..], &())?;

        let counts = frame_species(frame);
        let mut syms = BTreeSet::new();
        for (sym, cnt) in &counts {
            syms.insert(sym.clone());
            let ek = elem_count_key(sym, *cnt, fk);
            self.idx_elem_count.put(wtxn, &ek[..], &())?;
        }
        for s in &syms {
            let sk = symbol_key(s, fk);
            self.idx_symbol.put(wtxn, &sk[..], &())?;
        }
        let formula = composition_formula(&counts);
        if !formula.is_empty() {
            let fk_form = formula_key(&formula, fk);
            self.idx_formula.put(wtxn, &fk_form[..], &())?;
        }

        if let Some(e) = frame_energy(frame) {
            if let Some(ek) = energy_bin_key(e, fk) {
                self.idx_energy.put(wtxn, &ek[..], &())?;
            }
            let fk_flag = flag_key(FLAG_HAS_ENERGY, fk);
            self.idx_flags.put(wtxn, &fk_flag[..], &())?;
        }
        if frame_has_forces(frame) {
            let fk_flag = flag_key(FLAG_HAS_FORCES, fk);
            self.idx_flags.put(wtxn, &fk_flag[..], &())?;
            if let Some(fm) = frame_fmax(frame) {
                if let Some(fk_fm) = fmax_bin_key(fm, fk) {
                    self.idx_fmax.put(wtxn, &fk_fm[..], &())?;
                }
            }
        }
        if frame_has_velocities(frame) {
            let fk_flag = flag_key(FLAG_HAS_VELOCITIES, fk);
            self.idx_flags.put(wtxn, &fk_flag[..], &())?;
        }
        Ok(())
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
        let mut iter = ConFrameIterator::new(file_contents);
        loop {
            let (frame, blob) = match iter.next_with_raw_span(file_contents) {
                None => break,
                Some(Err(e)) => return Err(Error::Parse(e.to_string())),
                Some(Ok(x)) => x,
            };
            let fk = FrameKey {
                traj_id,
                frame_idx,
            };
            self.index_frame(&mut wtxn, fk, &frame, blob)?;
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

    /// Ingest already-parsed frames (chemfiles / builder path). Serializes with `ConFrameWriter`.
    pub fn append_trajectory_frames(
        &self,
        traj_id: TrajId,
        frames: &[ConFrame],
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
        for fr in frames {
            let mut buf = Cursor::new(Vec::new());
            {
                let mut w = ConFrameWriter::new(&mut buf);
                w.write_frame(fr)
                    .map_err(|e| Error::Parse(format!("serialize: {e}")))?;
            }
            let blob = String::from_utf8(buf.into_inner())
                .map_err(|e| Error::Message(format!("utf8: {e}")))?;
            let fk = FrameKey {
                traj_id,
                frame_idx,
            };
            self.index_frame(&mut wtxn, fk, fr, &blob)?;
            frame_idx += 1;
        }
        let meta = TrajMeta {
            n_frames: frame_idx,
            source,
        };
        self.traj_meta
            .put(&mut wtxn, &tid_key[..], &serde_json::to_string(&meta)?)?;
        wtxn.commit()?;
        Ok(frame_idx)
    }

    /// Append frames to an existing trajectory (or create it). Returns new total frame count.
    pub fn extend_trajectory_frames(
        &self,
        traj_id: TrajId,
        frames: &[ConFrame],
        source_hint: impl Into<String>,
    ) -> Result<u32> {
        let mut wtxn = self.env.write_txn()?;
        let tid_key = traj_id.to_be_bytes();
        let (mut frame_idx, source) = if let Some(existing) = self.traj_meta.get(&wtxn, &tid_key[..])? {
            let meta: TrajMeta = serde_json::from_str(existing)?;
            (meta.n_frames, meta.source)
        } else {
            (0u32, source_hint.into())
        };
        for fr in frames {
            let mut buf = Cursor::new(Vec::new());
            {
                let mut w = ConFrameWriter::new(&mut buf);
                w.write_frame(fr)
                    .map_err(|e| Error::Parse(format!("serialize: {e}")))?;
            }
            let blob = String::from_utf8(buf.into_inner())
                .map_err(|e| Error::Message(format!("utf8: {e}")))?;
            let fk = FrameKey {
                traj_id,
                frame_idx,
            };
            self.index_frame(&mut wtxn, fk, fr, &blob)?;
            frame_idx += 1;
        }
        let meta = TrajMeta {
            n_frames: frame_idx,
            source,
        };
        self.traj_meta
            .put(&mut wtxn, &tid_key[..], &serde_json::to_string(&meta)?)?;
        wtxn.commit()?;
        Ok(frame_idx)
    }

    fn clear_secondary(&self, wtxn: &mut RwTxn) -> Result<()> {
        self.idx_natoms.clear(wtxn)?;
        self.idx_symbol.clear(wtxn)?;
        self.idx_energy.clear(wtxn)?;
        self.idx_flags.clear(wtxn)?;
        self.idx_elem_count.clear(wtxn)?;
        self.idx_formula.clear(wtxn)?;
        self.idx_fmax.clear(wtxn)?;
        self.frame_by_hash.clear(wtxn)?;
        self.hash_by_frame.clear(wtxn)?;
        Ok(())
    }

    /// Rebuild all secondary indexes from authoritative `frames` blobs (schema upgrade path).
    pub fn reindex(&self) -> Result<u32> {
        let mut wtxn = self.env.write_txn()?;
        self.clear_secondary(&mut wtxn)?;

        let mut n = 0u32;
        // Collect keys first (heed iterator + puts on same txn is safer with snapshot list)
        let mut keys: Vec<Vec<u8>> = Vec::new();
        {
            let mut iter = self.frames.iter(&wtxn)?;
            while let Some(Ok((k, _))) = iter.next() {
                keys.push(k.to_vec());
            }
        }
        for k in keys {
            let Some(fk) = FrameKey::from_bytes(&k) else {
                continue;
            };
            let Some(blob) = self.frames.get(&wtxn, &k)? else {
                continue;
            };
            let blob_owned = blob.to_owned();
            let mut it = ConFrameIterator::new(&blob_owned);
            let frame = it
                .next()
                .ok_or_else(|| Error::Parse("empty blob on reindex".into()))?
                .map_err(|e| Error::Parse(e.to_string()))?;
            // index_frame would put frames again — only indexes + hashes
            let hash = hash_frame_bytes(blob_owned.as_bytes());
            let hb = hash.to_bytes();
            let fk_b = fk.to_bytes();
            self.hash_by_frame.put(&mut wtxn, &fk_b[..], &hb[..])?;
            if self.frame_by_hash.get(&wtxn, &hb[..])?.is_none() {
                self.frame_by_hash.put(&mut wtxn, &hb[..], &fk_b[..])?;
            }
            let n_atoms = frame.atom_data.len() as u32;
            self.idx_natoms
                .put(&mut wtxn, &natoms_key(n_atoms, fk)[..], &())?;
            let counts = frame_species(&frame);
            for (sym, cnt) in &counts {
                self.idx_elem_count
                    .put(&mut wtxn, &elem_count_key(sym, *cnt, fk)[..], &())?;
                self.idx_symbol
                    .put(&mut wtxn, &symbol_key(sym, fk)[..], &())?;
            }
            let formula = composition_formula(&counts);
            if !formula.is_empty() {
                self.idx_formula
                    .put(&mut wtxn, &formula_key(&formula, fk)[..], &())?;
            }
            if let Some(e) = frame_energy(&frame) {
                if let Some(ek) = energy_bin_key(e, fk) {
                    self.idx_energy.put(&mut wtxn, &ek[..], &())?;
                }
                self.idx_flags
                    .put(&mut wtxn, &flag_key(FLAG_HAS_ENERGY, fk)[..], &())?;
            }
            if frame_has_forces(&frame) {
                self.idx_flags
                    .put(&mut wtxn, &flag_key(FLAG_HAS_FORCES, fk)[..], &())?;
                if let Some(fm) = frame_fmax(&frame) {
                    if let Some(fk_fm) = fmax_bin_key(fm, fk) {
                        self.idx_fmax.put(&mut wtxn, &fk_fm[..], &())?;
                    }
                }
            }
            if frame_has_velocities(&frame) {
                self.idx_flags
                    .put(&mut wtxn, &flag_key(FLAG_HAS_VELOCITIES, fk)[..], &())?;
            }
            n += 1;
        }
        wtxn.commit()?;
        Ok(n)
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

    /// Canonical formula for a stored frame (decode once).
    pub fn frame_formula(&self, key: FrameKey) -> Result<String> {
        let fr = self.get_frame(key)?;
        Ok(composition_formula(&frame_species(&fr)))
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

        if let Some(ref formula) = sel.exact_formula {
            let mut s = BTreeSet::new();
            let pref = formula_prefix(formula);
            let mut iter = self.idx_formula.prefix_iter(&rtxn, &pref)?;
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

        for (sym, exact_c) in &sel.element_count_exact {
            let mut s = BTreeSet::new();
            let pref = elem_count_symbol_prefix(sym);
            let mut iter = self.idx_elem_count.prefix_iter(&rtxn, &pref)?;
            while let Some(Ok((k, _))) = iter.next() {
                if let Some((c, fk)) = parse_elem_count_key(k, sym) {
                    if c == *exact_c && sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                        s.insert(fk);
                    }
                }
            }
            sets.push(s);
        }

        for (sym, min_c) in &sel.element_count_min {
            let mut s = BTreeSet::new();
            let pref = elem_count_symbol_prefix(sym);
            let mut iter = self.idx_elem_count.prefix_iter(&rtxn, &pref)?;
            while let Some(Ok((k, _))) = iter.next() {
                if let Some((c, fk)) = parse_elem_count_key(k, sym) {
                    if c >= *min_c && sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                        s.insert(fk);
                    }
                }
            }
            sets.push(s);
        }

        if sel.energy_min.is_some() || sel.energy_max.is_some() {
            let lo_e = sel.energy_min.unwrap_or(f64::NEG_INFINITY);
            let hi_e = sel.energy_max.unwrap_or(f64::INFINITY);
            let lo_bits = ordered_f64_bits(lo_e).unwrap_or(0);
            let hi_bits = ordered_f64_bits(hi_e).unwrap_or(u64::MAX);
            let mut s = BTreeSet::new();
            let mut iter = self.idx_energy.iter(&rtxn)?;
            while let Some(Ok((k, _))) = iter.next() {
                if k.len() < 20 {
                    continue;
                }
                let mut eb = [0u8; 8];
                eb.copy_from_slice(&k[..8]);
                let bits = u64::from_be_bytes(eb);
                if bits > hi_bits {
                    break;
                }
                if bits >= lo_bits {
                    if let Some(fk) = FrameKey::from_bytes(&k[8..20]) {
                        if sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                            s.insert(fk);
                        }
                    }
                }
            }
            sets.push(s);
        }

        if sel.fmax_min.is_some() || sel.fmax_max.is_some() {
            let lo_e = sel.fmax_min.unwrap_or(0.0);
            let hi_e = sel.fmax_max.unwrap_or(f64::INFINITY);
            let lo_bits = ordered_f64_bits(lo_e).unwrap_or(0);
            let hi_bits = ordered_f64_bits(hi_e).unwrap_or(u64::MAX);
            let mut s = BTreeSet::new();
            let mut iter = self.idx_fmax.iter(&rtxn)?;
            while let Some(Ok((k, _))) = iter.next() {
                if k.len() < 20 {
                    continue;
                }
                let mut eb = [0u8; 8];
                eb.copy_from_slice(&k[..8]);
                let bits = u64::from_be_bytes(eb);
                if bits > hi_bits {
                    break;
                }
                if bits >= lo_bits {
                    if let Some(fk) = FrameKey::from_bytes(&k[8..20]) {
                        if sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                            s.insert(fk);
                        }
                    }
                }
            }
            sets.push(s);
        }

        let mut push_flag = |flag: u8| -> Result<()> {
            let mut s = BTreeSet::new();
            let pref = [flag];
            let mut iter = self.idx_flags.prefix_iter(&rtxn, &pref)?;
            while let Some(Ok((k, _))) = iter.next() {
                if k.len() < 13 {
                    continue;
                }
                if let Some(fk) = FrameKey::from_bytes(&k[1..13]) {
                    if sel.traj_id.is_none_or(|t| t == fk.traj_id) {
                        s.insert(fk);
                    }
                }
            }
            sets.push(s);
            Ok(())
        };
        if sel.require_forces {
            push_flag(FLAG_HAS_FORCES)?;
        }
        if sel.require_velocities {
            push_flag(FLAG_HAS_VELOCITIES)?;
        }
        if sel.require_energy {
            push_flag(FLAG_HAS_ENERGY)?;
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
        let ingested = db.ingest_directory(fixtures_dir(), 1).unwrap();
        assert!(ingested.len() >= 3);

        let keys = db
            .select(
                &Select::new()
                    .require_symbol("Cu")
                    .natoms_range(1, 10_000)
                    .limit(50),
            )
            .unwrap();
        assert!(!keys.is_empty());

        let uniq = db
            .unique_frame_keys(&Select::new().require_symbol("Cu"))
            .unwrap();
        assert!(!uniq.is_empty());

        let xyz = dir.path().join("train_subset.xyz");
        let n = db.export_extxyz(&uniq, &xyz, "energy").unwrap();
        assert_eq!(n, uniq.len());
        let text = std::fs::read_to_string(&xyz).unwrap();
        assert!(text.contains("Lattice="));
        assert!(text.contains("Properties="));
        assert!(text.lines().any(|l| l.trim_start().starts_with("Cu ")));
    }

    #[test]
    fn workflow_dedup_identical_ingest() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        let f = fixture("tiny_cuh2.con");
        db.append_trajectory_path(1, &f).unwrap();
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
        assert_eq!(db.find_by_hash(h1).unwrap(), Some(k1));
        let uniq = db.unique_frame_keys(&Select::new()).unwrap();
        assert!(uniq.contains(&k1));
        assert!(!uniq.contains(&k2));
    }

    #[test]
    fn metadata_indexes_forces_velocities_energy() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        db.append_trajectory_path(2, fixture("tiny_cuh2_forces.con"))
            .unwrap();
        db.append_trajectory_path(3, fixture("tiny_cuh2.convel"))
            .unwrap();
        db.append_trajectory_path(4, fixture("tiny_cuh2_vel_forces.con"))
            .unwrap();

        let forces = db.select(&Select::new().require_forces()).unwrap();
        assert!(forces.iter().any(|k| k.traj_id == 2));
        assert!(forces.iter().any(|k| k.traj_id == 4));
        assert!(!forces.iter().any(|k| k.traj_id == 1));

        let vels = db.select(&Select::new().require_velocities()).unwrap();
        assert!(vels.iter().any(|k| k.traj_id == 3));
        assert!(vels.iter().any(|k| k.traj_id == 4));

        let both = db
            .select(&Select::new().require_forces().require_velocities())
            .unwrap();
        assert!(both.iter().any(|k| k.traj_id == 4));
        assert!(!both.iter().any(|k| k.traj_id == 2));

        let with_e = db.select(&Select::new().require_energy()).unwrap();
        assert!(with_e.iter().any(|k| k.traj_id == 2));
        assert!(with_e.iter().any(|k| k.traj_id == 4));
        assert!(!with_e.iter().any(|k| k.traj_id == 1));

        let in_range = db
            .select(&Select::new().energy_range(-43.0, -42.0).require_forces())
            .unwrap();
        assert!(in_range.iter().any(|k| k.traj_id == 2));
        let miss = db.select(&Select::new().energy_range(0.0, 1.0)).unwrap();
        assert!(miss.is_empty());
    }

    #[test]
    fn composition_and_fmax_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        db.append_trajectory_path(2, fixture("tiny_cuh2_forces.con"))
            .unwrap();
        db.append_trajectory_path(3, fixture("sulfolene.con"))
            .unwrap();

        let formula = db
            .frame_formula(FrameKey {
                traj_id: 1,
                frame_idx: 0,
            })
            .unwrap();
        assert_eq!(formula, "Cu:2|H:2");

        let exact = db
            .select(&Select::new().exact_composition("Cu:2|H:2"))
            .unwrap();
        assert!(exact.iter().any(|k| k.traj_id == 1));
        assert!(exact.iter().any(|k| k.traj_id == 2));
        assert!(!exact.iter().any(|k| k.traj_id == 3));

        let cu2 = db
            .select(&Select::new().element_exact("Cu", 2).element_exact("H", 2))
            .unwrap();
        assert_eq!(cu2.len(), exact.len());

        let cu_min2 = db.select(&Select::new().element_min("Cu", 2)).unwrap();
        assert!(cu_min2.iter().any(|k| k.traj_id == 1));

        let wrong = db
            .select(&Select::new().exact_composition("Fe:1"))
            .unwrap();
        assert!(wrong.is_empty());

        let fr_f = db
            .get_frame(FrameKey {
                traj_id: 2,
                frame_idx: 0,
            })
            .unwrap();
        let expected_fmax = frame_fmax(&fr_f).expect("forces fixture has fmax");
        let in_fmax = db
            .select(
                &Select::new()
                    .require_forces()
                    .fmax_range(0.0, expected_fmax + 1e-6),
            )
            .unwrap();
        assert!(in_fmax.iter().any(|k| k.traj_id == 2));
        assert!(!in_fmax.iter().any(|k| k.traj_id == 1));

        let too_small = db
            .select(&Select::new().fmax_range(0.0, expected_fmax * 0.0 - 1.0))
            .unwrap();
        // negative max excludes all positive fmax postings that are > hi; lo>hi may be empty
        let impossible = db
            .select(&Select::new().fmax_range(1e9, 1e9 + 1.0))
            .unwrap();
        assert!(impossible.is_empty());
        let _ = too_small;
    }

    #[test]
    fn reindex_and_append_frames() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        db.append_trajectory_path(2, fixture("tiny_cuh2_forces.con"))
            .unwrap();

        let before = db
            .select(&Select::new().exact_composition("Cu:2|H:2"))
            .unwrap();
        assert!(!before.is_empty());

        let n = db.reindex().unwrap();
        assert!(n >= 2);
        let after = db
            .select(&Select::new().exact_composition("Cu:2|H:2"))
            .unwrap();
        assert_eq!(before, after);

        let fr = db
            .get_frame(FrameKey {
                traj_id: 1,
                frame_idx: 0,
            })
            .unwrap();
        let n2 = db
            .append_trajectory_frames(10, &[fr.clone()], "mem")
            .unwrap();
        assert_eq!(n2, 1);
        let mem = db
            .select(
                &Select::new()
                    .trajectory(10)
                    .exact_composition("Cu:2|H:2"),
            )
            .unwrap();
        assert_eq!(mem.len(), 1);

        let n3 = db.extend_trajectory_frames(10, &[fr], "mem-ext").unwrap();
        assert_eq!(n3, 2);
    }

    #[test]
    fn reindex_twice_stable() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2_forces.con"))
            .unwrap();
        let a = db.reindex().unwrap();
        let keys1 = db.select(&Select::new().require_forces()).unwrap();
        let b = db.reindex().unwrap();
        let keys2 = db.select(&Select::new().require_forces()).unwrap();
        assert_eq!(a, b);
        assert_eq!(keys1, keys2);
    }
}
