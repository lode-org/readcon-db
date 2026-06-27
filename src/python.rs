use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

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

    /// Returns list of (traj_id, frame_idx).
    #[pyo3(signature = (traj_id=None, symbol=None, natoms_min=0, natoms_max=u32::MAX, exact_hash=None, limit=None))]
    fn select(
        &self,
        traj_id: Option<u64>,
        symbol: Option<String>,
        natoms_min: u32,
        natoms_max: u32,
        exact_hash: Option<Vec<u8>>,
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

    fn frame_hash(&self, traj_id: u64, frame_idx: u32) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let h = self
                .inner
                .frame_hash(FrameKey {
                    traj_id,
                    frame_idx,
                })
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            Ok(PyBytes::new(py, &h.to_bytes()).into())
        })
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

    /// Write selected frames to extended XYZ (metatrain / ASE).
    /// `keys` is a list of (traj_id, frame_idx).
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

    fn ingest_directory(&self, dir: &str, start_traj_id: Option<u64>) -> PyResult<Vec<(u64, u32, String)>> {
        let start = start_traj_id.unwrap_or(1);
        self.inner
            .ingest_directory(dir, start)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    #[staticmethod]
    fn xxh3_128(data: &[u8]) -> PyObject {
        Python::with_gil(|py| {
            let h = crate::keys::hash_frame_bytes(data);
            PyBytes::new(py, &h.to_bytes()).into()
        })
    }
}

#[pymodule]
fn readcon_db(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConCorpus>()?;
    Ok(())
}
