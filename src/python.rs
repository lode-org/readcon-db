use std::sync::Arc;

use pyo3::exceptions::{PyIndexError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};

use crate::canonical::AtomOrderHit;
use crate::corpus::ConCorpus;
use crate::keys::{ContentHash, FrameKey};
use crate::select::Select;

/// One LMDB corpus directory (`data.mdb` and `lock.mdb`).
#[pyclass(name = "ConCorpus")]
struct PyConCorpus {
    inner: ConCorpus,
}

#[pymethods]
impl PyConCorpus {
    #[new]
    #[pyo3(signature = (path, readonly=false))]
    /// Open `path`. `readonly=True` refuses writes.
    fn new(path: &str, readonly: bool) -> PyResult<Self> {
        let inner = if readonly {
            ConCorpus::open_readonly(path)
        } else {
            ConCorpus::open(path)
        }
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    /// RCSO bytes for a unidirectional broadcast on the *caller* communicator.
    /// Rank 0 packs; workers call :func:`unpack_positions` on the copy.
    fn pack_frame(&self, traj_id: u64, frame_idx: u32) -> PyResult<Vec<u8>> {
        self.inner
            .pack_frame(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Many RCSO blobs in one RCSB envelope (one Bcast on the caller comm).
    fn pack_frames(&self, keys: Vec<(u64, u32)>) -> PyResult<Vec<u8>> {
        let fks: Vec<FrameKey> = keys
            .into_iter()
            .map(|(traj_id, frame_idx)| FrameKey { traj_id, frame_idx })
            .collect();
        self.inner
            .pack_frames(&fks)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[staticmethod]
    /// Positions from an RCSB batch packed by `pack_frames`, one list per frame.
    fn unpack_batch(buf: Vec<u8>) -> PyResult<Vec<Vec<(f64, f64, f64)>>> {
        let parts = crate::cooked_soa::decode_batch(&buf)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let mut out = Vec::with_capacity(parts.len());
        for p in parts {
            let cooked = crate::cooked_soa::CookedSoa::decode(&p)
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            out.push(
                cooked
                    .positions
                    .into_iter()
                    .map(|r| (r[0], r[1], r[2]))
                    .collect(),
            );
        }
        Ok(out)
    }

    /// Cooked H5MD interchange (h5py). CON text stays authority in the corpus.
    /// One `[T][N][3]` dataset per trajectory. Fixed N only.
    ///
    /// `velocity_time_unit` overrides D10. Pass `fs` when a v1, v2, or v3.0
    /// file really stored Angstrom/fs, or the natural unit
    /// `(amu*angstrom^2/eV)^0.5` when a v3.1 file mislabeled that unit as `fs`.
    /// The same string is the clock unit when a v3.1 frame has `time` or
    /// `timestep` and no `units.time`. A present `units.time` still scales
    /// the clock. v1, v2, and v3.0 clocks with no `units.time` are `fs`.
    #[pyo3(signature = (traj_id, path, velocity_time_unit=None))]
    fn export_h5md(
        &self,
        py: Python<'_>,
        traj_id: u64,
        path: &str,
        velocity_time_unit: Option<String>,
    ) -> PyResult<u32> {
        if std::path::Path::new(path).exists() {
            return Err(PyRuntimeError::new_err(format!(
                "export_h5md dest exists: {path}"
            )));
        }
        let h5py = py
            .import("h5py")
            .map_err(|_| PyRuntimeError::new_err("export_h5md requires h5py"))?;
        let a = self
            .inner
            .collect_h5md_with(
                traj_id,
                &crate::export_h5md::H5mdCollectOpts { velocity_time_unit },
            )
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let n_frames = a.n_frames;
        let natoms = a.natoms;
        let file = h5py.call_method1("File", (path, "x"))?;
        let written = (|| -> PyResult<u32> {
            let np = py.import("numpy")?;
            let put =
                |obj: &Bound<'_, PyAny>, name: &str, data: Bound<'_, PyAny>| -> PyResult<()> {
                    let d = PyDict::new(obj.py());
                    d.set_item("data", data)?;
                    obj.call_method("create_dataset", (name,), Some(&d))?;
                    Ok(())
                };
            let ascii_attr =
                |obj: &Bound<'_, PyAny>, name: &str, val: &str, n: usize| -> PyResult<()> {
                    let dt = h5py.call_method("string_dtype", ("ascii", n), None)?;
                    let kw = PyDict::new(obj.py());
                    kw.set_item("dtype", dt)?;
                    obj.getattr("attrs")?
                        .call_method("create", (name, val), Some(&kw))?;
                    Ok(())
                };
            // Physical `unit` is an H5MD string (a few chars). Store it as a
            // Python str so MDAnalysis 2.10 can index `_unit_translation`
            // (fixed ASCII `S` dtypes come back as `bytes` and KeyError).
            // author/creator/boundary stay fixed ASCII below.
            let unit_attr = |obj: &Bound<'_, PyAny>, name: &str, val: &str| -> PyResult<()> {
                obj.getattr("attrs")?.set_item(name, val)?;
                Ok(())
            };
            let write_td = |parent: &Bound<'_, PyAny>,
                            name: &str,
                            value: Bound<'_, PyAny>,
                            step: Bound<'_, PyAny>,
                            time: Bound<'_, PyAny>,
                            unit: &str,
                            tunit: &str|
             -> PyResult<()> {
                let g = parent.call_method1("create_group", (name,))?;
                put(&g, "value", value)?;
                put(&g, "step", step)?;
                put(&g, "time", time)?;
                let val = g.call_method1("__getitem__", ("value",))?;
                unit_attr(&val, "unit", unit)?;
                let tm = g.call_method1("__getitem__", ("time",))?;
                unit_attr(&tm, "unit", tunit)?;
                Ok(())
            };
            let h5md = file.call_method1("create_group", ("h5md",))?;
            let h5md_attrs = h5md.getattr("attrs")?;
            h5md_attrs.set_item("version", (1i32, 1i32))?;
            let author = h5md.call_method1("create_group", ("author",))?;
            ascii_attr(&author, "name", "readcon-db", 32)?;
            let creator = h5md.call_method1("create_group", ("creator",))?;
            ascii_attr(&creator, "name", "readcon-db", 32)?;
            ascii_attr(&creator, "version", env!("CARGO_PKG_VERSION"), 16)?;
            let particles = file.call_method1("create_group", ("particles",))?;
            let all = particles.call_method1("create_group", ("all",))?;
            let boxg = all.call_method1("create_group", ("box",))?;
            let attrs = boxg.getattr("attrs")?;
            attrs.set_item("dimension", 3)?;
            let dt8 = h5py.call_method("string_dtype", ("ascii", 8), None)?;
            let bnd_kw = PyDict::new(py);
            bnd_kw.set_item("dtype", dt8)?;
            let bnd = np.call_method(
                "array",
                ((
                    a.boundary[0].as_str(),
                    a.boundary[1].as_str(),
                    a.boundary[2].as_str(),
                ),),
                Some(&bnd_kw),
            )?;
            attrs.call_method("create", ("boundary", bnd), None)?;
            let dtype_kw = PyDict::new(py);
            dtype_kw.set_item("dtype", "float64")?;
            let i64_kw = PyDict::new(py);
            i64_kw.set_item("dtype", "int64")?;
            let i32_kw = PyDict::new(py);
            i32_kw.set_item("dtype", "int32")?;
            let step = np.call_method("arange", (n_frames as i64,), Some(&i64_kw))?;
            let time = np.call_method("asarray", (a.times,), Some(&dtype_kw))?;
            let pos_arr = np
                .call_method("asarray", (a.positions,), Some(&dtype_kw))?
                .call_method1("reshape", ((n_frames, natoms, 3),))?;
            write_td(
                &all,
                "position",
                pos_arr,
                step.clone(),
                time.clone(),
                a.length_unit.as_str(),
                a.time_unit.as_str(),
            )?;
            let edges_arr = np
                .call_method("asarray", (a.edges,), Some(&dtype_kw))?
                .call_method1("reshape", ((n_frames, 3, 3),))?;
            write_td(
                &boxg,
                "edges",
                edges_arr,
                step.clone(),
                time.clone(),
                a.length_unit.as_str(),
                a.time_unit.as_str(),
            )?;
            if let Some(fbuf) = a.forces {
                let f_arr = np
                    .call_method("asarray", (fbuf,), Some(&dtype_kw))?
                    .call_method1("reshape", ((n_frames, natoms, 3),))?;
                write_td(
                    &all,
                    "force",
                    f_arr,
                    step.clone(),
                    time.clone(),
                    a.force_unit.as_str(),
                    a.time_unit.as_str(),
                )?;
            }
            if let Some(vbuf) = a.velocities {
                let v_arr = np
                    .call_method("asarray", (vbuf,), Some(&dtype_kw))?
                    .call_method1("reshape", ((n_frames, natoms, 3),))?;
                write_td(
                    &all,
                    "velocity",
                    v_arr,
                    step,
                    time,
                    a.velocity_unit.as_str(),
                    a.time_unit.as_str(),
                )?;
            }
            let spec_arr = np.call_method("asarray", (a.species_z,), Some(&i32_kw))?;
            put(&all, "species", spec_arr)?;
            Ok(n_frames as u32)
        })();
        let _ = file.call_method0("close");
        if written.is_err() {
            let _ = std::fs::remove_file(path);
        }
        written
    }

    #[staticmethod]
    /// Positions from an RCSO blob packed by `pack_frame`.
    fn unpack_positions(buf: Vec<u8>) -> PyResult<Vec<(f64, f64, f64)>> {
        let cooked = crate::cooked_soa::CookedSoa::decode(&buf)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(cooked
            .positions
            .into_iter()
            .map(|p| (p[0], p[1], p[2]))
            .collect())
    }

    #[pyo3(signature = (traj_id, path, units=None))]
    /// Ingest a UTF-8 CON path as a new trajectory. Gzip and zstd are not decompressed.
    fn append_trajectory(
        &self,
        traj_id: u64,
        path: &str,
        units: Option<Bound<'_, PyDict>>,
    ) -> PyResult<u32> {
        let u = units_from_pydict(units)?;
        self.inner
            .append_trajectory_path_units(traj_id, path, u)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Ingest CON text without a temp file.
    #[pyo3(signature = (traj_id, text, source="memory"))]
    fn append_trajectory_str(&self, traj_id: u64, text: &str, source: &str) -> PyResult<u32> {
        self.inner
            .append_trajectory_str(traj_id, text, source)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Ingest already-parsed `readcon` frames, committing 64 frames at a time.
    ///
    /// The first group is `append_trajectory_str`. Each later group is
    /// `extend_trajectory_str`. A `read_con` result is written from the stored
    /// frames, so this does not build a Python frame per index. The return
    /// value is the number of frames in the trajectory after the last commit.
    #[pyo3(signature = (traj_id, frames, source="memory"))]
    fn append_trajectory_frames(
        &self,
        py: Python<'_>,
        traj_id: u64,
        frames: Bound<'_, PyAny>,
        source: &str,
    ) -> PyResult<u32> {
        let readcon = py
            .import("readcon")
            .map_err(|e| PyRuntimeError::new_err(format!("readcon required: {e}")))?;
        let mut total = 0u32;
        let mut started = false;
        let flush = |text: &str, started: bool| -> PyResult<u32> {
            let result = if started {
                self.inner.extend_trajectory_str(traj_id, text, source)
            } else {
                self.inner.append_trajectory_str(traj_id, text, source)
            };
            result.map_err(|e| PyRuntimeError::new_err(e.to_string()))
        };
        // A sequence is serialized in place, 64 frames at a time. Iterating a
        // `read_con` result builds a Python frame per index and keeps it.
        if let Ok(n) = frames.len() {
            if n == 0 {
                return Ok(0);
            }
            let mut start = 0usize;
            while start < n {
                let stop = (start + 64).min(n);
                let kwargs = PyDict::new(py);
                kwargs.set_item("start", start)?;
                kwargs.set_item("stop", stop)?;
                let text: String = readcon
                    .call_method("write_con_string", (frames.clone(),), Some(&kwargs))
                    .and_then(|v| v.extract())
                    .map_err(|e| PyRuntimeError::new_err(format!("write_con_string: {e}")))?;
                total = flush(&text, started)?;
                started = true;
                start = stop;
            }
            return Ok(total);
        }
        let mut chunk: Vec<Py<PyAny>> = Vec::new();
        for item in frames.try_iter()? {
            chunk.push(item?.unbind());
            if chunk.len() == 64 {
                let list = PyList::new(py, chunk.iter().map(|item| item.bind(py)))?;
                let text: String = readcon
                    .call_method1("write_con_string", (list,))
                    .and_then(|v| v.extract())
                    .map_err(|e| PyRuntimeError::new_err(format!("write_con_string: {e}")))?;
                total = flush(&text, started)?;
                started = true;
                chunk.clear();
            }
        }
        if !chunk.is_empty() {
            let list = PyList::new(py, chunk.iter().map(|item| item.bind(py)))?;
            let text: String = readcon
                .call_method1("write_con_string", (list,))
                .and_then(|v| v.extract())
                .map_err(|e| PyRuntimeError::new_err(format!("write_con_string: {e}")))?;
            total = flush(&text, started)?;
        }
        Ok(total)
    }

    /// Write caller units onto every frame of `traj_id` (canonical names in CON text).
    ///
    /// A v3.1 frame with a clock or velocities and no `units.time` is an
    /// error. The missing time unit is not treated as `fs`. v1, v2, and
    /// v3.0 clocks with no `units.time` still convert as `fs`.
    fn set_units(&self, traj_id: u64, units: Bound<'_, PyDict>) -> PyResult<u32> {
        let u = units_from_pydict(Some(units))?
            .ok_or_else(|| PyRuntimeError::new_err("units required"))?;
        self.inner
            .set_trajectory_units(traj_id, u)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Units JSON stored on one frame, or `None` when the frame has no units object.
    fn get_units(&self, traj_id: u64, frame_idx: u32) -> PyResult<Option<String>> {
        let v = self
            .inner
            .frame_units(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(v.map(|x| x.to_string()))
    }

    /// Append a UTF-8 CON path to a trajectory, or create it when it is absent.
    /// Gzip and zstd are not decompressed.
    #[pyo3(signature = (traj_id, path, units=None))]
    fn extend_trajectory(
        &self,
        traj_id: u64,
        path: &str,
        units: Option<Bound<'_, PyDict>>,
    ) -> PyResult<u32> {
        let u = units_from_pydict(units)?;
        self.inner
            .extend_trajectory_path_units(traj_id, path, u)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[pyo3(signature = (
        traj_id=None,
        symbol=None,
        natoms_min=0,
        natoms_max=u32::MAX,
        exact_hash=None,
        energy_min=None,
        energy_max=None,
        fmax_min=None,
        fmax_max=None,
        mass_min=None,
        mass_max=None,
        volume_min=None,
        volume_max=None,
        frame_index_min=None,
        frame_index_max=None,
        charge_min=None,
        charge_max=None,
        element_exact=None,
        element_min=None,
        formula=None,
        require_forces=false,
        require_velocities=false,
        require_energy=false,
        limit=None
    ))]
    /// Return `(traj_id, frame_idx)` pairs that match every predicate.
    ///
    /// The default atom-count window is the full `u32` domain. That window
    /// is not a predicate and does not walk `idx_natoms`. A narrower
    /// `natoms_min` or `natoms_max` does.
    fn select(
        &self,
        traj_id: Option<u64>,
        symbol: Option<String>,
        natoms_min: u32,
        natoms_max: u32,
        exact_hash: Option<Vec<u8>>,
        energy_min: Option<f64>,
        energy_max: Option<f64>,
        fmax_min: Option<f64>,
        fmax_max: Option<f64>,
        mass_min: Option<f64>,
        mass_max: Option<f64>,
        volume_min: Option<f64>,
        volume_max: Option<f64>,
        frame_index_min: Option<f64>,
        frame_index_max: Option<f64>,
        charge_min: Option<f64>,
        charge_max: Option<f64>,
        element_exact: Option<Vec<(String, u32)>>,
        element_min: Option<Vec<(String, u32)>>,
        formula: Option<String>,
        require_forces: bool,
        require_velocities: bool,
        require_energy: bool,
        limit: Option<usize>,
    ) -> PyResult<Vec<(u64, u32)>> {
        let mut sel = Select::new().natoms_range(natoms_min, natoms_max);
        if let Some(t) = traj_id {
            sel = sel.trajectory(t);
        }
        if let Some(s) = symbol {
            sel = sel.require_symbol(s);
        }
        if let Some(h) = exact_hash {
            if h.len() != 16 {
                return Err(PyRuntimeError::new_err("exact_hash must be 16 bytes"));
            }
            let mut a = [0u8; 16];
            a.copy_from_slice(&h);
            sel = sel.exact_hash(a);
        }
        if energy_min.is_some() || energy_max.is_some() {
            sel = sel.energy_range(
                energy_min.unwrap_or(f64::NEG_INFINITY),
                energy_max.unwrap_or(f64::INFINITY),
            );
        }
        if fmax_min.is_some() || fmax_max.is_some() {
            sel = sel.fmax_range(fmax_min.unwrap_or(0.0), fmax_max.unwrap_or(f64::INFINITY));
        }
        if mass_min.is_some() || mass_max.is_some() {
            sel = sel.mass_range(
                mass_min.unwrap_or(f64::NEG_INFINITY),
                mass_max.unwrap_or(f64::INFINITY),
            );
        }
        if volume_min.is_some() || volume_max.is_some() {
            sel = sel.volume_range(
                volume_min.unwrap_or(f64::NEG_INFINITY),
                volume_max.unwrap_or(f64::INFINITY),
            );
        }
        if frame_index_min.is_some() || frame_index_max.is_some() {
            sel = sel.frame_index_range(
                frame_index_min.unwrap_or(f64::NEG_INFINITY),
                frame_index_max.unwrap_or(f64::INFINITY),
            );
        }
        if charge_min.is_some() || charge_max.is_some() {
            sel = sel.charge_range(
                charge_min.unwrap_or(f64::NEG_INFINITY),
                charge_max.unwrap_or(f64::INFINITY),
            );
        }
        if let Some(pairs) = element_exact {
            for (sym, c) in pairs {
                sel = sel.element_exact(sym, c);
            }
        }
        if let Some(pairs) = element_min {
            for (sym, c) in pairs {
                sel = sel.element_min(sym, c);
            }
        }
        if let Some(f) = formula {
            sel = sel.exact_composition(f);
        }
        if require_forces {
            sel = sel.require_forces();
        }
        if require_velocities {
            sel = sel.require_velocities();
        }
        if require_energy {
            sel = sel.require_energy();
        }
        if let Some(n) = limit {
            sel = sel.limit(n);
        }
        let keys = self
            .inner
            .select(&sel)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(keys.into_iter().map(|k| (k.traj_id, k.frame_idx)).collect())
    }

    /// Rebuild the secondary indexes from the stored CON text. Returns the frame count.
    fn reindex(&self) -> PyResult<u32> {
        self.inner
            .reindex()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Composition string of one frame, for example `Cu:2|H:2`.
    fn frame_formula(&self, traj_id: u64, frame_idx: u32) -> PyResult<String> {
        self.inner
            .frame_formula(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Opt-in RCSO cook; CON text in `frames` remains authority.
    fn cook_frame(&self, traj_id: u64, frame_idx: u32) -> PyResult<usize> {
        self.inner
            .cook_frame(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Delete the cooked numeric copy of one frame. The CON text stays.
    fn delete_cooked_soa(&self, traj_id: u64, frame_idx: u32) -> PyResult<()> {
        self.inner
            .delete_cooked_soa(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// True when a cooked numeric copy of this frame is stored and decodes.
    fn has_valid_cooked_soa(&self, traj_id: u64, frame_idx: u32) -> PyResult<bool> {
        self.inner
            .has_valid_cooked_soa(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Rewrite the cooked numeric copy of every frame. Returns the frame count.
    fn recook_all(&self) -> PyResult<u32> {
        self.inner
            .recook_all()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Prefer frames_soa; fallback parse CON. List of (x,y,z).
    fn get_positions(&self, traj_id: u64, frame_idx: u32) -> PyResult<Vec<(f64, f64, f64)>> {
        let v = self
            .inner
            .get_positions(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(v.into_iter().map(|r| (r[0], r[1], r[2])).collect())
    }

    /// Forces of one frame, or `None` when that frame has no forces.
    fn get_forces(&self, traj_id: u64, frame_idx: u32) -> PyResult<Option<Vec<(f64, f64, f64)>>> {
        let v = self
            .inner
            .get_forces(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(v.map(|rows| rows.into_iter().map(|r| (r[0], r[1], r[2])).collect()))
    }

    /// Velocities of one frame, or `None` when that frame has no velocities.
    fn get_velocities(
        &self,
        traj_id: u64,
        frame_idx: u32,
    ) -> PyResult<Option<Vec<(f64, f64, f64)>>> {
        let v = self
            .inner
            .get_velocities(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(v.map(|rows| rows.into_iter().map(|r| (r[0], r[1], r[2])).collect()))
    }

    /// xxHash3-128 of the stored CON text, 16 bytes, little-endian.
    fn frame_hash(&self, traj_id: u64, frame_idx: u32) -> PyResult<Vec<u8>> {
        let h = self
            .inner
            .frame_hash(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(h.to_bytes().to_vec())
    }

    /// Build the atom-id canonical index. Returns the number of frames indexed.
    ///
    /// The xxHash3 content hash is unchanged. Run this again after the stored
    /// CON text is rewritten.
    fn index_canonical_identity(&self) -> PyResult<u32> {
        self.inner
            .index_canonical_identity()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Recomputed canonical identity of one stored frame: `(version, hash)`.
    ///
    /// `hash` is 16 bytes, little-endian. This does not read the index.
    fn frame_canonical_identity(&self, traj_id: u64, frame_idx: u32) -> PyResult<(u8, Vec<u8>)> {
        let ident = self
            .inner
            .frame_canonical_identity(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok((ident.version, ident.hash.to_bytes().to_vec()))
    }

    /// Stored frames that match `text` up to atom-id row order.
    fn find_by_atom_order(&self, text: &str) -> PyResult<Vec<PyAtomOrderHit>> {
        let hits = self
            .inner
            .find_by_atom_order_text(text)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(hits.into_iter().map(PyAtomOrderHit::from_hit).collect())
    }

    /// [`find_by_atom_order`](Self::find_by_atom_order) for a CON path.
    fn find_by_atom_order_path(&self, path: &str) -> PyResult<Vec<PyAtomOrderHit>> {
        let hits = self
            .inner
            .find_by_atom_order_path(path)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(hits.into_iter().map(PyAtomOrderHit::from_hit).collect())
    }

    /// First `(traj_id, frame_idx)` stored under this 16-byte hash.
    fn find_by_hash(&self, hash: Vec<u8>) -> PyResult<Option<(u64, u32)>> {
        if hash.len() != 16 {
            return Err(PyRuntimeError::new_err("hash must be 16 bytes"));
        }
        let mut a = [0u8; 16];
        a.copy_from_slice(&hash);
        let h = ContentHash(a);
        Ok(self
            .inner
            .find_by_hash(h)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
            .map(|k| (k.traj_id, k.frame_idx)))
    }

    /// Bulk columns for `traj_id`, or one frame when `frame_idx` is set.
    ///
    /// Positions, forces, masses, and cell lengths are NumPy arrays.
    /// An uncompressed span is parsed from the LMDB mapping. The scan
    /// releases the GIL and does not build a Python atom per site.
    #[pyo3(signature = (traj_id, frame_idx=None))]
    fn read_columns(
        &self,
        py: Python<'_>,
        traj_id: u64,
        frame_idx: Option<u32>,
    ) -> PyResult<Py<PyDict>> {
        let cols = py
            .detach(|| {
                let result = match frame_idx {
                    Some(frame_idx) => self
                        .inner
                        .read_frame_columns(FrameKey { traj_id, frame_idx }),
                    None => self.inner.read_columns(traj_id),
                };
                result.map_err(|e| e.to_string())
            })
            .map_err(PyRuntimeError::new_err)?;
        Ok(columns_to_py(py, &cols)?.unbind())
    }

    /// CON text of one stored frame.
    fn get_frame_text(&self, traj_id: u64, frame_idx: u32) -> PyResult<String> {
        self.inner
            .get_frame_text(FrameKey { traj_id, frame_idx })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Random-access parse of one stored frame.
    ///
    /// The frame is parsed in Rust. `atoms[i]` materialises that atom only,
    /// and `xyz` is the coordinate rows. This does not build a Python atom
    /// for every site and does not call `read_con_string`.
    fn get_frame(&self, py: Python<'_>, traj_id: u64, frame_idx: u32) -> PyResult<StoredFrame> {
        let frame = py
            .detach(|| {
                self.inner
                    .get_frame(crate::keys::FrameKey { traj_id, frame_idx })
                    .map_err(|e| e.to_string())
            })
            .map_err(PyRuntimeError::new_err)?;
        let rows = frame
            .atom_data
            .iter()
            .map(|atom| {
                (
                    atom.symbol.to_string(),
                    crate::atom_coord::atom_x(atom),
                    crate::atom_coord::atom_y(atom),
                    crate::atom_coord::atom_z(atom),
                )
            })
            .collect();
        Ok(StoredFrame {
            rows: Arc::new(rows),
            cell: frame.header.boxl,
            angles: frame.header.angles,
        })
    }

    /// Materialize all frame blobs for `traj_id` in one LMDB read txn (full extract).
    /// Returns `(total_bytes, payload_checksum)` so callers cannot elide the copy.
    fn touch_trajectory(&self, traj_id: u64, n_frames: u32) -> PyResult<(u64, u64)> {
        self.inner
            .touch_trajectory_blobs(traj_id, n_frames)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// CON text of each `(traj_id, frame_idx)` key, in order.
    fn get_frame_texts(&self, keys: Vec<(u64, u32)>) -> PyResult<Vec<String>> {
        let fks: Vec<FrameKey> = keys
            .into_iter()
            .map(|(t, f)| FrameKey {
                traj_id: t,
                frame_idx: f,
            })
            .collect();
        self.inner
            .get_frame_texts(&fks)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[pyo3(signature = (keys, path, energy_key=None))]
    /// Write the selected frames as extended XYZ. The destination must not exist.
    fn export_extxyz(
        &self,
        keys: Vec<(u64, u32)>,
        path: &str,
        energy_key: Option<String>,
    ) -> PyResult<usize> {
        let ek = energy_key.unwrap_or_else(|| "energy".into());
        let fks: Vec<FrameKey> = keys
            .into_iter()
            .map(|(t, f)| FrameKey {
                traj_id: t,
                frame_idx: f,
            })
            .collect();
        self.inner
            .export_extxyz(&fks, path, &ek)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[pyo3(signature = (dir, start_traj_id=None, units=None))]
    /// Ingest every `.con` and `.convel` file in `dir`, numbered from `start_traj_id`.
    fn ingest_directory(
        &self,
        dir: &str,
        start_traj_id: Option<u64>,
        units: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Vec<(u64, u32, String)>> {
        let start = start_traj_id.unwrap_or(1);
        let u = units_from_pydict(units)?;
        self.inner
            .ingest_directory_units(dir, start, u)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[staticmethod]
    /// xxHash3-128 of `data`, 16 bytes, little-endian.
    fn xxh3_128(data: Vec<u8>) -> Vec<u8> {
        crate::keys::hash_frame_bytes(&data).to_bytes().to_vec()
    }
}

/// Unidirectional RCSO broadcast on the caller's mpi4py communicator.
///
/// `comm` is an ``mpi4py.MPI.Comm`` (LAMMPS world/subcomm, ``Comm.Split``,
/// ``Comm.Dup``). This function never names the process-wide world handle
/// and never calls ``MPI_Init`` — mpi4py and LAMMPS already started MPI.
///
/// Rank ``root`` opens the corpus read-only and packs; every other rank
/// on ``comm`` receives the bytes and never opens LMDB.
#[pyfunction]
#[pyo3(signature = (comm, corpus_dir, traj_id, frame_idx, root=0))]
fn bcast_packed_frame(
    comm: Bound<'_, PyAny>,
    corpus_dir: &str,
    traj_id: u64,
    frame_idx: u32,
    root: i32,
) -> PyResult<Vec<u8>> {
    let rank: i32 = comm.call_method0("Get_rank")?.extract()?;
    let payload: (Option<String>, Option<Vec<u8>>) = if rank == root {
        match ConCorpus::open_readonly(corpus_dir) {
            Ok(db) => match db.pack_frame(FrameKey { traj_id, frame_idx }) {
                Ok(bytes) => {
                    db.close();
                    (None, Some(bytes))
                }
                Err(e) => {
                    db.close();
                    (Some(e.to_string()), None)
                }
            },
            Err(e) => (Some(e.to_string()), None),
        }
    } else {
        (None, None)
    };
    let got = comm.call_method1("bcast", (payload, root))?;
    let (err, blob): (Option<String>, Option<Vec<u8>>) = got.extract()?;
    if let Some(msg) = err {
        return Err(PyRuntimeError::new_err(msg));
    }
    blob.ok_or_else(|| PyRuntimeError::new_err("empty pack on root"))
}

/// Many frames, one collective on the caller mpi4py comm.
#[pyfunction]
#[pyo3(signature = (comm, corpus_dir, keys, root=0))]
fn bcast_packed_frames(
    comm: Bound<'_, PyAny>,
    corpus_dir: &str,
    keys: Vec<(u64, u32)>,
    root: i32,
) -> PyResult<Vec<u8>> {
    let rank: i32 = comm.call_method0("Get_rank")?.extract()?;
    let payload: (Option<String>, Option<Vec<u8>>) = if rank == root {
        match ConCorpus::open_readonly(corpus_dir) {
            Ok(db) => {
                let fks: Vec<FrameKey> = keys
                    .into_iter()
                    .map(|(traj_id, frame_idx)| FrameKey { traj_id, frame_idx })
                    .collect();
                match db.pack_frames(&fks) {
                    Ok(bytes) => {
                        db.close();
                        (None, Some(bytes))
                    }
                    Err(e) => {
                        db.close();
                        (Some(e.to_string()), None)
                    }
                }
            }
            Err(e) => (Some(e.to_string()), None),
        }
    } else {
        (None, None)
    };
    let got = comm.call_method1("bcast", (payload, root))?;
    let (err, blob): (Option<String>, Option<Vec<u8>>) = got.extract()?;
    if let Some(msg) = err {
        return Err(PyRuntimeError::new_err(msg));
    }
    blob.ok_or_else(|| PyRuntimeError::new_err("empty pack on root"))
}

fn f64_array<'py>(py: Python<'py>, data: &[f64], shape: &[usize]) -> PyResult<Bound<'py, PyAny>> {
    let np = py
        .import("numpy")
        .map_err(|_| PyRuntimeError::new_err("read_columns requires numpy"))?;
    let raw = unsafe {
        std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), std::mem::size_of_val(data))
    };
    let buf = PyBytes::new(py, raw);
    let kwargs = PyDict::new(py);
    kwargs.set_item("dtype", "float64")?;
    let flat = np.call_method("frombuffer", (buf,), Some(&kwargs))?;
    let shape_t = PyTuple::new(py, shape.iter().copied())?;
    flat.call_method1("reshape", (shape_t,))
}

fn columns_to_py<'py>(
    py: Python<'py>,
    cols: &crate::span_columns::Columns,
) -> PyResult<Bound<'py, PyDict>> {
    let n_frames = cols.n_atoms.len();
    let same = cols.n_atoms.first().copied().unwrap_or(0);
    let rectangular = cols.n_atoms.iter().all(|n| *n == same);
    let n_atoms = same as usize;
    let pos_shape: Vec<usize> = if rectangular {
        vec![n_frames, n_atoms, 3]
    } else {
        vec![cols.positions.len() / 3, 3]
    };
    let out = PyDict::new(py);
    out.set_item("n_frames", n_frames)?;
    out.set_item("n_atoms", PyList::new(py, cols.n_atoms.iter().copied())?)?;
    out.set_item("positions", f64_array(py, &cols.positions, &pos_shape)?)?;
    if cols.forces_uniform && cols.forces.len() == cols.positions.len() {
        out.set_item("forces", f64_array(py, &cols.forces, &pos_shape)?)?;
    } else {
        out.set_item("forces", py.None())?;
    }
    let mass_shape: Vec<usize> = if rectangular {
        vec![n_frames, n_atoms]
    } else {
        vec![cols.masses.len()]
    };
    out.set_item("masses", f64_array(py, &cols.masses, &mass_shape)?)?;
    out.set_item("cell", f64_array(py, &cols.cells, &[n_frames, 6])?)?;
    out.set_item("energy", f64_array(py, &cols.energy, &[n_frames])?)?;
    out.set_item(
        "energy_set",
        PyList::new(py, cols.energy_set.iter().copied())?,
    )?;
    out.set_item(
        "frame_index",
        f64_array(py, &cols.frame_index, &[n_frames])?,
    )?;
    out.set_item(
        "frame_index_set",
        PyList::new(py, cols.frame_index_set.iter().copied())?,
    )?;
    let pbc = PyList::empty(py);
    for flag in cols.pbc.chunks(3) {
        pbc.append(PyTuple::new(py, flag.iter().copied())?)?;
    }
    out.set_item("pbc", pbc)?;
    out.set_item("pbc_set", PyList::new(py, cols.pbc_set.iter().copied())?)?;
    let runs = PyList::empty(py);
    for frame in &cols.symbol_runs {
        let one = PyList::empty(py);
        for (sym, n) in frame {
            one.append((sym.as_str(), *n))?;
        }
        runs.append(one)?;
    }
    out.set_item("symbol_runs", runs)?;
    Ok(out)
}

fn units_from_pydict(units: Option<Bound<'_, PyDict>>) -> PyResult<Option<serde_json::Value>> {
    let Some(d) = units else {
        return Ok(None);
    };
    let mut map = serde_json::Map::new();
    for (k, v) in d.iter() {
        let key: String = k.extract()?;
        let val: String = v.extract()?;
        map.insert(key, serde_json::Value::String(val));
    }
    Ok(Some(serde_json::Value::Object(map)))
}

/// Metatomic-style conversion: `value_to = factor * value_from`.
#[pyfunction]
fn unit_conversion_factor(from_unit: &str, to_unit: &str) -> PyResult<f64> {
    readcon_core::units::unit_conversion_factor(from_unit, to_unit)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
/// Canonical name for one unit expression. Aliases such as `A` and `ev` are accepted.
fn canonicalize_unit(expr: &str) -> PyResult<String> {
    crate::units_canon::canonicalize_unit(expr).map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

/// One atom built by indexing `StoredAtoms`.
#[pyclass(name = "StoredAtom")]
struct StoredAtom {
    #[pyo3(get)]
    symbol: String,
    #[pyo3(get)]
    x: f64,
    #[pyo3(get)]
    y: f64,
    #[pyo3(get)]
    z: f64,
}

/// Atoms of a stored frame. Indexing builds that one atom.
#[pyclass(name = "StoredAtoms")]
struct StoredAtoms {
    rows: Arc<Vec<(String, f64, f64, f64)>>,
}

#[pymethods]
impl StoredAtoms {
    /// Number of atoms.
    fn __len__(&self) -> usize {
        self.rows.len()
    }

    /// Build the atom at `index`. A negative index counts from the end.
    fn __getitem__(&self, index: isize) -> PyResult<StoredAtom> {
        let n = self.rows.len() as isize;
        let resolved = if index < 0 { n + index } else { index };
        if resolved < 0 || resolved >= n {
            return Err(PyIndexError::new_err("atom index out of range"));
        }
        let (symbol, x, y, z) = &self.rows[resolved as usize];
        Ok(StoredAtom {
            symbol: symbol.clone(),
            x: *x,
            y: *y,
            z: *z,
        })
    }
}

/// A stored frame that matches a query up to atom-id row order.
///
/// `permutation[i]` is the index in the query of the atom whose `atom_id`
/// equals stored atom `i`.
#[pyclass(name = "AtomOrderHit")]
struct PyAtomOrderHit {
    query_index: u32,
    traj_id: u64,
    frame_idx: u32,
    permutation: Vec<usize>,
}

#[pymethods]
impl PyAtomOrderHit {
    #[getter]
    /// Zero-based index of the query frame in the text or file.
    fn query_index(&self) -> u32 {
        self.query_index
    }

    #[getter]
    /// Trajectory id of the stored frame.
    fn traj_id(&self) -> u64 {
        self.traj_id
    }

    #[getter]
    /// Frame index of the stored frame.
    fn frame_idx(&self) -> u32 {
        self.frame_idx
    }

    #[getter]
    /// Index in the query of each stored atom, in stored order.
    fn permutation(&self) -> Vec<usize> {
        self.permutation.clone()
    }
}

impl PyAtomOrderHit {
    fn from_hit(hit: AtomOrderHit) -> Self {
        Self {
            query_index: hit.query_index,
            traj_id: hit.key.traj_id,
            frame_idx: hit.key.frame_idx,
            permutation: hit.permutation,
        }
    }
}

/// One stored frame.
///
/// `xyz` is every coordinate row. `atoms[i]` builds that atom only.
/// `cell` is the three box lengths and `angles` is the three box angles.
/// This is not a `readcon.ConFrame`. `get_frame_text` plus
/// `readcon.read_con_string` still returns one.
#[pyclass(name = "StoredFrame")]
struct StoredFrame {
    rows: Arc<Vec<(String, f64, f64, f64)>>,
    cell: [f64; 3],
    angles: [f64; 3],
}

#[pymethods]
impl StoredFrame {
    #[getter]
    /// Coordinate rows `(x, y, z)` for every atom.
    fn xyz(&self) -> Vec<(f64, f64, f64)> {
        self.rows.iter().map(|(_, x, y, z)| (*x, *y, *z)).collect()
    }

    #[getter]
    /// Atom list. Indexing builds that atom only.
    fn atoms(&self) -> StoredAtoms {
        StoredAtoms {
            rows: Arc::clone(&self.rows),
        }
    }

    #[getter]
    /// Three box lengths.
    fn cell(&self) -> [f64; 3] {
        self.cell
    }

    #[getter]
    /// Three box angles, in degrees.
    fn angles(&self) -> [f64; 3] {
        self.angles
    }

    /// Number of atoms.
    fn __len__(&self) -> usize {
        self.rows.len()
    }
}

/// LMDB corpus of CON frames. Parsing and writing CON text is `readcon`.
#[pymodule]
fn readcon_db(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConCorpus>()?;
    m.add_class::<PyAtomOrderHit>()?;
    m.add_class::<StoredFrame>()?;
    m.add_class::<StoredAtom>()?;
    m.add_class::<StoredAtoms>()?;
    m.add_function(wrap_pyfunction!(bcast_packed_frame, m)?)?;
    m.add_function(wrap_pyfunction!(bcast_packed_frames, m)?)?;
    m.add_function(wrap_pyfunction!(unit_conversion_factor, m)?)?;
    m.add_function(wrap_pyfunction!(canonicalize_unit, m)?)?;
    Ok(())
}
