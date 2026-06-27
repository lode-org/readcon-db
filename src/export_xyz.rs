//! Minimal extended-XYZ writer for metatrain / ASE without depending on ASE at compile time.
//! Energies/forces from CON frame metadata and atom sections when present.

use std::io::{self, Write};

use readcon_core::types::ConFrame;

/// Write one frame as ASE-compatible extxyz (Lattice, Properties, energy/forces in info/arrays).
pub fn write_frame_extxyz<W: Write>(w: &mut W, frame: &ConFrame, energy_key: &str) -> io::Result<()> {
    let n = frame.atom_data.len();
    writeln!(w, "{n}")?;

    // Cell as 3x3 row-major from lengths/angles (orthorhombic approximation if angles ~90)
    let (lx, ly, lz) = (
        frame.header.boxl[0],
        frame.header.boxl[1],
        frame.header.boxl[2],
    );
    // CON stores cell lengths on header; angles on header.angles — use orthorhombic box for export
    // (full triclinic can be added; metatrain accepts Lattice=)
    let lattice = format!("{lx:.10} 0 0 0 {ly:.10} 0 0 0 {lz:.10}");

    let mut energy = None;
    if let Some(v) = frame.header.metadata.get("energy") {
        if let Some(f) = v.as_f64() {
            energy = Some(f);
        } else if let Some(s) = v.as_str() {
            energy = s.parse().ok();
        }
    }

    let has_forces = frame.atom_data.iter().any(|a| a.force.is_some());
    let props = if has_forces {
        "species:S:1:pos:R:3:forces:R:3"
    } else {
        "species:S:1:pos:R:3"
    };

    write!(w, "Lattice=\"{lattice}\" Properties={props} pbc=\"T T T\"")?;
    if let Some(e) = energy {
        write!(w, " {energy_key}={e:.10}")?;
    }
    writeln!(w)?;

    for a in &frame.atom_data {
        write!(w, "{:<2} {:16.10} {:16.10} {:16.10}", a.symbol, a.x, a.y, a.z)?;
        if has_forces {
            let f = a.force.unwrap_or([0.0; 3]);
            write!(w, " {:16.10} {:16.10} {:16.10}", f[0], f[1], f[2])?;
        }
        writeln!(w)?;
    }
    Ok(())
}

pub fn write_frames_extxyz<W: Write>(
    w: &mut W,
    frames: &[ConFrame],
    energy_key: &str,
) -> io::Result<()> {
    for fr in frames {
        write_frame_extxyz(w, fr, energy_key)?;
    }
    Ok(())
}
