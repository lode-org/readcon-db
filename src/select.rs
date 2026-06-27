use crate::keys::TrajId;

/// Non-SQL selection builder (filters composed in process, executed via indexes).
#[derive(Clone, Debug, Default)]
pub struct Select {
    pub traj_id: Option<TrajId>,
    pub natoms_min: Option<u32>,
    pub natoms_max: Option<u32>,
    pub symbols_all: Vec<String>,
    /// Exact content match (xxHash3 of stored blob).
    pub content_hash: Option<[u8; 16]>,
    /// Inclusive energy range on `header.energy()` / metadata `energy` (finite only).
    pub energy_min: Option<f64>,
    pub energy_max: Option<f64>,
    /// Require forces section or per-atom force data.
    pub require_forces: bool,
    /// Require velocities section or per-atom velocity data.
    pub require_velocities: bool,
    /// Require finite energy in frame metadata.
    pub require_energy: bool,
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
    pub fn exact_hash(mut self, hash: [u8; 16]) -> Self {
        self.content_hash = Some(hash);
        self
    }
    /// Inclusive energy window (uses ordered `idx_energy` bins).
    pub fn energy_range(mut self, min: f64, max: f64) -> Self {
        self.energy_min = Some(min);
        self.energy_max = Some(max);
        self
    }
    pub fn require_forces(mut self) -> Self {
        self.require_forces = true;
        self
    }
    pub fn require_velocities(mut self) -> Self {
        self.require_velocities = true;
        self
    }
    pub fn require_energy(mut self) -> Self {
        self.require_energy = true;
        self
    }
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }
}
