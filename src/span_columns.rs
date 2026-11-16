//! Columns of a CON span.
//!
//! The bytes are the stored frame (or a decompressed group member). One
//! readcon-core parse fills the columns, so a label, a duplicate section,
//! and a declared-empty `sections` array match that parse.

use crate::error::{Error, Result};

/// One trajectory's columns, frame-major.
#[derive(Debug, Clone)]
pub struct Columns {
    /// Atoms in each frame.
    pub n_atoms: Vec<u32>,
    /// `x y z` for every atom.
    pub positions: Vec<f64>,
    /// `fx fy fz` when every frame has forces. Empty when none do.
    pub forces: Vec<f64>,
    /// True when `forces` is one block per frame.
    pub forces_uniform: bool,
    /// Per-atom masses in position order.
    pub masses: Vec<f64>,
    /// Box lengths then angles, six numbers per frame.
    pub cells: Vec<f64>,
    /// Frame energy. Meaningful where `energy_set` is true.
    pub energy: Vec<f64>,
    /// Whether that frame's metadata has `energy`.
    pub energy_set: Vec<bool>,
    /// Metadata `frame_index`.
    pub frame_index: Vec<f64>,
    /// Whether that frame's metadata has `frame_index`.
    pub frame_index_set: Vec<bool>,
    /// Three flags per frame, packed. Meaningful where `pbc_set` is true.
    pub pbc: Vec<bool>,
    /// Whether that frame's metadata has `pbc`.
    pub pbc_set: Vec<bool>,
    /// `(symbol, count)` runs, one list per frame.
    pub symbol_runs: Vec<Vec<(String, u32)>>,
}

impl Columns {
    pub(crate) fn new() -> Self {
        Self {
            n_atoms: Vec::new(),
            positions: Vec::new(),
            forces: Vec::new(),
            forces_uniform: true,
            masses: Vec::new(),
            cells: Vec::new(),
            energy: Vec::new(),
            energy_set: Vec::new(),
            frame_index: Vec::new(),
            frame_index_set: Vec::new(),
            pbc: Vec::new(),
            pbc_set: Vec::new(),
            symbol_runs: Vec::new(),
        }
    }
}

fn as_str(line: &[u8]) -> Result<&str> {
    std::str::from_utf8(line).map_err(|_| Error::Parse("CON line is not UTF-8".into()))
}

fn meta_of(line: &[u8], cols: &mut Columns) -> Result<()> {
    let text = as_str(line)?.trim();
    if !text.starts_with('{') {
        cols.energy.push(0.0);
        cols.energy_set.push(false);
        cols.frame_index.push(0.0);
        cols.frame_index_set.push(false);
        cols.pbc.extend_from_slice(&[false, false, false]);
        cols.pbc_set.push(false);
        return Ok(());
    }
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| Error::Parse(e.to_string()))?;
    if let Some(e) = v.get("energy").and_then(serde_json::Value::as_f64) {
        cols.energy.push(e);
        cols.energy_set.push(true);
    } else {
        cols.energy.push(0.0);
        cols.energy_set.push(false);
    }
    if let Some(i) = v.get("frame_index").and_then(serde_json::Value::as_f64) {
        cols.frame_index.push(i);
        cols.frame_index_set.push(true);
    } else {
        cols.frame_index.push(0.0);
        cols.frame_index_set.push(false);
    }
    if let Some(arr) = v.get("pbc").and_then(serde_json::Value::as_array) {
        if arr.len() != 3 {
            return Err(Error::Parse("pbc must have three flags".into()));
        }
        for flag in arr {
            let b = flag
                .as_bool()
                .ok_or_else(|| Error::Parse("pbc flag is not a bool".into()))?;
            cols.pbc.push(b);
        }
        cols.pbc_set.push(true);
    } else {
        cols.pbc.extend_from_slice(&[false, false, false]);
        cols.pbc_set.push(false);
    }
    Ok(())
}

/// Append every frame in `bytes` to `cols`.
///
/// Columns come from one readcon-core parse of the whole buffer. A second
/// frame that the parser rejects is not dropped, and a declared section is
/// stored the way that parse stored it.
pub fn append_span(cols: &mut Columns, bytes: &[u8]) -> Result<()> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| Error::Parse("CON line is not UTF-8".into()))?;
    for item in readcon_core::iterators::ConFrameIterator::new(text) {
        let frame = item.map_err(|err| Error::Parse(err.to_string()))?;
        push_frame_columns(cols, &frame)?;
    }
    Ok(())
}

fn push_frame_columns(cols: &mut Columns, frame: &readcon_core::types::ConFrame) -> Result<()> {
    meta_of(frame.header.prebox_header.metadata_line().as_bytes(), cols)?;
    cols.cells.extend_from_slice(&frame.header.boxl);
    cols.cells.extend_from_slice(&frame.header.angles);
    let n = frame.atom_data.len();
    if n == 0 {
        cols.n_atoms.push(0);
        cols.symbol_runs.push(Vec::new());
        cols.forces_uniform = false;
        return Ok(());
    }
    let natoms = u32::try_from(n).map_err(|_| Error::Parse("count overflow".into()))?;
    cols.n_atoms.push(natoms);
    cols.symbol_runs.push(symbol_runs(frame));
    if let Some(xs) = frame.positions.f64_slice().filter(|xs| xs.len() == n * 3) {
        cols.positions.extend_from_slice(xs);
    } else {
        for atom in &frame.atom_data {
            cols.positions
                .extend_from_slice(&crate::atom_coord::atom_xyz(atom));
        }
    }
    if let Some(xs) = frame.masses.f64_slice().filter(|xs| xs.len() == n) {
        cols.masses.extend_from_slice(xs);
    } else {
        for i in 0..n {
            cols.masses.push(frame.masses.get_f64(i));
        }
    }
    let force_at = cols.forces.len();
    if frame.has_forces() {
        if let Some(xs) = frame.forces.f64_slice().filter(|xs| xs.len() == n * 3) {
            cols.forces.extend_from_slice(xs);
        } else {
            for atom in &frame.atom_data {
                cols.forces
                    .extend_from_slice(&atom.force().unwrap_or([0.0; 3]));
            }
        }
    } else {
        cols.forces_uniform = false;
        cols.forces.truncate(force_at);
    }
    Ok(())
}

fn symbol_runs(frame: &readcon_core::types::ConFrame) -> Vec<(String, u32)> {
    let counts = &frame.header.natms_per_type;
    let mut runs: Vec<(String, u32)> = Vec::new();
    let mut offset = 0usize;
    if counts.is_empty() {
        for atom in &frame.atom_data {
            let sym = atom.symbol.as_ref();
            if let Some(last) = runs.last_mut() {
                if last.0 == sym {
                    last.1 += 1;
                    continue;
                }
            }
            runs.push((sym.to_string(), 1));
        }
        return runs;
    }
    for &count in counts {
        if count == 0 || offset >= frame.atom_data.len() {
            continue;
        }
        let sym = frame.atom_data[offset].symbol.as_ref().to_string();
        let n = u32::try_from(count).unwrap_or(u32::MAX);
        runs.push((sym, n));
        offset += count;
    }
    runs
}

/// Index fields of one frame, without a per-atom object or a coordinate array.
#[derive(Debug, Clone, PartialEq)]
pub struct SpanFacts {
    /// Atoms in the frame.
    pub n_atoms: u32,
    /// Symbol counts, sorted by symbol.
    pub species: Vec<(String, u32)>,
    /// Finite frame energy.
    pub energy: Option<f64>,
    /// Max force magnitude.
    pub fmax: Option<f64>,
    /// Sum of per-type mass times count.
    pub total_mass: Option<f64>,
    /// Cell volume.
    pub cell_volume: Option<f64>,
    /// Metadata periodicity.
    pub pbc: Option<[bool; 3]>,
    /// Forces are declared or present.
    pub has_forces: bool,
    /// Velocities are declared or present.
    pub has_velocities: bool,
    /// An energies section or a finite frame energy is present.
    pub has_energy: bool,
    /// Metadata `time`.
    pub time: Option<f64>,
    /// Metadata `timestep`.
    pub timestep: Option<f64>,
    /// Metadata `frame_index`.
    pub frame_index: Option<f64>,
    /// Metadata `neb_bead`.
    pub neb_bead: Option<f64>,
    /// Metadata `neb_band`.
    pub neb_band: Option<f64>,
    /// Metadata `charge`.
    pub charge: Option<f64>,
    /// Metadata `magmom`.
    pub magmom: Option<f64>,
}

/// One readcon-core frame, and it must cover `text`.
///
/// The iterator is walked to the end. A later error is that error. A later
/// successful frame means this slice is not one frame. Trailing whitespace
/// after the single frame is the end of the span, not another frame.
fn one_consuming_frame(text: &str) -> Result<Option<readcon_core::types::ConFrame>> {
    let mut iter = readcon_core::iterators::ConFrameIterator::new(text);
    let (frame, span) = match iter.next_with_raw_span(text) {
        None => return Ok(None),
        Some(Err(err)) => return Err(Error::Parse(err.to_string())),
        Some(Ok(pair)) => pair,
    };
    match iter.next() {
        Some(Err(err)) => Err(Error::Parse(err.to_string())),
        Some(Ok(_)) => Err(Error::Parse(
            "CON validation failed: frame span contains more than one frame".into(),
        )),
        None => {
            let start = (span.as_ptr() as usize).saturating_sub(text.as_ptr() as usize);
            let end = start.saturating_add(span.len());
            let rest = text.get(end..).unwrap_or("");
            if rest.bytes().any(|byte| !byte.is_ascii_whitespace()) {
                return Err(Error::Parse(
                    "CON validation failed: frame span contains more than one frame".into(),
                ));
            }
            Ok(Some(frame))
        }
    }
}

/// Index one frame from its stored bytes.
///
/// One [`readcon_core::iterators::ConFrameIterator`] pass applies every
/// core check. The index fields are the projection of that frame.
#[allow(dead_code)] // index-only entry; ingest uses project_frame_cooked
pub fn project_frame(bytes: &[u8]) -> Result<SpanFacts> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| Error::Parse("CON line is not UTF-8".into()))?;
    match one_consuming_frame(text)? {
        Some(frame) => Ok(facts_from_frame(&frame)),
        None => facts_from_meta(b""),
    }
}

/// Index fields plus an RCSO blob from one readcon-core parse.
#[cfg_attr(not(any(test, feature = "mpi")), allow(dead_code))]
pub(crate) fn project_frame_cooked(bytes: &[u8]) -> Result<(SpanFacts, Vec<u8>)> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| Error::Parse("CON line is not UTF-8".into()))?;
    match one_consuming_frame(text)? {
        Some(frame) => {
            let facts = facts_from_frame(&frame);
            let cooked = crate::cooked_soa::CookedSoa::encode_frame(&frame)?;
            Ok((facts, cooked))
        }
        None => {
            let facts = facts_from_meta(b"")?;
            let cooked = crate::cooked_soa::CookedSoa::encode_columns(
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Some(0.0),
                &[],
                None,
                [0.0; 9],
                facts.energy.unwrap_or(0.0),
                facts.energy.is_some(),
                facts.frame_index.unwrap_or(0.0),
                facts.frame_index.is_some(),
                facts.pbc.unwrap_or([false, false, false]),
                facts.pbc.is_some(),
                &[],
            )?;
            Ok((facts, cooked))
        }
    }
}

fn facts_from_meta(line: &[u8]) -> Result<SpanFacts> {
    let mut facts = SpanFacts {
        n_atoms: 0,
        species: Vec::new(),
        energy: None,
        fmax: None,
        total_mass: None,
        cell_volume: None,
        pbc: None,
        has_forces: false,
        has_velocities: false,
        has_energy: false,
        time: None,
        timestep: None,
        frame_index: None,
        neb_bead: None,
        neb_band: None,
        charge: None,
        magmom: None,
    };
    let text = as_str(line)?.trim();
    if !text.starts_with('{') {
        return Ok(facts);
    }
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| Error::Parse(e.to_string()))?;
    facts.energy = v.get("energy").and_then(json_finite);
    if facts.energy.is_some() {
        facts.has_energy = true;
    }
    facts.frame_index = v.get("frame_index").and_then(|x| {
        x.as_u64()
            .map(|u| u as f64)
            .filter(|f| f.is_finite())
            .or_else(|| json_finite(x))
    });
    facts.time = v.get("time").and_then(json_finite);
    facts.timestep = v.get("timestep").and_then(json_finite);
    facts.neb_bead = v.get("neb_bead").and_then(json_finite);
    facts.neb_band = v.get("neb_band").and_then(json_finite);
    facts.charge = v.get("charge").and_then(json_finite);
    facts.magmom = v.get("magmom").and_then(json_finite);
    if let Some(arr) = v.get("pbc").and_then(serde_json::Value::as_array) {
        if arr.len() == 3 {
            if let (Some(x), Some(y), Some(z)) =
                (arr[0].as_bool(), arr[1].as_bool(), arr[2].as_bool())
            {
                facts.pbc = Some([x, y, z]);
            }
        }
    }
    if let Some(lv) = v
        .get("lattice_vectors")
        .and_then(serde_json::Value::as_array)
    {
        if let Some(vol) = lattice_volume(lv) {
            facts.cell_volume = Some(vol);
        }
    }
    if let Some(sections) = v.get("sections").and_then(serde_json::Value::as_array) {
        for section in sections {
            let Some(name) = section.as_str() else {
                continue;
            };
            if name.eq_ignore_ascii_case("forces") {
                facts.has_forces = true;
            } else if name.eq_ignore_ascii_case("velocities") {
                facts.has_velocities = true;
            } else if name.eq_ignore_ascii_case("energies") {
                facts.has_energy = true;
            }
        }
    }
    Ok(facts)
}

fn json_finite(v: &serde_json::Value) -> Option<f64> {
    let f = if let Some(x) = v.as_f64() {
        x
    } else if let Some(i) = v.as_i64() {
        i as f64
    } else {
        let u = v.as_u64()?;
        u as f64
    };
    f.is_finite().then_some(f)
}

fn lattice_volume(rows: &[serde_json::Value]) -> Option<f64> {
    if rows.len() != 3 {
        return None;
    }
    let mut m = [[0.0f64; 3]; 3];
    for (i, row) in rows.iter().enumerate() {
        let arr = row.as_array()?;
        if arr.len() != 3 {
            return None;
        }
        for (j, v) in arr.iter().enumerate() {
            m[i][j] = v.as_f64()?;
        }
    }
    let det = scalar_triple(m[0], m[1], m[2]).abs();
    det.is_finite().then_some(det)
}

fn scalar_triple(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0])
}

/// The same fields [`readcon_core::index_proj::FrameIndexProjection::from_frame`]
/// stores, so a span index can be compared with a parsed frame.
///
/// Species counts come from the header's type runs, and `fmax` from the
/// force column. Both match the projection without walking every atom.
pub fn facts_from_frame(frame: &readcon_core::types::ConFrame) -> SpanFacts {
    if let Some(facts) = facts_from_columns(frame) {
        return facts;
    }
    let p = readcon_core::index_proj::FrameIndexProjection::from_frame(frame);
    SpanFacts {
        n_atoms: p.n_atoms,
        species: p.species_counts,
        energy: p.energy,
        fmax: p.fmax,
        total_mass: p.total_mass,
        cell_volume: p.cell_volume,
        pbc: p.pbc,
        has_forces: p.has_forces,
        has_velocities: p.has_velocities,
        has_energy: p.has_energy,
        time: p.time,
        timestep: p.timestep,
        frame_index: p.frame_index,
        neb_bead: p.neb_bead,
        neb_band: p.neb_band,
        charge: p.charge,
        magmom: p.magmom,
    }
}

fn section_named(frame: &readcon_core::types::ConFrame, name: &str) -> bool {
    frame
        .header
        .sections
        .iter()
        .any(|section| section.eq_ignore_ascii_case(name))
}

fn facts_from_columns(frame: &readcon_core::types::ConFrame) -> Option<SpanFacts> {
    let species = species_from_runs(frame)?;
    let n = frame.atom_data.len();
    let has_forces = section_named(frame, "forces") || frame.has_forces();
    let has_velocities = section_named(frame, "velocities") || frame.has_velocities();
    let energy = frame.header.energy().filter(|value| value.is_finite());
    let has_energy = section_named(frame, "energies") || frame.has_energies() || energy.is_some();
    let fmax = if has_forces {
        let xs = frame.forces.f64_slice()?;
        if xs.len() != n.saturating_mul(3) {
            return None;
        }
        column_fmax(xs)
    } else {
        None
    };
    Some(SpanFacts {
        n_atoms: n as u32,
        species,
        energy,
        fmax,
        total_mass: readcon_core::index_proj::frame_total_mass(frame),
        cell_volume: readcon_core::index_proj::frame_cell_volume(frame),
        pbc: frame.header.pbc(),
        has_forces,
        has_velocities,
        has_energy,
        time: finite_meta(frame, "time", frame.header.time()),
        timestep: finite_meta(frame, "timestep", frame.header.timestep()),
        frame_index: finite_meta(
            frame,
            "frame_index",
            frame.header.frame_index().map(|index| index as f64),
        ),
        neb_bead: finite_meta(
            frame,
            "neb_bead",
            frame.header.neb_bead().map(|index| index as f64),
        ),
        neb_band: meta_f64(frame, "neb_band"),
        charge: meta_f64(frame, "charge"),
        magmom: meta_f64(frame, "magmom"),
    })
}

fn species_from_runs(frame: &readcon_core::types::ConFrame) -> Option<Vec<(String, u32)>> {
    let counts = &frame.header.natms_per_type;
    if counts.is_empty() {
        return None;
    }
    let n = frame.atom_data.len();
    let mut covered = 0usize;
    let mut order: Vec<(String, u32)> = Vec::new();
    for &count in counts {
        if count == 0 {
            continue;
        }
        if covered >= n {
            return None;
        }
        let symbol = frame.atom_data[covered].symbol.as_ref();
        if !symbol.is_empty() {
            if let Some(slot) = order.iter_mut().find(|(name, _)| name == symbol) {
                slot.1 = slot.1.saturating_add(count as u32);
            } else {
                order.push((symbol.to_string(), count as u32));
            }
        }
        covered = covered.checked_add(count)?;
    }
    if covered != n {
        return None;
    }
    order.sort_by(|left, right| left.0.cmp(&right.0));
    Some(order)
}

fn column_fmax(xs: &[f64]) -> Option<f64> {
    // `sqrt` is monotonic on the finite squares, so one square-root of the
    // winning square matches the per-atom Euclidean magnitude.
    let mut max_sq: Option<f64> = None;
    for tri in xs.as_chunks::<3>().0 {
        let sq = tri[0] * tri[0] + tri[1] * tri[1] + tri[2] * tri[2];
        if sq.is_finite() {
            max_sq = Some(match max_sq {
                Some(cur) if cur >= sq => cur,
                _ => sq,
            });
        }
    }
    max_sq.map(f64::sqrt)
}

fn finite_meta(
    frame: &readcon_core::types::ConFrame,
    key: &str,
    header: Option<f64>,
) -> Option<f64> {
    header
        .filter(|value| value.is_finite())
        .or_else(|| meta_f64(frame, key))
}

fn meta_f64(frame: &readcon_core::types::ConFrame, key: &str) -> Option<f64> {
    let value = frame.header.metadata.get(key)?;
    if let Some(number) = value.as_f64() {
        return number.is_finite().then_some(number);
    }
    if let Some(number) = value.as_i64() {
        let number = number as f64;
        return number.is_finite().then_some(number);
    }
    if let Some(number) = value.as_u64() {
        let number = number as f64;
        return number.is_finite().then_some(number);
    }
    None
}

/// Scan one CON buffer that may hold several frames.
pub fn scan_bytes(bytes: &[u8]) -> Result<Columns> {
    let mut cols = Columns::new();
    append_span(&mut cols, bytes)?;
    Ok(cols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use readcon_core::iterators::ConFrameIterator;

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test")
            .join(name);
        std::fs::read_to_string(path).unwrap()
    }

    fn frames(text: &str) -> Vec<readcon_core::types::ConFrame> {
        ConFrameIterator::new(text)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn columns_match_the_frame_parser_on_fixtures() {
        for name in [
            "tiny_cuh2.con",
            "tiny_cuh2_forces.con",
            "tiny_cuh2_vel_forces.con",
            "tiny_multi_cuh2.con",
        ] {
            let text = fixture(name);
            let got = scan_bytes(text.as_bytes()).unwrap();
            let src = frames(&text);
            assert_eq!(got.n_atoms.len(), src.len(), "{name}");
            let mut p = 0usize;
            for (i, fr) in src.iter().enumerate() {
                let n = fr.atom_data.len();
                assert_eq!(got.n_atoms[i], n as u32, "{name}");
                let masses = fr.masses.f64_slice().expect("f64 masses");
                for (a, atom) in fr.atom_data.iter().enumerate() {
                    let row = &got.positions[p + a * 3..p + a * 3 + 3];
                    assert_eq!(row, &crate::atom_coord::atom_xyz(atom), "{name} pos {a}");
                    assert_eq!(got.masses[p / 3 + a], masses[a], "{name} mass");
                }
                if fr.atom_data.iter().any(|a| a.force().is_some()) {
                    for (a, atom) in fr.atom_data.iter().enumerate() {
                        let f = atom.force().unwrap_or([0.0; 3]);
                        let row = &got.forces[p + a * 3..p + a * 3 + 3];
                        assert_eq!(row, &f, "{name} force {a}");
                    }
                }
                p += n * 3;
                assert_eq!(got.cells[i * 6..i * 6 + 3], fr.header.boxl, "{name} box");
            }
        }
    }

    #[test]
    fn negative_zero_and_pi_keep_their_bits() {
        let text = "\n{\"con_spec_version\":3,\"con_spec_minor\":1,\"energy\":-42.5,\"frame_index\":3,\"pbc\":[true,true,false],\"sections\":[\"forces\"],\"units\":{\"length\":\"angstrom\",\"energy\":\"eV\"}}\n10.5 12.25 8.0\n90 90 90\n\n\n1\n1\n63.546\nCu\nCoordinates of Component 1\n-0.0 3.141592653589793 1.0000000000000002 0 0\n\nCu\nForces of Component 1\n-0.0 0.5 0.25 0 0\n";
        let got = scan_bytes(text.as_bytes()).unwrap();
        let src = frames(text);
        assert_eq!(got.positions[0].to_bits(), (-0.0f64).to_bits());
        assert_eq!(got.positions[1].to_bits(), std::f64::consts::PI.to_bits());
        assert_eq!(got.forces[0].to_bits(), (-0.0f64).to_bits());
        assert_eq!(
            got.positions[0].to_bits(),
            crate::atom_coord::atom_x(&src[0].atom_data[0]).to_bits()
        );
        assert_eq!(
            got.forces[0].to_bits(),
            src[0].atom_data[0].force().unwrap()[0].to_bits()
        );
        assert!(got.energy_set[0]);
        assert_eq!(got.energy[0].to_bits(), (-42.5f64).to_bits());
        assert_eq!(got.frame_index[0], 3.0);
        assert_eq!(&got.pbc[..3], &[true, true, false]);
        assert_eq!(got.symbol_runs[0], vec![("Cu".into(), 1)]);
    }

    fn energies_text(row: Option<&str>) -> String {
        energies_at(3, row)
    }

    fn energies_at(spec: u32, row: Option<&str>) -> String {
        let mut text = format!(
            "\n{{\"con_spec_version\":{spec},\"con_spec_minor\":1,\"energy\":-42.5,\"frame_index\":0,\"pbc\":[true,true,false],\"sections\":[\"energies\"],\"units\":{{\"length\":\"angstrom\",\"energy\":\"eV\"}}}}\n10.5 12.25 8.0\n90 90 90\n\n\n1\n1\n63.546\nCu\nCoordinates of Component 1\n0.125 0.5 1.0 0 0\n\nCu\nEnergies of Component 1\n"
        );
        if let Some(row) = row {
            text.push_str(row);
            text.push('\n');
        }
        text
    }

    fn parser_message(text: &str) -> String {
        match ConFrameIterator::new(text).next() {
            Some(Err(err)) => err.to_string(),
            Some(Ok(_)) => panic!("parser accepted the frame"),
            None => panic!("parser saw no frame"),
        }
    }

    fn span_message(text: &str) -> String {
        match project_frame(text.as_bytes()) {
            Err(Error::Parse(msg)) => msg,
            Err(err) => panic!("span failed with {err}"),
            Ok(_) => panic!("span accepted the frame"),
        }
    }

    /// An empty energies row, a wide row, and a missing row use the parser's words.
    #[test]
    fn energy_row_errors_match_the_parser() {
        let cases = [
            (Some(""), "expected 3 values on line, found 0"),
            (Some("   "), "expected 3 values on line, found 0"),
            (Some("0.25 0 0 4"), "expected 3 values on line, found 4"),
            (Some("0.25 0 0 4 5 6"), "expected 3 values on line, found 6"),
            (
                Some("0.25 9 0"),
                "invalid fixed flag: column 4 flag \"9\" is not a legal integer for spec 3",
            ),
            (
                Some("0.25 0 zz"),
                "invalid number format: invalid integer: zz",
            ),
            (
                Some("0.25 zz 0"),
                "invalid fixed flag: column 4 flag \"zz\" is not a legal integer for spec 3",
            ),
            (
                Some("0.25 -1 0"),
                "invalid fixed flag: column 4 flag \"-1\" is not a legal integer for spec 3",
            ),
            (
                Some("0.25 +1 0"),
                "invalid fixed flag: column 4 flag \"+1\" is not a legal integer for spec 3",
            ),
            (
                Some("0.25 18446744073709551616 0"),
                "invalid fixed flag: column 4 flag \"18446744073709551616\" is not a legal integer for spec 3",
            ),
            (
                Some("0.25 0 18446744073709551616"),
                "invalid number format: integer out of range 0..=2^64-1: 18446744073709551616",
            ),
            (
                None,
                "file ended unexpectedly while reading energies section",
            ),
        ];
        for (row, needle) in cases {
            let text = energies_text(row);
            let parsed = parser_message(&text);
            assert_eq!(span_message(&text), parsed, "row={row:?}");
            assert!(parsed.contains(needle), "{parsed}");
        }
        let spec1 = energies_at(1, Some("0.25 9 0"));
        let parsed = parser_message(&spec1);
        assert_eq!(span_message(&spec1), parsed);
        assert!(
            parsed.contains("unsupported con_spec_version: 1"),
            "{parsed}"
        );
        let headers = [
            (
                r#"{"energy":-1.0}"#,
                "1",
                "line 2 must be a JSON object containing \"con_spec_version\"",
            ),
            (
                r#"{"con_spec_version":-1}"#,
                "1",
                "invalid JSON metadata on line 2: con_spec_version must be a JSON integer, the major version",
            ),
            (
                r#"{"con_spec_version":3}"#,
                "1",
                "invalid units: con_spec_version >= 3 requires metadata \"units\" with length and energy",
            ),
            (
                r#"{"con_spec_version":3,"con_spec_minor":1,"units":{"length":"angstrom","energy":"eV"}}"#,
                "-1",
                "invalid count: count field is not an unsigned integer: -1",
            ),
            (
                r#"{"con_spec_version":3,"con_spec_minor":1,"units":{"length":"angstrom","energy":"eV"}}"#,
                "+1",
                "invalid count: count field is not an unsigned integer: +1",
            ),
            (
                r#"{"con_spec_version":3,"con_spec_minor":1,"units":{"length":"angstrom","energy":"eV"}}"#,
                "18446744073709551616",
                "invalid count: count field is out of range: 18446744073709551616",
            ),
        ];
        for (meta, ntypes, needle) in headers {
            let text = format!("\n{meta}\n10.5 12.25 8.0\n90 90 90\n\n\n{ntypes}\n");
            let parsed = parser_message(&text);
            assert_eq!(span_message(&text), parsed, "{meta} count={ntypes}");
            assert!(parsed.contains(needle), "{parsed}");
        }
    }

    #[test]
    fn cooked_span_matches_the_parsed_blob() {
        for name in [
            "tiny_cuh2.con",
            "tiny_cuh2_forces.con",
            "tiny_cuh2_vel_forces.con",
            "tiny_multi_cuh2.con",
            "sulfolene.con",
        ] {
            let text = fixture(name);
            let mut iter = ConFrameIterator::new(&text);
            let mut n = 0u32;
            while let Some(item) = iter.next_with_raw_span(&text) {
                let (frame, raw) = item.unwrap();
                let facts = project_frame(raw.as_bytes()).unwrap();
                assert_eq!(facts.n_atoms as usize, frame.atom_data.len(), "{name}");
                let (_facts, cooked) = project_frame_cooked(raw.as_bytes()).unwrap();
                let parsed = crate::cooked_soa::CookedSoa::encode_frame(&frame).unwrap();
                let got =
                    crate::cooked_soa::MaterializedFrame::from_blob(&cooked).expect("span blob");
                let expect =
                    crate::cooked_soa::MaterializedFrame::from_blob(&parsed).expect("parsed blob");
                assert_eq!(got.positions, expect.positions, "{name} positions");
                assert_eq!(got.forces, expect.forces, "{name} forces");
                assert_eq!(got.velocities, expect.velocities, "{name} velocities");
                assert_eq!(got.masses, expect.masses, "{name} masses");
                assert_eq!(got.numbers, expect.numbers, "{name} numbers");
                assert_eq!(got.constraints, expect.constraints, "{name} constraints");
                assert_eq!(got.cell, expect.cell, "{name} cell");
                assert_eq!(
                    got.energy.to_bits(),
                    expect.energy.to_bits(),
                    "{name} energy"
                );
                assert_eq!(got.energy_set, expect.energy_set, "{name}");
                assert_eq!(
                    got.frame_index.to_bits(),
                    expect.frame_index.to_bits(),
                    "{name} index"
                );
                assert_eq!(got.frame_index_set, expect.frame_index_set, "{name}");
                assert_eq!(got.pbc, expect.pbc, "{name} pbc");
                assert_eq!(got.pbc_set, expect.pbc_set, "{name}");
                assert_eq!(got.symbol_runs, expect.symbol_runs, "{name} runs");
                n += 1;
            }
            assert!(n > 0, "{name}");
        }
    }

    #[test]
    #[ignore]
    fn profile_million_atom_span() {
        let mut text = String::from(
            "\n{\"con_spec_version\":3,\"con_spec_minor\":1,\"energy\":-42.5,\"frame_index\":0,\"pbc\":[true,true,false],\"sections\":[\"forces\"],\"units\":{\"length\":\"angstrom\",\"energy\":\"eV\"}}\n10.5 12.25 8.0\n90 90 90\n\n\n1\n1000000\n63.546\nCu\nCoordinates of Component 1\n",
        );
        for i in 0..1_000_000 {
            text.push_str(&format!("{i}.0 0.5 1.0 0 {i}\n"));
        }
        text.push_str("\nCu\nForces of Component 1\n");
        for i in 0..1_000_000 {
            text.push_str(&format!("0.1 0.2 0.3 0 {i}\n"));
        }
        let bytes = text.as_bytes();
        let t0 = std::time::Instant::now();
        let cols = scan_bytes(bytes).unwrap();
        let scan = t0.elapsed();
        let t1 = std::time::Instant::now();
        let copied = bytes.to_vec();
        let copy = t1.elapsed();
        let t2 = std::time::Instant::now();
        let compressed = zstd::stream::encode_all(bytes, 1).unwrap();
        let enc = t2.elapsed();
        let t3 = std::time::Instant::now();
        let plain = zstd::stream::decode_all(&compressed[..]).unwrap();
        let dec = t3.elapsed();
        eprintln!(
            "atoms {} bytes {} scan {:?} copy {:?} copied {} zstd_enc {:?} zstd_dec {:?} compressed {} plain {} positions {}",
            cols.n_atoms[0],
            bytes.len(),
            scan,
            copy,
            copied.len(),
            enc,
            dec,
            compressed.len(),
            plain.len(),
            cols.positions.len()
        );
    }

    #[test]
    fn span_and_parser_agree_on_generated_frames() {
        let mut rng = Lcg(0x5eed_0c0f_u64);
        let mut mismatches = Vec::new();
        let n = 2_048usize;
        for i in 0..n {
            let body = mutate_frame(&mut rng, i);
            let text = pad_span(&body);
            let span = frame_message(project_frame(text.as_bytes()));
            let walked = walk_one_frame(&text);
            let parsed = match &walked {
                Ok(_) => "accepted".to_string(),
                Err(err) => err.clone(),
            };
            if span != parsed {
                mismatches.push(format!(
                    "case {i}\nspan:   {span}\nparser: {parsed}\n{body}"
                ));
                if mismatches.len() == 12 {
                    break;
                }
            }
            if let Ok(Some(frame)) = walked {
                let cooked = match project_frame_cooked(text.as_bytes()) {
                    Ok((_facts, blob)) => blob,
                    Err(err) => {
                        mismatches.push(format!("case {i} columns rejected {err}\n{body}"));
                        break;
                    }
                };
                let expect = match crate::cooked_soa::CookedSoa::encode_frame(&frame) {
                    Ok(blob) => blob,
                    Err(err) => {
                        mismatches.push(format!("case {i} encode failed {err}\n{body}"));
                        break;
                    }
                };
                if cooked != expect {
                    mismatches.push(format!("case {i} stored columns differ\n{body}"));
                    if mismatches.len() == 12 {
                        break;
                    }
                }
                if let Ok(cols) = scan_bytes(text.as_bytes()) {
                    if let Some(xs) = frame.forces.f64_slice() {
                        if frame.has_forces() && cols.forces.as_slice() != xs {
                            mismatches.push(format!(
                                "case {i} force columns differ from the parser\n{body}"
                            ));
                            if mismatches.len() == 12 {
                                break;
                            }
                        }
                    }
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "{} mismatches in {n}\n{}",
            mismatches.len(),
            mismatches.join("\n---\n")
        );
    }

    struct Lcg(u64);

    impl Lcg {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
            self.0
        }

        fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
            items[(self.next_u64() as usize) % items.len()]
        }
    }

    fn pad_span(body: &str) -> String {
        let min = 512 * 1024 + 1;
        if body.len() >= min {
            return body.to_string();
        }
        let mut text = " ".repeat(min - body.len());
        text.push_str(body);
        text
    }

    fn frame_message<T>(result: Result<T>) -> String {
        match result {
            Ok(_) => "accepted".into(),
            Err(err) => match err {
                Error::Parse(msg) => msg,
                other => other.to_string(),
            },
        }
    }

    /// Full iterator walk. One `Ok` frame must cover the slice. A later
    /// error is that error, not a silent accept of the first frame.
    fn walk_one_frame(
        text: &str,
    ) -> std::result::Result<Option<readcon_core::types::ConFrame>, String> {
        let mut iter = ConFrameIterator::new(text);
        let (frame, span) = match iter.next_with_raw_span(text) {
            None => return Ok(None),
            Some(Err(err)) => return Err(err.to_string()),
            Some(Ok(pair)) => pair,
        };
        match iter.next() {
            Some(Err(err)) => Err(err.to_string()),
            Some(Ok(_)) => {
                Err("CON validation failed: frame span contains more than one frame".into())
            }
            None => {
                let start = (span.as_ptr() as usize).saturating_sub(text.as_ptr() as usize);
                let end = start.saturating_add(span.len());
                let rest = text.get(end..).unwrap_or("");
                if rest.bytes().any(|byte| !byte.is_ascii_whitespace()) {
                    return Err(
                        "CON validation failed: frame span contains more than one frame".into(),
                    );
                }
                Ok(Some(frame))
            }
        }
    }

    fn mutate_frame(rng: &mut Lcg, index: usize) -> String {
        let spec = ["2", "3", "1", "0", "4", "-1", "01"][index % 7];
        let units = if index.is_multiple_of(5) {
            ""
        } else if index.is_multiple_of(11) {
            r#","units":{"length":"","energy":"eV"}"#
        } else {
            r#","units":{"length":"angstrom","energy":"eV"}"#
        };
        let sections = [
            "",
            r#","sections":["energies"]"#,
            r#","sections":["forces"]"#,
            r#","sections":["velocities"]"#,
            r#","sections":["forces","energies"]"#,
            r#","sections":["energies","forces"]"#,
            r#","sections":["spreads"]"#,
            r#","sections":["charges"]"#,
            r#","sections":["spins"]"#,
            r#","sections":["magmoms"]"#,
            r#","sections":["displacements"]"#,
            r#","sections":["widgets"]"#,
            r#","sections":[]"#,
        ][index % 13];
        let count = [
            "1",
            "0",
            "-1",
            "+1",
            "2",
            "01",
            "18446744073709551616",
            "00",
        ][index % 8];
        let per = if count == "2" { "1 1" } else { "1" };
        let row = rng.pick(&[
            "0.1 0.2 0.3 0 0",
            "-1.5 0 0 0 0",
            "0.1 0.2 0 0",
            "0.1 0.2 0.3 0 0 9",
            "",
            "+1.0 0.2 0.3 0 0",
            "nan 0.2 0.3 0 0",
            "inf 0.2 0.3 0 0",
            "0.1 0.2 0.3 +0 0",
            "0.1 0.2 0.3 0 00",
            "1e999 0 0 0 0",
            "-0.1 0.2 0.3 0 0",
        ]);
        let energy = rng.pick(&[
            "-1.0 0 0",
            "0.25 0 0 4",
            "0.25 zz 0",
            "",
            "nan 0 0",
            "+1 0 0",
        ]);
        let order = index.is_multiple_of(17);
        let mut body = format!(
            "\n{{\"con_spec_version\":{spec}{units}{sections}}}\n10.5 12.25 8.0\n90 90 90\n\n\n{count}\n"
        );
        if count == "0" {
            body.push_str("\n\n");
            if index.is_multiple_of(19) {
                return body[..body.len().saturating_sub(index % 7)].to_string();
            }
            return body;
        }
        if count == "-1" || count == "+1" || count == "18446744073709551616" {
            return body;
        }
        body.push_str(per);
        body.push('\n');
        body.push_str("63.546");
        if count == "2" {
            body.push_str(" 1.0");
        }
        body.push_str("\nCu\nCoordinates of Component 1\n");
        body.push_str(row);
        body.push('\n');
        if count == "2" {
            body.push_str("H\nCoordinates of Component 2\n0.4 0.5 0.6 0 1\n");
        }
        let force = "\nCu\nForces of Component 1\n0.0 0.1 0.2 0 0\n";
        let energy_block = format!("\nCu\nEnergies of Component 1\n{energy}\n");
        let spread = "\nCu\nSpreads of Component 1\n-0.1 0.2 0.3 0 0\n";
        let charge = "\nCu\nCharges of Component 1\n0.5 0 0\n";
        let spin = "\nCu\nSpins of Component 1\n1 0 0\n";
        let mag = "\nCu\nMagmoms of Component 1\n0.1 0.0 0.0 0 0\n";
        let disp = "\nCu\nDisplacements of Component 1\n0.1 0.0 0.0 0 0\n";
        let vel = "\nCu\nVelocities of Component 1\n0.1 0.0 0.0 0 0\n";
        if sections.contains("forces") && sections.contains("energies") {
            if order {
                body.push_str(&energy_block);
                body.push_str(force);
            } else {
                body.push_str(force);
                body.push_str(&energy_block);
            }
        } else if sections.contains("spreads") {
            body.push_str(spread);
        } else if sections.contains("charges") {
            body.push_str(charge);
        } else if sections.contains("spins") {
            body.push_str(spin);
        } else if sections.contains("magmoms") {
            body.push_str(mag);
        } else if sections.contains("displacements") {
            body.push_str(disp);
        } else if sections.contains("velocities") {
            body.push_str(vel);
        } else if sections.contains("energies") {
            body.push_str(&energy_block);
        } else if sections.contains("forces") {
            body.push_str(force);
        }
        if index.is_multiple_of(29) && sections.contains("\"forces\"") {
            body = body.replacen(
                "\"sections\":[\"forces\"]",
                "\"sections\":[\"forces\",\"forces\"]",
                1,
            );
            body.push_str("\nCu\nForces of Component 1\n9.0 8.0 7.0 0 0\n");
        }
        if index.is_multiple_of(31) && sections.contains("\"velocities\"") {
            body = body.replacen(
                "\"sections\":[\"velocities\"]",
                "\"sections\":[\"velocities\",\"velocities\"]",
                1,
            );
            body.push_str("\nCu\nVelocities of Component 1\n4.0 5.0 6.0 0 0\n");
        }
        if index.is_multiple_of(37) {
            body = body.replace("Forces of Component", "FORCES of Component");
            body = body.replace("Velocities of Component", "velocities of Component");
        }
        if index.is_multiple_of(41) && !sections.is_empty() {
            body.push_str("\nCu\nEnergies of Component 1\n0.2 0 0\n");
        }
        if index.is_multiple_of(23) {
            let cut = (index * 17) % body.len().max(1);
            body.truncate(cut);
        }
        body
    }
}
