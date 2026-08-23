//! Canonical unit spellings (metatomic SI table). Callers may write `A`,
//! `ev`, `femtosecond`; metadata stores `angstrom`, `eV`, `fs`.

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
    if !out.contains_key("length") || !out.contains_key("energy") {
        return Err(Error::Message(
            "units must include length and energy".into(),
        ));
    }
    Ok(serde_json::Value::Object(out))
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
    }
}