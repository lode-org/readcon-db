//! ASE.db-competitive screening scalars — thin wrappers over
//! [`readcon_core::index_proj`] so CON semantics live in one crate.

use readcon_core::index_proj::{self, FrameIndexProjection};
use readcon_core::types::ConFrame;

pub use index_proj::{
    finite_energy, frame_cell_volume, frame_composition_formula, frame_fmax, frame_total_mass,
    sections_present_mask, FrameIndexProjection as CoreFrameIndexProjection, SECTIONS_MASK_ENERGIES,
    SECTIONS_MASK_FORCES, SECTIONS_MASK_VELOCITIES,
};

/// Full index projection (preferred entry for prepare/reindex).
pub fn project_frame(frame: &ConFrame) -> FrameIndexProjection {
    FrameIndexProjection::from_frame(frame)
}

/// Explicit PBC from metadata; None if key absent (not indexed → range/match cannot succeed).
pub fn frame_pbc_mask(frame: &ConFrame) -> Option<u8> {
    let p = frame.header.pbc()?;
    Some(crate::keys::pbc_mask_from_bools(p))
}

pub fn frame_time(frame: &ConFrame) -> Option<f64> {
    project_frame(frame).time
}

pub fn frame_timestep(frame: &ConFrame) -> Option<f64> {
    project_frame(frame).timestep
}

pub fn frame_frame_index(frame: &ConFrame) -> Option<f64> {
    project_frame(frame).frame_index
}

pub fn frame_neb_bead(frame: &ConFrame) -> Option<f64> {
    project_frame(frame).neb_bead
}

pub fn frame_neb_band(frame: &ConFrame) -> Option<f64> {
    project_frame(frame).neb_band
}

pub fn frame_charge(frame: &ConFrame) -> Option<f64> {
    project_frame(frame).charge
}

pub fn frame_magmom(frame: &ConFrame) -> Option<f64> {
    project_frame(frame).magmom
}

#[cfg(test)]
mod tests {
    use super::*;
    use readcon_core::iterators::ConFrameIterator;
    use std::path::PathBuf;

    #[test]
    fn mass_and_volume_from_fixture() {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test/tiny_cuh2.con");
        let text = std::fs::read_to_string(p).unwrap();
        let fr = ConFrameIterator::new(&text).next().unwrap().unwrap();
        let m = frame_total_mass(&fr);
        assert!(m.is_some_and(|x| x > 0.0));
        let v = frame_cell_volume(&fr).unwrap();
        assert!(v > 0.0);
        // orthogonal cell: a*b*c
        let [a, b, c] = fr.header.boxl;
        let expected = a * b * c;
        assert!((v - expected).abs() < 1e-4 * expected.max(1.0));
    }
}
