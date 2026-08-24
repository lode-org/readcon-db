//! Extended-XYZ writer for optional on-disk handoff (no ASE).
//! Prefer CON + readcon-core; chemfiles ingress for XYZ *input*.

use std::io::{self, Write};

use readcon_core::types::ConFrame;

/// Write one frame as ASE-compatible extxyz (Lattice, Properties, energy/forces in info/arrays).
pub fn write_frame_extxyz<W: Write>(
    w: &mut W,
    frame: &ConFrame,
    energy_key: &str,
) -> io::Result<()> {
    let n = frame.atom_data.len();
    writeln!(w, "{n}")?;

    let e = crate::export_h5md::edges33_from_header(&frame.header);
    let lattice = format!(
        "{:.10} {:.10} {:.10} {:.10} {:.10} {:.10} {:.10} {:.10} {:.10}",
        e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7], e[8]
    );

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

    let pbc = frame.header.pbc().unwrap_or([true, true, true]);
    let tf = |b: bool| if b { "T" } else { "F" };
    write!(
        w,
        "Lattice=\"{lattice}\" Properties={props} pbc=\"{} {} {}\"",
        tf(pbc[0]),
        tf(pbc[1]),
        tf(pbc[2])
    )?;
    if let Some(e) = energy {
        write!(w, " {energy_key}={e:.10}")?;
    }
    writeln!(w)?;

    for a in &frame.atom_data {
        write!(
            w,
            "{:<2} {:16.10} {:16.10} {:16.10}",
            a.symbol, a.x, a.y, a.z
        )?;
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
