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
                    assert_eq!(row, &[atom.x, atom.y, atom.z], "{name} pos {a}");
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
        assert_eq!(got.positions[0].to_bits(), src[0].atom_data[0].x.to_bits());
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
            "atoms {} bytes {} scan {:?} copy {:?} zstd_enc {:?} zstd_dec {:?} compressed {} positions {}",
            cols.n_atoms[0],
            bytes.len(),
            scan,
            copy,
            enc,
            dec,
            compressed.len(),
            cols.positions.len()
        );
    }
}
