//! Canonical unit spellings (same preferred-atom table as
//! `readcon_core::units::canonicalize_unit_expression` on core git main).
//! crates.io `readcon-core` 0.14 has the SI parser but not that symbol,
//! so this crate keeps the rewrite here. Callers may write `A`, `ev`,
//! `femtosecond`; metadata stores `angstrom`, `eV`, `fs`.

use crate::error::{Error, Result};

fn preferred_atom(alias: &str) -> Option<&'static str> {
    Some(match alias.trim().to_ascii_lowercase().as_str() {
        "a" | "å" | "angstrom" | "ångstrom" => "angstrom",
        "nm" | "nanometer" | "nanometre" => "nm",
        "m" | "meter" | "metre" => "m",
        "bohr" | "a0" => "bohr",
        "fs" | "femtosecond" | "femtoseconds" => "fs",
        "ps" | "picosecond" | "picoseconds" => "ps",
        "ns" | "nanosecond" | "nanoseconds" => "ns",
        "s" | "sec" | "second" | "seconds" => "s",
        "ev" => "eV",
        "mev" => "meV",
        "hartree" | "ha" => "hartree",
        "j" | "joule" => "J",
        "kj" => "kJ",
        "kcal" => "kcal",
        "amu" | "u" | "dalton" | "da" => "amu",
        "kg" | "kilogram" => "kg",
        "mol" => "mol",
        _ => return None,
    })
}

/// Validate with the SI parser and rewrite aliases to preferred names.
pub fn canonicalize_unit(expr: &str) -> Result<String> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Err(Error::Message("empty unit".into()));
    }
    readcon_core::units::unit_conversion_factor(trimmed, trimmed)
        .map_err(|e| Error::Message(e.to_string()))?;
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out = String::new();
    let bytes = compact.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if matches!(b, b'*' | b'/' | b'(' | b')' | b'^') {
            if !out.is_empty() && !out.ends_with(' ') && b != b')' && b != b'^' {
                out.push(' ');
            }
            out.push(b as char);
            if b != b'(' && b != b'^' {
                out.push(' ');
            }
            i += 1;
            continue;
        }
        if b == b'-' || b.is_ascii_digit() || b == b'.' {
            out.push(b as char);
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphabetic() || bytes[i] > 127)
        {
            i += 1;
        }
        if start == i {
            return Err(Error::Message(format!("bad unit token in '{expr}'")));
        }
        let atom = &compact[start..i];
        let pref = preferred_atom(atom).unwrap_or(atom);
        out.push_str(pref);
    }
    Ok(out.split_whitespace().collect::<Vec<_>>().join(" "))
}

pub fn canonicalize_units_object(units: serde_json::Value) -> Result<serde_json::Value> {
    let obj = units
        .as_object()
        .ok_or_else(|| Error::Message("units must be an object".into()))?;
    let mut out = serde_json::Map::new();
    for (k, v) in obj {
        let Some(s) = v.as_str() else {
            return Err(Error::Message(format!("units.{k} must be a string")));
        };
        out.insert(k.clone(), serde_json::Value::String(canonicalize_unit(s)?));
    }
    Ok(serde_json::Value::Object(out))
}

/// LODE CON defaults when a dimension is missing (spec v3).
pub fn default_con_units() -> serde_json::Value {
    serde_json::json!({
        "length": "angstrom",
        "energy": "eV",
        "mass": "amu",
        "time": "fs"
    })
}

fn dim_unit<'a>(u: &'a serde_json::Value, dim: &str, default: &'a str) -> &'a str {
    u.get(dim).and_then(|v| v.as_str()).unwrap_or(default)
}

fn dim_factor(old: &serde_json::Value, new: &serde_json::Value, dim: &str, default: &str) -> Result<f64> {
    let from = dim_unit(old, dim, default);
    let to = dim_unit(new, dim, from);
    if from == to {
        return Ok(1.0);
    }
    readcon_core::units::unit_conversion_factor(from, to)
        .map_err(|e| Error::Message(e.to_string()))
}

/// Merge `new` onto existing (or CON default) units. Rewrites aliases.
pub fn merge_units(
    old: Option<&serde_json::Value>,
    new: serde_json::Value,
) -> Result<serde_json::Value> {
    let new = canonicalize_units_object(new)?;
    let mut out = match old.and_then(|v| v.as_object()) {
        Some(m) => m.clone(),
        None => default_con_units()
            .as_object()
            .cloned()
            .unwrap_or_default(),
    };
    if let Some(n) = new.as_object() {
        for (k, v) in n {
            out.insert(k.clone(), v.clone());
        }
    }
    Ok(serde_json::Value::Object(out))
}

/// Scale stored numbers so `new_units` is an honest label (not a relabel).
pub fn rescale_frame_units(
    fr: &mut readcon_core::types::ConFrame,
    new_units: &serde_json::Value,
) -> Result<()> {
    let old = fr
        .header
        .units()
        .cloned()
        .unwrap_or_else(default_con_units);
    let lf = dim_factor(&old, new_units, "length", "angstrom")?;
    let ef = dim_factor(&old, new_units, "energy", "eV")?;
    let tf = dim_factor(&old, new_units, "time", "fs")?;
    let mf = dim_factor(&old, new_units, "mass", "amu")?;
    let vf = lf / tf;
    let ff = ef / lf;
    if lf != 1.0 {
        fr.header.boxl[0] *= lf;
        fr.header.boxl[1] *= lf;
        fr.header.boxl[2] *= lf;
        if let Some(arr) = fr.header.metadata.get_mut("lattice_vectors") {
            if let Some(rows) = arr.as_array_mut() {
                for row in rows {
                    if let Some(cs) = row.as_array_mut() {
                        for c in cs {
                            if let Some(x) = c.as_f64() {
                                *c = serde_json::Value::from(x * lf);
                            }
                        }
                    }
                }
            }
        }
    }
    if tf != 1.0 {
        if let Some(t) = fr.header.time() {
            fr.header.set_time(t * tf);
        }
        if let Some(dt) = fr.header.timestep() {
            fr.header.set_timestep(dt * tf);
        }
    }
    if ef != 1.0 {
        if let Some(e) = fr.header.energy() {
            fr.header.set_energy(e * ef);
        }
    }
    let n = fr.positions.nrows();
    for i in 0..n {
        if lf != 1.0 {
            let mut p = fr.positions.as_f64_row(i);
            p[0] *= lf;
            p[1] *= lf;
            p[2] *= lf;
            fr.positions.set_f64_row(i, p);
        }
        if fr.velocities.nrows() == n && vf != 1.0 {
            let mut v = fr.velocities.as_f64_row(i);
            v[0] *= vf;
            v[1] *= vf;
            v[2] *= vf;
            fr.velocities.set_f64_row(i, v);
        }
        if fr.forces.nrows() == n && ff != 1.0 {
            let mut f = fr.forces.as_f64_row(i);
            f[0] *= ff;
            f[1] *= ff;
            f[2] *= ff;
            fr.forces.set_f64_row(i, f);
        }
        if fr.atom_energies.len() == n && ef != 1.0 {
            let e = fr.atom_energies.get_f64(i);
            fr.atom_energies.set_f64(i, e * ef);
        }
        if fr.masses.len() == n && mf != 1.0 {
            let m = fr.masses.get_f64(i);
            fr.masses.set_f64(i, m * mf);
        }
        if let Some(a) = fr.atom_data.get_mut(i) {
            if lf != 1.0 {
                a.x *= lf;
                a.y *= lf;
                a.z *= lf;
            }
            if let Some(v) = a.velocity.as_mut() {
                if vf != 1.0 {
                    v[0] *= vf;
                    v[1] *= vf;
                    v[2] *= vf;
                }
            }
            if let Some(f) = a.force.as_mut() {
                if ff != 1.0 {
                    f[0] *= ff;
                    f[1] *= ff;
                    f[2] *= ff;
                }
            }
            if let Some(e) = a.energy.as_mut() {
                if ef != 1.0 {
                    *e *= ef;
                }
            }
        }
    }
    if mf != 1.0 {
        for m in &mut fr.header.masses_per_type {
            *m *= mf;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_to_preferred() {
        assert_eq!(canonicalize_unit("A").unwrap(), "angstrom");
        assert_eq!(canonicalize_unit("ev").unwrap(), "eV");
        assert_eq!(canonicalize_unit("femtosecond").unwrap(), "fs");
        assert_eq!(
            canonicalize_unit("eV/angstrom").unwrap(),
            "eV / angstrom"
        );
        let u = canonicalize_units_object(serde_json::json!({"time": "femtosecond"})).unwrap();
        assert_eq!(u["time"], "fs");
        assert!(u.get("length").is_none());
    }
}
