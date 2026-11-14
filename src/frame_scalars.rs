//! ASE.db-competitive screening scalars — thin wrappers over
//! [`readcon_core::index_proj`] so CON semantics live in one crate.

pub use readcon_core::index_proj::{frame_cell_volume, frame_total_mass};

#[cfg(test)]
mod tests {
    use super::*;
    use readcon_core::iterators::ConFrameIterator;
    use std::path::PathBuf;

    #[test]
    fn mass_and_volume_from_fixture() {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test/tiny_cuh2.con");
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
