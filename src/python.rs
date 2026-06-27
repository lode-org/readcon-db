use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

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
    fn new(path: &str) -> PyResult<Self> {
        ConCorpus::open(path)
            .map(|inner| Self { inner })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn append_trajectory(&self, traj_id: u64, path: &str) -> PyResult<u32> {
        self.inner
            .append_trajectory_path(traj_id, path)
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

#[pymodule]
fn readcon_db(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConCorpus>()?;
    Ok(())
}
