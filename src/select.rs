use crate::keys::TrajId;

/// Non-SQL selection builder (ASE.db-competitive campaign filters via secondary indexes).
#[derive(Clone, Debug, Default)]
pub struct Select {
    /// When set, only this trajectory is scanned.
    pub traj_id: Option<TrajId>,
    /// Inclusive lower bound on the atom count.
    pub natoms_min: Option<u32>,
    /// Inclusive upper bound on the atom count.
    pub natoms_max: Option<u32>,
    /// Element symbols that must all be present.
    pub symbols_all: Vec<String>,
    /// Exact xxHash3-128 of the CON text, little-endian.
    pub content_hash: Option<[u8; 16]>,
    /// Inclusive lower bound on a finite frame energy.
    pub energy_min: Option<f64>,
    /// Inclusive upper bound on a finite frame energy.
    pub energy_max: Option<f64>,
    /// Inclusive lower bound on the maximum force magnitude.
    pub fmax_min: Option<f64>,
    /// Inclusive upper bound on the maximum force magnitude.
    pub fmax_max: Option<f64>,
    /// Inclusive lower bound on the total mass.
    pub mass_min: Option<f64>,
    /// Inclusive upper bound on the total mass.
    pub mass_max: Option<f64>,
    /// Inclusive lower bound on the cell volume.
    pub volume_min: Option<f64>,
    /// Inclusive upper bound on the cell volume.
    pub volume_max: Option<f64>,
    /// Exact PBC mask from metadata (`pbc`); frames without `pbc` key never match.
    pub pbc_exact: Option<[bool; 3]>,
    /// Inclusive lower bound on the frame clock.
    pub time_min: Option<f64>,
    /// Inclusive upper bound on the frame clock.
    pub time_max: Option<f64>,
    /// Inclusive lower bound on the recorded timestep.
    pub timestep_min: Option<f64>,
    /// Inclusive upper bound on the recorded timestep.
    pub timestep_max: Option<f64>,
    /// Inclusive lower bound on the metadata frame index.
    pub frame_index_min: Option<f64>,
    /// Inclusive upper bound on the metadata frame index.
    pub frame_index_max: Option<f64>,
    /// Inclusive lower bound on the NEB bead index.
    pub neb_bead_min: Option<f64>,
    /// Inclusive upper bound on the NEB bead index.
    pub neb_bead_max: Option<f64>,
    /// Inclusive lower bound on the NEB band index.
    pub neb_band_min: Option<f64>,
    /// Inclusive upper bound on the NEB band index.
    pub neb_band_max: Option<f64>,
    /// Inclusive lower bound on the frame charge.
    pub charge_min: Option<f64>,
    /// Inclusive upper bound on the frame charge.
    pub charge_max: Option<f64>,
    /// Inclusive lower bound on the magnetic moment.
    pub magmom_min: Option<f64>,
    /// Inclusive upper bound on the magnetic moment.
    pub magmom_max: Option<f64>,
    /// Each pair requires at least that many atoms of the symbol.
    pub element_count_min: Vec<(String, u32)>,
    /// Each pair requires exactly that many atoms of the symbol.
    pub element_count_exact: Vec<(String, u32)>,
    /// Exact composition, for example `Cu:2|H:2`.
    pub exact_formula: Option<String>,
    /// Bonded-topology HEX from `seams fingerprint` (`idx_topo` prefix).
    pub topo_key: Option<String>,
    /// Keep only frames that store forces.
    pub require_forces: bool,
    /// Keep only frames that store velocities.
    pub require_velocities: bool,
    /// Keep only frames that store a finite energy.
    pub require_energy: bool,
    /// Maximum number of keys to return, after sorting.
    pub limit: Option<usize>,
}

impl Select {
    /// A query with no predicates. It matches every frame.
    pub fn new() -> Self {
        Self::default()
    }
    /// Restrict the query to one trajectory.
    pub fn trajectory(mut self, id: TrajId) -> Self {
        self.traj_id = Some(id);
        self
    }
    /// Keep frames whose atom count is inside `min..=max`.
    pub fn natoms_range(mut self, min: u32, max: u32) -> Self {
        self.natoms_min = Some(min);
        self.natoms_max = Some(max);
        self
    }
    /// Require this element symbol. Call again to require another.
    pub fn require_symbol(mut self, s: impl Into<String>) -> Self {
        self.symbols_all.push(s.into());
        self
    }
    /// Match one xxHash3-128 of the CON text.
    pub fn exact_hash(mut self, hash: [u8; 16]) -> Self {
        self.content_hash = Some(hash);
        self
    }
    /// Keep frames whose finite energy is inside `min..=max`.
    pub fn energy_range(mut self, min: f64, max: f64) -> Self {
        self.energy_min = Some(min);
        self.energy_max = Some(max);
        self
    }
    /// Keep frames whose maximum force is inside `min..=max`.
    pub fn fmax_range(mut self, min: f64, max: f64) -> Self {
        self.fmax_min = Some(min);
        self.fmax_max = Some(max);
        self
    }
    /// Keep frames whose total mass is inside `min..=max`.
    pub fn mass_range(mut self, min: f64, max: f64) -> Self {
        self.mass_min = Some(min);
        self.mass_max = Some(max);
        self
    }
    /// Keep frames whose cell volume is inside `min..=max`.
    pub fn volume_range(mut self, min: f64, max: f64) -> Self {
        self.volume_min = Some(min);
        self.volume_max = Some(max);
        self
    }
    /// Match this exact periodic-boundary mask. Frames with no `pbc` key do not match.
    pub fn pbc(mut self, xyz: [bool; 3]) -> Self {
        self.pbc_exact = Some(xyz);
        self
    }
    /// Keep frames whose clock is inside `min..=max`.
    pub fn time_range(mut self, min: f64, max: f64) -> Self {
        self.time_min = Some(min);
        self.time_max = Some(max);
        self
    }
    /// Keep frames whose timestep is inside `min..=max`.
    pub fn timestep_range(mut self, min: f64, max: f64) -> Self {
        self.timestep_min = Some(min);
        self.timestep_max = Some(max);
        self
    }
    /// Keep frames whose metadata frame index is inside `min..=max`.
    pub fn frame_index_range(mut self, min: f64, max: f64) -> Self {
        self.frame_index_min = Some(min);
        self.frame_index_max = Some(max);
        self
    }
    /// Keep frames whose NEB bead index is inside `min..=max`.
    pub fn neb_bead_range(mut self, min: f64, max: f64) -> Self {
        self.neb_bead_min = Some(min);
        self.neb_bead_max = Some(max);
        self
    }
    /// Keep frames whose NEB band index is inside `min..=max`.
    pub fn neb_band_range(mut self, min: f64, max: f64) -> Self {
        self.neb_band_min = Some(min);
        self.neb_band_max = Some(max);
        self
    }
    /// Keep frames whose charge is inside `min..=max`.
    pub fn charge_range(mut self, min: f64, max: f64) -> Self {
        self.charge_min = Some(min);
        self.charge_max = Some(max);
        self
    }
    /// Keep frames whose magnetic moment is inside `min..=max`.
    pub fn magmom_range(mut self, min: f64, max: f64) -> Self {
        self.magmom_min = Some(min);
        self.magmom_max = Some(max);
        self
    }
    /// Require at least `count` atoms of `symbol`.
    pub fn element_min(mut self, symbol: impl Into<String>, count: u32) -> Self {
        self.element_count_min.push((symbol.into(), count));
        self
    }
    /// Require exactly `count` atoms of `symbol`.
    pub fn element_exact(mut self, symbol: impl Into<String>, count: u32) -> Self {
        self.element_count_exact.push((symbol.into(), count));
        self
    }
    /// Match one composition string, for example `Cu:2|H:2`.
    pub fn exact_composition(mut self, formula: impl Into<String>) -> Self {
        self.exact_formula = Some(formula.into());
        self
    }
    /// Match a bonded-topology key recorded by `annotate_topology`.
    pub fn topo_key(mut self, hex: impl Into<String>) -> Self {
        self.topo_key = Some(hex.into());
        self
    }
    /// Keep only frames that store forces.
    pub fn require_forces(mut self) -> Self {
        self.require_forces = true;
        self
    }
    /// Keep only frames that store velocities.
    pub fn require_velocities(mut self) -> Self {
        self.require_velocities = true;
        self
    }
    /// Keep only frames that store a finite energy.
    pub fn require_energy(mut self) -> Self {
        self.require_energy = true;
        self
    }
    /// Return at most `n` keys.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }
}
