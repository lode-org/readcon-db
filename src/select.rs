use crate::keys::TrajId;

/// Non-SQL selection builder (ASE.db-competitive campaign filters via secondary indexes).
#[derive(Clone, Debug, Default)]
pub struct Select {
    pub traj_id: Option<TrajId>,
    pub natoms_min: Option<u32>,
    pub natoms_max: Option<u32>,
    pub symbols_all: Vec<String>,
    pub content_hash: Option<[u8; 16]>,
    pub energy_min: Option<f64>,
    pub energy_max: Option<f64>,
    pub fmax_min: Option<f64>,
    pub fmax_max: Option<f64>,
    pub mass_min: Option<f64>,
    pub mass_max: Option<f64>,
    pub volume_min: Option<f64>,
    pub volume_max: Option<f64>,
    /// Exact PBC mask from metadata (`pbc`); frames without `pbc` key never match.
    pub pbc_exact: Option<[bool; 3]>,
    pub time_min: Option<f64>,
    pub time_max: Option<f64>,
    pub timestep_min: Option<f64>,
    pub timestep_max: Option<f64>,
    pub frame_index_min: Option<f64>,
    pub frame_index_max: Option<f64>,
    pub neb_bead_min: Option<f64>,
    pub neb_bead_max: Option<f64>,
    pub neb_band_min: Option<f64>,
    pub neb_band_max: Option<f64>,
    pub charge_min: Option<f64>,
    pub charge_max: Option<f64>,
    pub magmom_min: Option<f64>,
    pub magmom_max: Option<f64>,
    pub element_count_min: Vec<(String, u32)>,
    pub element_count_exact: Vec<(String, u32)>,
    pub exact_formula: Option<String>,
    pub require_forces: bool,
    pub require_velocities: bool,
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
    pub fn energy_range(mut self, min: f64, max: f64) -> Self {
        self.energy_min = Some(min);
        self.energy_max = Some(max);
        self
    }
    pub fn fmax_range(mut self, min: f64, max: f64) -> Self {
        self.fmax_min = Some(min);
        self.fmax_max = Some(max);
        self
    }
    pub fn mass_range(mut self, min: f64, max: f64) -> Self {
        self.mass_min = Some(min);
        self.mass_max = Some(max);
        self
    }
    pub fn volume_range(mut self, min: f64, max: f64) -> Self {
        self.volume_min = Some(min);
        self.volume_max = Some(max);
        self
    }
    pub fn pbc(mut self, xyz: [bool; 3]) -> Self {
        self.pbc_exact = Some(xyz);
        self
    }
    pub fn time_range(mut self, min: f64, max: f64) -> Self {
        self.time_min = Some(min);
        self.time_max = Some(max);
        self
    }
    pub fn timestep_range(mut self, min: f64, max: f64) -> Self {
        self.timestep_min = Some(min);
        self.timestep_max = Some(max);
        self
    }
    pub fn frame_index_range(mut self, min: f64, max: f64) -> Self {
        self.frame_index_min = Some(min);
        self.frame_index_max = Some(max);
        self
    }
    pub fn neb_bead_range(mut self, min: f64, max: f64) -> Self {
        self.neb_bead_min = Some(min);
        self.neb_bead_max = Some(max);
        self
    }
    pub fn neb_band_range(mut self, min: f64, max: f64) -> Self {
        self.neb_band_min = Some(min);
        self.neb_band_max = Some(max);
        self
    }
    pub fn charge_range(mut self, min: f64, max: f64) -> Self {
        self.charge_min = Some(min);
        self.charge_max = Some(max);
        self
    }
    pub fn magmom_range(mut self, min: f64, max: f64) -> Self {
        self.magmom_min = Some(min);
        self.magmom_max = Some(max);
        self
    }
    pub fn element_min(mut self, symbol: impl Into<String>, count: u32) -> Self {
        self.element_count_min.push((symbol.into(), count));
        self
    }
    pub fn element_exact(mut self, symbol: impl Into<String>, count: u32) -> Self {
        self.element_count_exact.push((symbol.into(), count));
        self
    }
    pub fn exact_composition(mut self, formula: impl Into<String>) -> Self {
        self.exact_formula = Some(formula.into());
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
