//! Column scan of a CON span.
//!
//! The bytes are the stored frame (or a decompressed group member). Numeric
//! tokens go through [`readcon_core::parser::parse_line_of_range_f64_stack`],
//! the same fast-float kernel the core column reader uses, so the bits match
//! a frame parse. No per-atom object is built.

use readcon_core::parser::parse_line_of_range_f64_stack;

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

struct Cur<'a> {
    bytes: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    fn rest_blank(&self) -> bool {
        self.bytes[self.i..].iter().all(|b| b.is_ascii_whitespace())
    }

    fn peek(&self) -> Option<&'a [u8]> {
        line_at(self.bytes, self.i)
    }

    fn take(&mut self) -> Result<&'a [u8]> {
        let (line, next) = split_line(self.bytes, self.i)
            .ok_or_else(|| Error::Parse("truncated CON frame".into()))?;
        self.i = next;
        Ok(line)
    }
}

fn line_at(bytes: &[u8], i: usize) -> Option<&[u8]> {
    split_line(bytes, i).map(|(line, _)| line)
}

fn split_line(bytes: &[u8], i: usize) -> Option<(&[u8], usize)> {
    if i >= bytes.len() {
        return None;
    }
    let rest = &bytes[i..];
    match rest.iter().position(|b| *b == b'\n') {
        Some(n) => {
            let mut line = &rest[..n];
            if line.last() == Some(&b'\r') {
                line = &line[..line.len() - 1];
            }
            Some((line, i + n + 1))
        }
        None => {
            let mut line = rest;
            if line.last() == Some(&b'\r') {
                line = &line[..line.len() - 1];
            }
            Some((line, bytes.len()))
        }
    }
}

fn is_blank(line: &[u8]) -> bool {
    line.iter().all(|b| b.is_ascii_whitespace())
}

fn as_str(line: &[u8]) -> Result<&str> {
    std::str::from_utf8(line).map_err(|_| Error::Parse("CON line is not UTF-8".into()))
}

fn xyz_prefix(line: &[u8]) -> Result<[f64; 3]> {
    // Same kernel as `parse_line_of_range_f64_stack`: fast-float2 on the
    // raw bytes, stopping at whitespace. Trailing flag and id are ignored.
    let mut i = 0usize;
    let mut out = [0.0f64; 3];
    for slot in &mut out {
        while i < line.len() && line[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= line.len() {
            return Err(Error::Parse("coordinate row needs three floats".into()));
        }
        let (val, consumed) = fast_float2::parse_partial::<f64, _>(&line[i..])
            .map_err(|_| Error::Parse("invalid float".into()))?;
        let next = i + consumed;
        if next < line.len() && !line[next].is_ascii_whitespace() {
            return Err(Error::Parse("invalid float".into()));
        }
        if !val.is_finite() {
            return Err(Error::Parse("non-finite coordinate".into()));
        }
        *slot = val;
        i = next;
    }
    Ok(out)
}

fn f64s(line: &[u8], n: usize) -> Result<Vec<f64>> {
    let text = as_str(line)?;
    if n <= 5 {
        let mut buf = [0.0f64; 5];
        parse_line_of_range_f64_stack(text.trim(), n, n, &[0.0; 5], &mut buf)
            .map_err(|e| Error::Parse(e.to_string()))?;
        return Ok(buf[..n].to_vec());
    }
    readcon_core::parser::parse_line_of_n_f64(text.trim(), n)
        .map_err(|e| Error::Parse(e.to_string()))
}

fn uints(line: &[u8]) -> Result<Vec<usize>> {
    let text = as_str(line)?;
    let mut out = Vec::new();
    for tok in text.split_whitespace() {
        let n: usize = tok
            .parse()
            .map_err(|_| Error::Parse(format!("bad count {tok}")))?;
        out.push(n);
    }
    Ok(out)
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
pub fn append_span(cols: &mut Columns, bytes: &[u8]) -> Result<()> {
    let mut cur = Cur { bytes, i: 0 };
    while cur.i < bytes.len() && !cur.rest_blank() {
        parse_frame(&mut cur, cols)?;
    }
    Ok(())
}

fn parse_frame(cur: &mut Cur<'_>, cols: &mut Columns) -> Result<()> {
    let _preamble = cur.take()?;
    let meta = cur.take()?;
    meta_of(meta, cols)?;
    let boxl = f64s(cur.take()?, 3)?;
    let angles = f64s(cur.take()?, 3)?;
    cols.cells.extend_from_slice(&boxl);
    cols.cells.extend_from_slice(&angles);
    let _post_a = cur.take()?;
    let _post_b = cur.take()?;
    let n_types = uints(cur.take()?)?;
    if n_types.len() != 1 {
        return Err(Error::Parse("type-count line must be one integer".into()));
    }
    let n_types = n_types[0];
    if n_types == 0 {
        cols.n_atoms.push(0);
        cols.symbol_runs.push(Vec::new());
        cols.forces_uniform = false;
        return Ok(());
    }
    let counts = uints(cur.take()?)?;
    if counts.len() != n_types {
        return Err(Error::Parse("per-type counts do not match".into()));
    }
    let masses = f64s(cur.take()?, n_types)?;
    let mut runs = Vec::with_capacity(n_types);
    let mut natoms = 0u32;
    let force_at = cols.forces.len();
    let _vel_at = cols.forces.len();
    for (count, mass) in counts.iter().zip(&masses) {
        let symbol = as_str(cur.take()?)?.trim().to_string();
        let label = as_str(cur.take()?)?;
        if !label.contains("Coordinates") {
            return Err(Error::Parse(format!("expected coordinates, got {label}")));
        }
        let n = u32::try_from(*count).map_err(|_| Error::Parse("count overflow".into()))?;
        runs.push((symbol, n));
        for _ in 0..*count {
            let row = xyz_prefix(cur.take()?)?;
            cols.positions.extend_from_slice(&row);
            cols.masses.push(*mass);
            natoms += 1;
        }
    }
    cols.n_atoms.push(natoms);
    cols.symbol_runs.push(runs);
    let mut saw_forces = false;
    loop {
        if cur.i >= cur.bytes.len() || cur.rest_blank() {
            break;
        }
        let Some(peek) = cur.peek() else { break };
        if !is_blank(peek) {
            break;
        }
        let after_blank = split_line(cur.bytes, cur.i)
            .map(|(_, n)| n)
            .unwrap_or(cur.i);
        let Some(next) = line_at(cur.bytes, after_blank) else {
            break;
        };
        if next.first() == Some(&b'{') || is_blank(next) {
            break;
        }
        let _blank = cur.take()?;
        parse_section(cur, &counts, cols, &mut saw_forces)?;
    }
    if !saw_forces {
        cols.forces_uniform = false;
        cols.forces.truncate(force_at);
    }
    Ok(())
}

fn parse_section(
    cur: &mut Cur<'_>,
    counts: &[usize],
    cols: &mut Columns,
    saw_forces: &mut bool,
) -> Result<()> {
    for count in counts {
        let _symbol = cur.take()?;
        let label = as_str(cur.take()?)?;
        let kind = label.split_whitespace().next().unwrap_or("");
        let is_force = kind == "Forces";
        if is_force {
            *saw_forces = true;
        }
        for _ in 0..*count {
            let line = cur.take()?;
            if is_force {
                let row = xyz_prefix(line)?;
                cols.forces.extend_from_slice(&row);
            }
        }
    }
    Ok(())
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

/// Index one frame from its stored bytes.
///
/// Every coordinate, force, and velocity row is checked for field count,
/// float syntax, and the fixed flag. No per-atom object is built. Force
/// magnitudes are reduced to `fmax`. Rows of a large block are checked
/// in parallel.
#[allow(dead_code)] // index-only entry; ingest uses project_frame_cooked
pub fn project_frame(bytes: &[u8]) -> Result<SpanFacts> {
    Ok(project_frame_parts(bytes, false)?.0)
}

/// Index fields plus an RCSO blob, from the same byte walk.
///
/// The coordinate scan already parses every float and fixed flag. Cooking
/// keeps those values instead of building a `ConFrame` and parsing again.
pub(crate) fn project_frame_cooked(bytes: &[u8]) -> Result<(SpanFacts, Vec<u8>)> {
    let (facts, acc) = project_frame_parts(bytes, true)?;
    let blob = finish_cook(&facts, acc.unwrap_or_else(CookAcc::empty))?;
    Ok((facts, blob))
}

pub(crate) struct CookAcc {
    positions: Vec<f64>,
    forces: Vec<f64>,
    velocities: Vec<f64>,
    constraints: Vec<u8>,
    masses: Vec<f64>,
    runs: Vec<(String, u32)>,
    cell: [f64; 9],
}

impl CookAcc {
    pub(crate) fn empty() -> Self {
        Self {
            positions: Vec::new(),
            forces: Vec::new(),
            velocities: Vec::new(),
            constraints: Vec::new(),
            masses: Vec::new(),
            runs: Vec::new(),
            cell: [0.0; 9],
        }
    }
}

pub(crate) fn project_frame_parts(
    bytes: &[u8],
    cook: bool,
) -> Result<(SpanFacts, Option<CookAcc>)> {
    let mut cur = Cur { bytes, i: 0 };
    if cur.rest_blank() {
        return Err(Error::Parse("empty frame".into()));
    }
    let _preamble = cur.take()?;
    let meta = cur.take()?;
    let (mut facts, gate) = facts_from_meta(meta)?;
    let boxl = f64s(cur.take()?, 3)?;
    let angles = f64s(cur.take()?, 3)?;
    if facts.cell_volume.is_none() {
        facts.cell_volume = volume_from_box(&boxl, &angles);
    }
    let _post_a = cur.take()?;
    let _post_b = cur.take()?;
    let n_types_line = uints(cur.take()?)?;
    if n_types_line.len() != 1 {
        return Err(Error::Parse("type-count line must be one integer".into()));
    }
    let n_types = n_types_line[0];
    let mut acc = cook.then(CookAcc::empty);
    if let Some(acc) = acc.as_mut() {
        acc.cell = cell_row_major(gate.lattice, &boxl, &angles);
    }
    if n_types == 0 {
        return Ok((facts, acc));
    }
    let counts = uints(cur.take()?)?;
    if counts.len() != n_types {
        return Err(Error::Parse(format!(
            "expected {n_types} values on line, found {}",
            counts.len()
        )));
    }
    if counts.contains(&0) {
        return Err(Error::Parse(
            "invalid count: each per-type count must be >= 1".into(),
        ));
    }
    let masses = f64s(cur.take()?, n_types)?;
    facts.total_mass = total_mass(&counts, &masses);
    let natoms_sum: usize = counts.iter().sum();
    if let Some(acc) = acc.as_mut() {
        acc.positions.resize(natoms_sum * 3, 0.0);
        acc.constraints.resize(natoms_sum * 3, 0);
        acc.masses = masses.clone();
        acc.runs.reserve(n_types);
    }
    let mut species = std::collections::BTreeMap::<String, u32>::new();
    let mut natoms = 0u32;
    for count in &counts {
        let symbol = as_str(cur.take()?)?.trim().to_string();
        let label = as_str(cur.take()?)?;
        if !label.contains("Coordinates") {
            return Err(Error::Parse(format!("expected coordinates, got {label}")));
        }
        let n = u32::try_from(*count).map_err(|_| Error::Parse("count overflow".into()))?;
        if !symbol.is_empty() {
            *species.entry(symbol.clone()).or_insert(0) += n;
        }
        let spec = gate.spec as u32;
        if let Some(acc) = acc.as_mut() {
            let start = (natoms as usize) * 3;
            let end = start + (*count) * 3;
            check_vector_block(
                &mut cur,
                *count,
                "atom data",
                spec,
                false,
                false,
                Some(&mut acc.positions[start..end]),
                Some(&mut acc.constraints[start..end]),
            )?;
            acc.runs.push((symbol, n));
        } else {
            check_vector_block(
                &mut cur,
                *count,
                "atom data",
                spec,
                false,
                false,
                None,
                None,
            )?;
        }
        natoms += n;
    }
    facts.n_atoms = natoms;
    facts.species = species.into_iter().collect();
    let mut fmax = None;
    loop {
        if cur.i >= cur.bytes.len() || cur.rest_blank() {
            break;
        }
        let Some(peek) = cur.peek() else { break };
        if !is_blank(peek) {
            break;
        }
        let after_blank = split_line(cur.bytes, cur.i)
            .map(|(_, n)| n)
            .unwrap_or(cur.i);
        let Some(next) = line_at(cur.bytes, after_blank) else {
            break;
        };
        if next.first() == Some(&b'{') || is_blank(next) {
            break;
        }
        let _blank = cur.take()?;
        note_section(
            &mut cur,
            &counts,
            gate.spec as u32,
            &mut facts,
            &mut fmax,
            acc.as_mut(),
        )?;
    }
    facts.fmax = fmax;
    if facts.has_velocities && gate.spec >= 3 && gate.minor >= 1 && !gate.units_time {
        return Err(Error::Parse(
            "minor >= 1 requires units.time when a velocities section is present".into(),
        ));
    }
    Ok((facts, acc))
}

pub(crate) fn finish_cook(facts: &SpanFacts, mut acc: CookAcc) -> Result<Vec<u8>> {
    let n = facts.n_atoms as usize;
    let mass_uniform = if n == 0 {
        Some(0.0)
    } else if !acc.masses.is_empty()
        && acc
            .masses
            .iter()
            .all(|m| m.to_bits() == acc.masses[0].to_bits())
    {
        Some(acc.masses[0])
    } else {
        None
    };
    let expanded = if mass_uniform.is_none() {
        let mut out = Vec::with_capacity(n);
        for ((_, count), mass) in acc.runs.iter().zip(&acc.masses) {
            out.extend(std::iter::repeat(*mass).take(*count as usize));
        }
        out
    } else {
        Vec::new()
    };
    let constraints = if acc.constraints.iter().any(|b| *b != 0) {
        Some(std::mem::take(&mut acc.constraints))
    } else {
        drop(std::mem::take(&mut acc.constraints));
        None
    };
    let forces = if acc.forces.len() == n * 3 {
        std::mem::take(&mut acc.forces)
    } else {
        drop(std::mem::take(&mut acc.forces));
        Vec::new()
    };
    let velocities = if acc.velocities.len() == n * 3 {
        std::mem::take(&mut acc.velocities)
    } else {
        drop(std::mem::take(&mut acc.velocities));
        Vec::new()
    };
    let runs: Vec<(&str, u32)> = acc
        .runs
        .iter()
        .map(|(sym, count)| (sym.as_str(), *count))
        .collect();
    let pbc = facts.pbc.unwrap_or([false, false, false]);
    crate::cooked_soa::CookedSoa::encode_columns(
        facts.n_atoms,
        acc.positions,
        forces,
        velocities,
        mass_uniform,
        &expanded,
        constraints,
        acc.cell,
        facts.energy.unwrap_or(0.0),
        facts.energy.is_some(),
        facts.frame_index.unwrap_or(0.0),
        facts.frame_index.is_some(),
        pbc,
        facts.pbc.is_some(),
        &runs,
    )
}

fn cell_row_major(lattice: Option<[[f64; 3]; 3]>, boxl: &[f64], angles: &[f64]) -> [f64; 9] {
    let m = if let Some(lv) = lattice {
        lv
    } else if boxl.len() >= 3 && angles.len() >= 3 {
        readcon_core::types::cell_vectors(
            [boxl[0], boxl[1], boxl[2]],
            [angles[0], angles[1], angles[2]],
        )
    } else {
        [[0.0; 3]; 3]
    };
    [
        m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
    ]
}

fn note_section(
    cur: &mut Cur<'_>,
    counts: &[usize],
    spec: u32,
    facts: &mut SpanFacts,
    fmax: &mut Option<f64>,
    mut cook: Option<&mut CookAcc>,
) -> Result<()> {
    for count in counts {
        let _symbol = cur
            .take()
            .map_err(|_| Error::Parse("file ended unexpectedly while reading atom data".into()))?;
        let label = as_str(cur.take()?)?;
        let kind = label.split_whitespace().next().unwrap_or("");
        let where_ = match kind {
            "Forces" => "force section",
            "Velocities" => "velocity section",
            "Energies" => "energy section",
            _ => "atom data",
        };
        match kind {
            "Forces" => {
                facts.has_forces = true;
                let n = *count;
                let local = if let Some(c) = cook.as_mut() {
                    let start = c.forces.len();
                    c.forces.resize(start + n * 3, 0.0);
                    check_vector_block(
                        cur,
                        n,
                        where_,
                        spec,
                        true,
                        true,
                        Some(&mut c.forces[start..]),
                        None,
                    )?
                } else {
                    check_vector_block(cur, n, where_, spec, true, true, None, None)?
                };
                if let Some(mag) = local {
                    *fmax = Some(fmax.map_or(mag, |cur| cur.max(mag)));
                }
            }
            "Velocities" => {
                facts.has_velocities = true;
                let n = *count;
                if let Some(c) = cook.as_mut() {
                    let start = c.velocities.len();
                    c.velocities.resize(start + n * 3, 0.0);
                    check_vector_block(
                        cur,
                        n,
                        where_,
                        spec,
                        true,
                        false,
                        Some(&mut c.velocities[start..]),
                        None,
                    )?;
                } else {
                    check_vector_block(cur, n, where_, spec, true, false, None, None)?;
                }
            }
            "Energies" => {
                facts.has_energy = true;
                let ranges = take_row_ranges(cur, *count, where_)?;
                check_scalar_rows(cur.bytes, &ranges, spec)?;
            }
            _ => {
                check_vector_block(cur, *count, where_, spec, true, false, None, None)?;
            }
        }
    }
    Ok(())
}

/// Short blocks are checked in place. A long block records line ranges and
/// checks them on several threads.
fn check_vector_block(
    cur: &mut Cur<'_>,
    n: usize,
    where_: &str,
    spec: u32,
    id_required: bool,
    fmax: bool,
    xyz_out: Option<&mut [f64]>,
    mask_out: Option<&mut [u8]>,
) -> Result<Option<f64>> {
    if n < 8_192 {
        let mut max_mag = None;
        let mut xyz_out = xyz_out;
        let mut mask_out = mask_out;
        for i in 0..n {
            let line = cur.take().map_err(|_| {
                Error::Parse(format!("file ended unexpectedly while reading {where_}"))
            })?;
            let (xyz, mask) = check_vector_line(line, spec, id_required)?;
            if let Some(buf) = xyz_out.as_mut() {
                buf[i * 3..i * 3 + 3].copy_from_slice(&xyz);
            }
            if let Some(buf) = mask_out.as_mut() {
                buf[i * 3..i * 3 + 3].copy_from_slice(&mask);
            }
            if fmax {
                let mag = (xyz[0] * xyz[0] + xyz[1] * xyz[1] + xyz[2] * xyz[2]).sqrt();
                if mag.is_finite() {
                    max_mag = Some(max_mag.map_or(mag, |cur: f64| cur.max(mag)));
                }
            }
        }
        return Ok(max_mag);
    }
    // A million coordinate lines is 16 MiB of ranges. Walk them in
    // bounded pieces so that scratch does not sit next to the columns.
    let mut max_mag = None;
    let mut done = 0usize;
    let mut xyz_out = xyz_out;
    let mut mask_out = mask_out;
    while done < n {
        let take = (n - done).min(16_384);
        let ranges = take_row_ranges(cur, take, where_)?;
        let xyz_piece = match xyz_out {
            Some(buf) => {
                let (head, tail) = buf.split_at_mut(take * 3);
                xyz_out = Some(tail);
                Some(head)
            }
            None => None,
        };
        let mask_piece = match mask_out {
            Some(buf) => {
                let (head, tail) = buf.split_at_mut(take * 3);
                mask_out = Some(tail);
                Some(head)
            }
            None => None,
        };
        if let Some(mag) = check_vector_rows(
            cur.bytes,
            &ranges,
            spec,
            id_required,
            fmax,
            xyz_piece,
            mask_piece,
        )? {
            max_mag = Some(max_mag.map_or(mag, |cur: f64| cur.max(mag)));
        }
        done += take;
    }
    Ok(max_mag)
}

fn take_row_ranges(cur: &mut Cur<'_>, n: usize, where_: &str) -> Result<Vec<(usize, usize)>> {
    let mut ranges = Vec::with_capacity(n);
    let base = cur.bytes.as_ptr() as usize;
    for _ in 0..n {
        let line = cur
            .take()
            .map_err(|_| Error::Parse(format!("file ended unexpectedly while reading {where_}")))?;
        let start = line.as_ptr() as usize - base;
        ranges.push((start, start + line.len()));
    }
    Ok(ranges)
}

/// `id_required` is false for coordinate rows, where the id may be omitted.
/// `fmax` reduces the Euclidean force magnitude when the row is a force.
fn check_vector_rows(
    bytes: &[u8],
    ranges: &[(usize, usize)],
    spec: u32,
    id_required: bool,
    fmax: bool,
    mut xyz_out: Option<&mut [f64]>,
    mut mask_out: Option<&mut [u8]>,
) -> Result<Option<f64>> {
    if ranges.len() < 8_192 {
        return check_vector_rows_seq(bytes, ranges, spec, id_required, fmax, xyz_out, mask_out);
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 8);
    let chunk = ranges.len().div_ceil(threads).max(1);
    let mut maxes = Vec::new();
    let mut error = None;
    std::thread::scope(|scope| {
        let mut joins = Vec::new();
        for piece in ranges.chunks(chunk) {
            let n = piece.len();
            let xyz_piece = match xyz_out {
                Some(buf) => {
                    let (head, tail) = buf.split_at_mut(n * 3);
                    xyz_out = Some(tail);
                    Some(head)
                }
                None => None,
            };
            let mask_piece = match mask_out {
                Some(buf) => {
                    let (head, tail) = buf.split_at_mut(n * 3);
                    mask_out = Some(tail);
                    Some(head)
                }
                None => None,
            };
            joins.push(scope.spawn(move || {
                check_vector_rows_seq(bytes, piece, spec, id_required, fmax, xyz_piece, mask_piece)
            }));
        }
        for join in joins {
            match join.join().expect("row check") {
                Ok(Some(mag)) => maxes.push(mag),
                Ok(None) => {}
                Err(err) => error = Some(err),
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    Ok(maxes.into_iter().reduce(f64::max))
}

fn check_vector_rows_seq(
    bytes: &[u8],
    ranges: &[(usize, usize)],
    spec: u32,
    id_required: bool,
    fmax: bool,
    mut xyz_out: Option<&mut [f64]>,
    mut mask_out: Option<&mut [u8]>,
) -> Result<Option<f64>> {
    let mut max_mag = None;
    for (i, &(start, end)) in ranges.iter().enumerate() {
        let (xyz, mask) = check_vector_line(&bytes[start..end], spec, id_required)?;
        if let Some(buf) = xyz_out.as_mut() {
            buf[i * 3..i * 3 + 3].copy_from_slice(&xyz);
        }
        if let Some(buf) = mask_out.as_mut() {
            buf[i * 3..i * 3 + 3].copy_from_slice(&mask);
        }
        if fmax {
            let mag = (xyz[0] * xyz[0] + xyz[1] * xyz[1] + xyz[2] * xyz[2]).sqrt();
            if mag.is_finite() {
                max_mag = Some(max_mag.map_or(mag, |cur: f64| cur.max(mag)));
            }
        }
    }
    Ok(max_mag)
}

fn check_scalar_rows(bytes: &[u8], ranges: &[(usize, usize)], spec: u32) -> Result<()> {
    for &(start, end) in ranges {
        check_scalar_line(&bytes[start..end], spec)?;
    }
    Ok(())
}

fn next_token(line: &[u8], mut i: usize) -> Option<(usize, usize)> {
    while i < line.len() && line[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= line.len() {
        return None;
    }
    let start = i;
    while i < line.len() && !line[i].is_ascii_whitespace() {
        i += 1;
    }
    Some((start, i))
}

fn check_vector_line(line: &[u8], spec: u32, id_required: bool) -> Result<([f64; 3], [u8; 3])> {
    let mut i = 0usize;
    let mut xyz = [0.0f64; 3];
    for (slot, dest) in xyz.iter_mut().enumerate() {
        let Some((start, end)) = next_token(line, i) else {
            return Err(width_err(slot));
        };
        *dest = parse_f64_token(&line[start..end])?;
        i = end;
    }
    let Some((start, end)) = next_token(line, i) else {
        return Err(width_err(3));
    };
    let mask = fixed_axes(&line[start..end], spec)?;
    i = end;
    match next_token(line, i) {
        Some((start, end)) => {
            parse_id(&line[start..end])?;
            i = end;
            if next_token(line, i).is_some() {
                let mut found = 5usize;
                let mut at = i;
                while let Some((_, end)) = next_token(line, at) {
                    found += 1;
                    at = end;
                }
                return Err(width_err(found));
            }
        }
        None if id_required => return Err(width_err(4)),
        None => {}
    }
    Ok((xyz, mask))
}

fn check_scalar_line(line: &[u8], spec: u32) -> Result<()> {
    let mut i = 0usize;
    let Some((start, end)) = next_token(line, i) else {
        return Err(width_err(0));
    };
    parse_f64_token(&line[start..end])?;
    i = end;
    let Some((start, end)) = next_token(line, i) else {
        return Err(Error::Parse("expected 3 values on line, found 1".into()));
    };
    parse_flag(&line[start..end], spec)?;
    i = end;
    let Some((start, end)) = next_token(line, i) else {
        return Err(Error::Parse("expected 3 values on line, found 2".into()));
    };
    parse_id(&line[start..end])?;
    if next_token(line, end).is_some() {
        return Err(Error::Parse("expected 3 values on line, found more".into()));
    }
    Ok(())
}

fn width_err(found: usize) -> Error {
    Error::Parse(format!("expected 5 values on line, found {found}"))
}

fn parse_f64_token(tok: &[u8]) -> Result<f64> {
    let shown = String::from_utf8_lossy(tok);
    let (val, consumed) = fast_float2::parse_partial::<f64, _>(tok)
        .map_err(|_| Error::Parse(format!("invalid number format: invalid float: {shown}")))?;
    if consumed != tok.len() {
        return Err(Error::Parse(format!(
            "invalid number format: invalid float: {shown}"
        )));
    }
    if !val.is_finite() {
        return Err(Error::Parse(format!("non-finite value: {shown}")));
    }
    Ok(val)
}

fn fixed_axes(tok: &[u8], spec: u32) -> Result<[u8; 3]> {
    if tok.is_empty() || !tok.iter().all(|b| b.is_ascii_digit()) {
        return Err(Error::Parse(format!(
            "invalid fixed flag: {}",
            String::from_utf8_lossy(tok)
        )));
    }
    let val: u64 = std::str::from_utf8(tok)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            Error::Parse(format!(
                "invalid fixed flag: {}",
                String::from_utf8_lossy(tok)
            ))
        })?;
    if spec < 2 {
        return match val {
            0 => Ok([0, 0, 0]),
            1 => Ok([1, 1, 1]),
            _ => Err(Error::Parse(format!(
                "invalid fixed flag: {}",
                String::from_utf8_lossy(tok)
            ))),
        };
    }
    if val > 7 {
        return Err(Error::Parse(format!(
            "invalid fixed flag: {}",
            String::from_utf8_lossy(tok)
        )));
    }
    Ok([
        u8::from(val & 1 != 0),
        u8::from(val & 2 != 0),
        u8::from(val & 4 != 0),
    ])
}

fn parse_flag(tok: &[u8], spec: u32) -> Result<()> {
    fixed_axes(tok, spec).map(|_| ())
}

fn parse_id(tok: &[u8]) -> Result<()> {
    if tok.is_empty() || !tok.iter().all(|b| b.is_ascii_digit()) {
        return Err(Error::Parse(format!(
            "invalid number format: invalid float: {}",
            String::from_utf8_lossy(tok)
        )));
    }
    Ok(())
}

struct MetaGate {
    spec: u64,
    minor: u64,
    units_time: bool,
    lattice: Option<[[f64; 3]; 3]>,
}

fn facts_from_meta(line: &[u8]) -> Result<(SpanFacts, MetaGate)> {
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
        return Ok((
            facts,
            MetaGate {
                spec: 0,
                minor: 0,
                units_time: false,
                lattice: None,
            },
        ));
    }
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| Error::Parse(e.to_string()))?;
    let gate = MetaGate {
        spec: v.get("con_spec_version").and_then(json_u64).unwrap_or(0),
        minor: v.get("con_spec_minor").and_then(json_u64).unwrap_or(0),
        units_time: v
            .get("units")
            .and_then(|units| units.get("time"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|unit| !unit.trim().is_empty()),
        lattice: lattice_matrix(&v),
    };
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
    Ok((facts, gate))
}

fn json_u64(v: &serde_json::Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_i64().and_then(|i| u64::try_from(i).ok()))
}

fn json_finite(v: &serde_json::Value) -> Option<f64> {
    let f = if let Some(x) = v.as_f64() {
        x
    } else if let Some(i) = v.as_i64() {
        i as f64
    } else if let Some(u) = v.as_u64() {
        u as f64
    } else {
        return None;
    };
    f.is_finite().then_some(f)
}

fn total_mass(counts: &[usize], masses: &[f64]) -> Option<f64> {
    let n = counts.len().min(masses.len());
    if n == 0 {
        return None;
    }
    let mut sum = 0.0f64;
    for i in 0..n {
        let mi = masses[i];
        let ni = counts[i] as f64;
        if !mi.is_finite() || !ni.is_finite() {
            return None;
        }
        sum += mi * ni;
    }
    sum.is_finite().then_some(sum)
}

fn lattice_matrix(v: &serde_json::Value) -> Option<[[f64; 3]; 3]> {
    let rows = v.get("lattice_vectors")?.as_array()?;
    if rows.len() != 3 {
        return None;
    }
    let mut m = [[0.0f64; 3]; 3];
    for (i, row) in rows.iter().enumerate() {
        let arr = row.as_array()?;
        if arr.len() != 3 {
            return None;
        }
        for (j, cell) in arr.iter().enumerate() {
            m[i][j] = json_finite(cell)?;
        }
    }
    Some(m)
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

fn volume_from_box(boxl: &[f64], angles: &[f64]) -> Option<f64> {
    if boxl.len() < 3 || angles.len() < 3 {
        return None;
    }
    let [a, b, c] = [boxl[0], boxl[1], boxl[2]];
    let [alpha, beta, gamma] = [angles[0], angles[1], angles[2]];
    if ![a, b, c, alpha, beta, gamma]
        .iter()
        .all(|x| x.is_finite() && *x > 0.0)
    {
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

/// The same fields [`readcon_core::index_proj::FrameIndexProjection::from_frame`]
/// stores, so a span index can be compared with a parsed frame.
pub fn facts_from_frame(frame: &readcon_core::types::ConFrame) -> SpanFacts {
    use readcon_core::index_proj::FrameIndexProjection;
    let p = FrameIndexProjection::from_frame(frame);
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
}
