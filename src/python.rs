use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::corpus::ConCorpus;
use crate::keys::{ContentHash, FrameKey};
use crate::select::Select;

#[pyclass(name = "ConCorpus")]
struct PyConCorpus {
    inner: ConCorpus,
}

#[pymethods]
impl PyConCorpus {
    #[new]
    #[pyo3(signature = (path, readonly=false))]
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
            .pack_frame(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Many RCSO blobs in one RCSB envelope (one Bcast on the caller comm).
    fn pack_frames(&self, keys: Vec<(u64, u32)>) -> PyResult<Vec<u8>> {
        let fks: Vec<FrameKey> = keys
            .into_iter()
            .map(|(traj_id, frame_idx)| FrameKey {
                traj_id,
                frame_idx,
            })
            .collect();
        self.inner
            .pack_frames(&fks)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[staticmethod]
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
    fn export_h5md(&self, py: Python<'_>, traj_id: u64, path: &str) -> PyResult<u32> {
        let h5py = py
            .import("h5py")
            .map_err(|_| PyRuntimeError::new_err("export_h5md requires h5py"))?;
        let a = self
            .inner
            .collect_h5md(traj_id)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let n_frames = a.n_frames;
        let natoms = a.natoms;
        let file = h5py.call_method1("File", (path, "w"))?;
        let np = py.import("numpy")?;
        let put = |obj: &Bound<'_, PyAny>, name: &str, data: Bound<'_, PyAny>| -> PyResult<()> {
            let d = PyDict::new(obj.py());
            d.set_item("data", data)?;
            obj.call_method("create_dataset", (name,), Some(&d))?;
            Ok(())
        };
        let ascii_attr = |obj: &Bound<'_, PyAny>, name: &str, val: &str, n: usize| -> PyResult<()> {
            // utf-8 fixed (not VL): H5MD forbids VL; MDA decodes utf-8 to str.
            let dt = h5py.call_method("string_dtype", ("utf-8", n), None)?;
            let kw = PyDict::new(obj.py());
            kw.set_item("dtype", dt)?;
            obj.getattr("attrs")?
                .call_method("create", (name, val), Some(&kw))?;
            Ok(())
        };
        let write_td = |parent: &Bound<'_, PyAny>, name: &str, value: Bound<'_, PyAny>, step: Bound<'_, PyAny>, time: Bound<'_, PyAny>, unit: &str, tunit: &str| -> PyResult<()> {
            let g = parent.call_method1("create_group", (name,))?;
            put(&g, "value", value)?;
            put(&g, "step", step)?;
            put(&g, "time", time)?;
            let val = g.call_method1("__getitem__", ("value",))?;
            ascii_attr(&val, "unit", unit, 32)?;
            let tm = g.call_method1("__getitem__", ("time",))?;
            ascii_attr(&tm, "unit", tunit, 8)?;
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
                step,
                time,
                a.force_unit.as_str(),
                a.time_unit.as_str(),
            )?;
        }
        let spec_arr = np.call_method("asarray", (a.species_z,), Some(&i32_kw))?;
        put(&all, "species", spec_arr)?;
        file.call_method0("close")?;
        Ok(n_frames as u32)
    }

    #[staticmethod]
    fn unpack_positions(buf: Vec<u8>) -> PyResult<Vec<(f64, f64, f64)>> {
        let cooked = crate::cooked_soa::CookedSoa::decode(&buf)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(cooked
            .positions
            .into_iter()
            .map(|p| (p[0], p[1], p[2]))
            .collect())
    }

    fn append_trajectory(&self, traj_id: u64, path: &str) -> PyResult<u32> {
        self.inner
            .append_trajectory_path(traj_id, path)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Append frames to an existing trajectory (or create it).
    fn extend_trajectory(&self, traj_id: u64, path: &str) -> PyResult<u32> {
        self.inner
            .extend_trajectory_path(traj_id, path)
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
            sel = sel.mass_range(mass_min.unwrap_or(f64::NEG_INFINITY), mass_max.unwrap_or(f64::INFINITY));
        }
        if volume_min.is_some() || volume_max.is_some() {
            sel = sel.volume_range(volume_min.unwrap_or(f64::NEG_INFINITY), volume_max.unwrap_or(f64::INFINITY));
        }
        if frame_index_min.is_some() || frame_index_max.is_some() {
            sel = sel.frame_index_range(frame_index_min.unwrap_or(f64::NEG_INFINITY), frame_index_max.unwrap_or(f64::INFINITY));
        }
        if charge_min.is_some() || charge_max.is_some() {
            sel = sel.charge_range(charge_min.unwrap_or(f64::NEG_INFINITY), charge_max.unwrap_or(f64::INFINITY));
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
        Ok(keys
            .into_iter()
            .map(|k| (k.traj_id, k.frame_idx))
            .collect())
    }

    fn reindex(&self) -> PyResult<u32> {
        self.inner
            .reindex()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn frame_formula(&self, traj_id: u64, frame_idx: u32) -> PyResult<String> {
        self.inner
            .frame_formula(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Opt-in RCSO cook; CON text in `frames` remains authority.
    fn cook_frame(&self, traj_id: u64, frame_idx: u32) -> PyResult<usize> {
        self.inner
            .cook_frame(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn delete_cooked_soa(&self, traj_id: u64, frame_idx: u32) -> PyResult<()> {
        self.inner
            .delete_cooked_soa(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn has_valid_cooked_soa(&self, traj_id: u64, frame_idx: u32) -> PyResult<bool> {
        self.inner
            .has_valid_cooked_soa(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn recook_all(&self) -> PyResult<u32> {
        self.inner
            .recook_all()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Prefer frames_soa; fallback parse CON. List of (x,y,z).
    fn get_positions(&self, traj_id: u64, frame_idx: u32) -> PyResult<Vec<(f64, f64, f64)>> {
        let v = self
            .inner
            .get_positions(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(v.into_iter().map(|r| (r[0], r[1], r[2])).collect())
    }

    fn get_forces(&self, traj_id: u64, frame_idx: u32) -> PyResult<Option<Vec<(f64, f64, f64)>>> {
        let v = self
            .inner
            .get_forces(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(v.map(|rows| rows.into_iter().map(|r| (r[0], r[1], r[2])).collect()))
    }

    fn frame_hash(&self, traj_id: u64, frame_idx: u32) -> PyResult<Vec<u8>> {
        let h = self
            .inner
            .frame_hash(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(h.to_bytes().to_vec())
    }

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

    fn get_frame_text(&self, traj_id: u64, frame_idx: u32) -> PyResult<String> {
        self.inner
            .get_frame_text(FrameKey {
                traj_id,
                frame_idx,
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Random-access parse: stored blob → `readcon.ConFrame`.
    fn get_frame<'py>(
        &self,
        py: Python<'py>,
        traj_id: u64,
        frame_idx: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let text = self.get_frame_text(traj_id, frame_idx)?;
        let readcon = py.import("readcon")?;
        let frames = readcon.call_method1("read_con_string", (text,))?;
        frames.get_item(0)
    }

    /// Materialize all frame blobs for `traj_id` in one LMDB read txn (full extract).
    /// Returns `(total_bytes, payload_checksum)` so callers cannot elide the copy.
    fn touch_trajectory(&self, traj_id: u64, n_frames: u32) -> PyResult<(u64, u64)> {
        self.inner
            .touch_trajectory_blobs(traj_id, n_frames)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

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

    #[pyo3(signature = (dir, start_traj_id=None))]
    fn ingest_directory(
        &self,
        dir: &str,
        start_traj_id: Option<u64>,
    ) -> PyResult<Vec<(u64, u32, String)>> {
        let start = start_traj_id.unwrap_or(1);
        self.inner
            .ingest_directory(dir, start)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[staticmethod]
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
            Ok(db) => match db.pack_frame(FrameKey {
                traj_id,
                frame_idx,
            }) {
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
                    .map(|(traj_id, frame_idx)| FrameKey {
                        traj_id,
                        frame_idx,
                    })
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

/// Metatomic-style conversion: `value_to = factor * value_from`.
#[pyfunction]
fn unit_conversion_factor(from_unit: &str, to_unit: &str) -> PyResult<f64> {
    readcon_core::units::unit_conversion_factor(from_unit, to_unit)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pymodule]
fn readcon_db(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConCorpus>()?;
    m.add_function(wrap_pyfunction!(bcast_packed_frame, m)?)?;
    m.add_function(wrap_pyfunction!(bcast_packed_frames, m)?)?;
    m.add_function(wrap_pyfunction!(unit_conversion_factor, m)?)?;
    Ok(())
}
