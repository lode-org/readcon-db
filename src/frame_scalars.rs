//! Derive ASE.db-competitive screening scalars from CON `ConFrame` (authoritative blob elsewhere).

use readcon_core::types::ConFrame;

/// Total mass = Σ masses_per_type[i] * natms_per_type[i] (finite only).
pub fn frame_total_mass(frame: &ConFrame) -> Option<f64> {
    let h = &frame.header;
    if h.masses_per_type.is_empty() || h.natms_per_type.is_empty() {
        return None;
    }
    let n = h.masses_per_type.len().min(h.natms_per_type.len());
    let mut m = 0.0f64;
    for i in 0..n {
        let mi = h.masses_per_type[i];
        let ni = h.natms_per_type[i] as f64;
        if !mi.is_finite() || !ni.is_finite() {
            return None;
        }
        m += mi * ni;
    }
    m.is_finite().then_some(m)
}

/// Cell volume: prefer `lattice_vectors` determinant; else triclinic from `boxl` + `angles` (degrees).
pub fn frame_cell_volume(frame: &ConFrame) -> Option<f64> {
    if let Some(lv) = frame.header.lattice_vectors() {
        let det = scalar_triple(lv[0], lv[1], lv[2]).abs();
        return det.is_finite().then_some(det);
    }
    let [a, b, c] = frame.header.boxl;
    let [alpha, beta, gamma] = frame.header.angles;
    if ![a, b, c, alpha, beta, gamma].iter().all(|x| x.is_finite() && *x > 0.0) {
        return None;
    }
    let ar = alpha.to_radians();
    let br = beta.to_radians();
    let gr = gamma.to_radians();
    let ca = ar.cos();
    let cb = br.cos();
    let cg = gr.cos();
    let sg = gr.sin();
    if sg.abs() < 1e-15 {
        return None;
    }
    // V = abc * sqrt(1 - cos²α - cos²β - cos²γ + 2 cosα cosβ cosγ)
    let t = 1.0 - ca * ca - cb * cb - cg * cg + 2.0 * ca * cb * cg;
    if t <= 0.0 {
        return None;
    }
    let v = a * b * c * t.sqrt();
    v.is_finite().then_some(v)
}

fn scalar_triple(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0])
}

/// Explicit PBC from metadata; None if key absent (not indexed → range/match cannot succeed).
pub fn frame_pbc_mask(frame: &ConFrame) -> Option<u8> {
    let p = frame.header.pbc()?;
    Some(crate::keys::pbc_mask_from_bools(p))
}

fn meta_f64(frame: &ConFrame, key: &str) -> Option<f64> {
    let v = frame.header.metadata.get(key)?;
    if let Some(f) = v.as_f64() {
        return f.is_finite().then_some(f);
    }
    if let Some(i) = v.as_i64() {
        let f = i as f64;
        return f.is_finite().then_some(f);
    }
    if let Some(u) = v.as_u64() {
        let f = u as f64;
        return f.is_finite().then_some(f);
    }
    None
}

pub fn frame_time(frame: &ConFrame) -> Option<f64> {
    frame.header.time().filter(|t| t.is_finite()).or_else(|| meta_f64(frame, "time"))
}

pub fn frame_timestep(frame: &ConFrame) -> Option<f64> {
    frame
        .header
        .timestep()
        .filter(|t| t.is_finite())
        .or_else(|| meta_f64(frame, "timestep"))
}

pub fn frame_frame_index(frame: &ConFrame) -> Option<f64> {
    frame
        .header
        .frame_index()
        .map(|i| i as f64)
        .filter(|t| t.is_finite())
        .or_else(|| meta_f64(frame, "frame_index"))
}

pub fn frame_neb_bead(frame: &ConFrame) -> Option<f64> {
    frame
        .header
        .neb_bead()
        .map(|i| i as f64)
        .filter(|t| t.is_finite())
        .or_else(|| meta_f64(frame, "neb_bead"))
}

pub fn frame_neb_band(frame: &ConFrame) -> Option<f64> {
    meta_f64(frame, "neb_band").or_else(|| {
        frame
            .header
            .metadata
            .get("neb_band")
            .and_then(|v| v.as_u64())
            .map(|u| u as f64)
            .filter(|t| t.is_finite())
    })
}

pub fn frame_charge(frame: &ConFrame) -> Option<f64> {
    meta_f64(frame, "charge")
}

pub fn frame_magmom(frame: &ConFrame) -> Option<f64> {
    meta_f64(frame, "magmom")
}

#[cfg(test)]
mod tests {
    use super::*;
    use readcon_core::iterators::ConFrameIterator;
    use std::path::PathBuf;

    #[test]
    fn mass_and_volume_from_fixture() {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../readcon-core/resources/test/tiny_cuh2.con");
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
