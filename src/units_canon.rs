//! Canonical unit spellings (same preferred-atom table as
//! `readcon_core::units::canonicalize_unit_expression` on core git main).
//! crates.io `readcon-core` 0.14 has the SI parser but not that symbol,
//! so this crate keeps the rewrite here. Callers may write `A`, `ev`,
//! `femtosecond`; metadata stores `angstrom`, `eV`, `fs`.

use std::collections::BTreeMap;

use crate::error::{Error, Result};

use readcon_core::types::meta;

/// v3.1 clock and velocity numbers have no historical `fs` default.
const V31_CLOCK_NEEDS_TIME: &str = "v3.1 frame has a clock but no units.time (D10)";
/// Same rule when the frame has velocities and no clock.
const V31_VELOCITY_NEEDS_TIME: &str = "v3.1 frame has velocities but no units.time (D10)";

/// Natural time unit eOn and ASE write: sqrt(amu * Angstrom^2 / eV).
///
/// readcon-core 0.16.2 parses this as a time (about 10.180505710759414 fs).
/// eOn hard-codes 10.1805055 fs. D10.
///
/// R. Goswami, MethodsX 2026, doi:10.1016/j.mex.2026.103899.
/// R. Goswami, MethodsX 2026, doi:10.1016/j.mex.2026.103851.
/// R. Goswami et al., J. Chem. Theory Comput. 2025, doi:10.1021/acs.jctc.5c00866.
/// S. T. Chill et al., Modelling and Simulation in Materials Science and Engineering 22 (2014) 055002, doi:10.1088/0965-0393/22/5/055002.
pub const NATURAL_TIME_UNIT: &str = "(amu*angstrom^2/eV)^0.5";

/// Minor from line 2. Absent means v3.0 when `spec_version` is 3 (D10, D15).
pub fn con_spec_minor(header: &readcon_core::types::FrameHeader) -> Option<u64> {
    header
        .metadata
        .get("con_spec_minor")
        .and_then(|v| v.as_u64())
}

/// From v3.1 on, `units.time` is the velocity time unit.
/// v1, v2, and v3.0 (no `con_spec_minor`) keep the natural unit whatever the label says.
pub fn velocity_time_is_declared(header: &readcon_core::types::FrameHeader) -> bool {
    velocity_time_is_declared_spec(header.spec_version, con_spec_minor(header))
}

fn velocity_time_is_declared_spec(spec_version: u32, minor: Option<u64>) -> bool {
    if spec_version > 3 {
        return true;
    }
    spec_version == 3 && minor.unwrap_or(0) >= 1
}

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
        while i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i] > 127) {
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
///
/// `time: fs` is the historical clock for v1, v2, and v3.0.
/// Spec v3.1 does not use it. [`rescale_frame_units`] rejects a v3.1
/// clock or velocities frame that has no `units.time`.
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

fn dim_factor(
    old: &serde_json::Value,
    new: &serde_json::Value,
    dim: &str,
    default: &str,
) -> Result<f64> {
    let from = dim_unit(old, dim, default);
    let to = dim_unit(new, dim, from);
    if from == to {
        return Ok(1.0);
    }
    readcon_core::units::unit_conversion_factor(from, to).map_err(|e| Error::Message(e.to_string()))
}

/// Merge `new` onto existing (or CON default) units. Rewrites aliases.
pub fn merge_units(
    old: Option<&serde_json::Value>,
    new: serde_json::Value,
) -> Result<serde_json::Value> {
    let new = canonicalize_units_object(new)?;
    let mut out = match old.and_then(|v| v.as_object()) {
        Some(m) => m.clone(),
        None => default_con_units().as_object().cloned().unwrap_or_default(),
    };
    if let Some(n) = new.as_object() {
        for (k, v) in n {
            out.insert(k.clone(), v.clone());
        }
    }
    Ok(serde_json::Value::Object(out))
}

fn declared_time_unit(header: &readcon_core::types::FrameHeader) -> Option<&str> {
    declared_time_unit_in(&header.metadata)
}

fn declared_time_unit_in(metadata: &BTreeMap<String, serde_json::Value>) -> Option<&str> {
    metadata
        .get(meta::UNITS)
        .and_then(|units| units.get("time"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|unit| !unit.is_empty())
}

fn frame_has_clock(header: &readcon_core::types::FrameHeader) -> bool {
    metadata_has_clock(&header.metadata)
}

/// `time`, or a finite `timestep` greater than zero. Same predicate as
/// [`frame_has_clock`].
fn metadata_has_clock(metadata: &BTreeMap<String, serde_json::Value>) -> bool {
    if metadata
        .get(meta::TIME)
        .and_then(serde_json::Value::as_f64)
        .is_some()
    {
        return true;
    }
    matches!(
        metadata.get(meta::TIMESTEP).and_then(serde_json::Value::as_f64),
        Some(dt) if dt.is_finite() && dt > 0.0
    )
}

fn frame_has_velocities(fr: &readcon_core::types::ConFrame) -> bool {
    fr.velocities.nrows() > 0 || fr.atom_data.iter().any(|atom| atom.velocity().is_some())
}

/// v3.1 clock and velocity numbers have no historical `fs` default.
/// Missing `units.time` is the same error `collect_h5md` reports.
fn require_declared_v31_time(fr: &readcon_core::types::ConFrame) -> Result<()> {
    if !velocity_time_is_declared(&fr.header) || declared_time_unit(&fr.header).is_some() {
        return Ok(());
    }
    if frame_has_clock(&fr.header) {
        return Err(Error::Message(V31_CLOCK_NEEDS_TIME.into()));
    }
    if frame_has_velocities(fr) {
        return Err(Error::Message(V31_VELOCITY_NEEDS_TIME.into()));
    }
    Ok(())
}

/// Make `units.time` describe `time`, `timestep`, and the velocities on a
/// frame [`readcon_core::types::ConFrameBuilder`] will write as v3.1.
///
/// Returns the factor that multiplies velocity components still stored in
/// [`NATURAL_TIME_UNIT`]. Clock numbers are not changed.
///
/// This is [`velocity_time_is_declared`] and the velocity term of
/// [`rescale_frame_units`] (`vf = lf / vel_tf` with length unchanged),
/// applied before the builder stamps minor 1:
///
/// * A v3.1 frame that already has `units.time` is left alone (scale 1).
/// * A v3.1 clock or velocities frame with no `units.time` is the same
///   error as [`require_declared_v31_time`]. The missing unit is not `fs`.
/// * Pre-v3.1 velocities are in the natural unit. The clock is the declared
///   `units.time`, or `fs` from [`default_con_units`] when that key is
///   absent. A clock sets `units.time` to that unit. Velocities are
///   multiplied by `unit_conversion_factor(clock, natural)`.
/// * Pre-v3.1 velocities and no clock keep the natural unit and a scale of 1.
/// * A clock with no velocities still receives `units.time`.
///
/// An explicit `con_spec_minor` below 1 is raised to 1 when a clock or
/// velocities are present, so the written label is authoritative. The
/// builder already stamps 1 when the key is absent.
#[cfg(any(test, feature = "python"))]
pub fn align_written_v31_time(
    meta: &mut BTreeMap<String, serde_json::Value>,
    spec_version: u32,
    has_velocity: bool,
) -> Result<f64> {
    let minor = meta
        .get(meta::CON_SPEC_MINOR)
        .and_then(serde_json::Value::as_u64);
    let has_clock = metadata_has_clock(meta);
    if velocity_time_is_declared_spec(spec_version, minor) {
        if declared_time_unit_in(meta).is_none() && (has_clock || has_velocity) {
            let msg = if has_clock {
                V31_CLOCK_NEEDS_TIME
            } else {
                V31_VELOCITY_NEEDS_TIME
            };
            return Err(Error::Message(msg.into()));
        }
        return Ok(1.0);
    }
    if !has_clock && !has_velocity {
        return Ok(1.0);
    }
    let (unit, scale) = if has_clock {
        let raw = declared_time_unit_in(meta).unwrap_or("fs");
        let unit = readcon_core::units::canonicalize_unit_expression(raw)
            .map_err(|e| Error::Message(e.to_string()))?;
        let scale = if has_velocity {
            velocity_scale_from_natural(&unit)?
        } else {
            1.0
        };
        (unit, scale)
    } else {
        (NATURAL_TIME_UNIT.to_string(), 1.0)
    };
    put_time_unit(meta, &unit)?;
    if meta.contains_key(meta::CON_SPEC_MINOR) {
        meta.insert(
            meta::CON_SPEC_MINOR.to_string(),
            serde_json::Value::from(1_u64),
        );
    }
    Ok(scale)
}

/// `v_clock = v_natural * unit_conversion_factor(clock, natural)`.
///
/// Velocity scales as 1/time, so this is `1 / unit_conversion_factor(natural, clock)`,
/// the same factor [`rescale_frame_units`] applies when length is unchanged.
#[cfg(any(test, feature = "python"))]
fn velocity_scale_from_natural(target: &str) -> Result<f64> {
    if target == NATURAL_TIME_UNIT {
        return Ok(1.0);
    }
    let factor = readcon_core::units::unit_conversion_factor(target, NATURAL_TIME_UNIT)
        .map_err(|e| Error::Message(e.to_string()))?;
    if factor == 0.0 || !factor.is_finite() {
        return Err(Error::Message(format!(
            "time unit '{target}' has a zero scale"
        )));
    }
    Ok(factor)
}

#[cfg(any(test, feature = "python"))]
fn put_time_unit(meta: &mut BTreeMap<String, serde_json::Value>, unit: &str) -> Result<()> {
    let time = serde_json::Value::String(unit.to_string());
    match meta.get_mut(meta::UNITS) {
        Some(serde_json::Value::Object(map)) => {
            map.insert("time".to_string(), time);
        }
        Some(_) => return Err(Error::Message("units must be an object".into())),
        None => {
            let mut units = readcon_core::units::default_v3_units_json();
            if let Some(map) = units.as_object_mut() {
                map.insert("time".to_string(), time);
            }
            meta.insert(meta::UNITS.to_string(), units);
        }
    }
    Ok(())
}

/// Scale stored numbers so `new_units` is an honest label (not a relabel).
///
/// A v3.1 frame with a clock (`time` or a positive `timestep`) or with
/// velocities and no `units.time` is an error. The missing time unit is
/// not read as `fs`. v1, v2, and v3.0 clocks still use that default.
pub fn rescale_frame_units(
    fr: &mut readcon_core::types::ConFrame,
    new_units: &serde_json::Value,
) -> Result<()> {
    require_declared_v31_time(fr)?;
    let old = fr.header.units().cloned().unwrap_or_else(default_con_units);
    let lf = dim_factor(&old, new_units, "length", "angstrom")?;
    let ef = dim_factor(&old, new_units, "energy", "eV")?;
    let tf = dim_factor(&old, new_units, "time", "fs")?;
    let mf = dim_factor(&old, new_units, "mass", "amu")?;
    // D10: pre-v3.1 velocities stay in the natural unit. The time label
    // still rescales the clock (`time`, `timestep`) and is not a velocity unit.
    // v3.1 frames that lack `units.time` never reach here when a clock or
    // velocities would otherwise pick up the `fs` default.
    let vel_tf = if velocity_time_is_declared(&fr.header) {
        tf
    } else {
        1.0
    };
    let vf = lf / vel_tf;
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
            if let Some(mut v) = a.velocity() {
                if vf != 1.0 {
                    v[0] *= vf;
                    v[1] *= vf;
                    v[2] *= vf;
                    a.set_velocity(Some(v));
                }
            }
            if let Some(mut f) = a.force() {
                if ff != 1.0 {
                    f[0] *= ff;
                    f[1] *= ff;
                    f[2] *= ff;
                    a.set_force(Some(f));
                }
            }
            if let Some(e) = a.energy() {
                if ef != 1.0 {
                    a.set_energy(Some(e * ef));
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
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn aliases_to_preferred() {
        assert_eq!(canonicalize_unit("A").unwrap(), "angstrom");
        assert_eq!(canonicalize_unit("ev").unwrap(), "eV");
        assert_eq!(canonicalize_unit("femtosecond").unwrap(), "fs");
        assert_eq!(canonicalize_unit("eV/angstrom").unwrap(), "eV / angstrom");
        let u = canonicalize_units_object(serde_json::json!({"time": "femtosecond"})).unwrap();
        assert_eq!(u["time"], "fs");
        assert!(u.get("length").is_none());
    }

    fn load_fixture(name: &str) -> readcon_core::types::ConFrame {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test")
            .join(name);
        let text = std::fs::read_to_string(path).unwrap();
        readcon_core::iterators::ConFrameIterator::new(&text)
            .next()
            .unwrap()
            .unwrap()
    }

    fn mark_v31(fr: &mut readcon_core::types::ConFrame) {
        fr.header.spec_version = 3;
        fr.header
            .metadata
            .insert("con_spec_minor".into(), serde_json::json!(1));
    }

    #[test]
    fn v31_clock_without_time_is_not_femtoseconds() {
        let mut fr = load_fixture("tiny_cuh2.con");
        mark_v31(&mut fr);
        fr.header.set_time(12.5);
        fr.header
            .set_units(serde_json::json!({"length": "angstrom", "energy": "eV"}));
        let box0 = fr.header.boxl[0];
        let err = rescale_frame_units(
            &mut fr,
            &serde_json::json!({"length": "nm", "energy": "eV", "time": "ps"}),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no units.time"), "{msg}");
        assert!(msg.contains("clock"), "{msg}");
        assert!((fr.header.time().unwrap() - 12.5).abs() < 1e-15);
        assert!((fr.header.boxl[0] - box0).abs() < 1e-15);
        assert!(fr.header.unit_for("time").is_none());
    }

    #[test]
    fn v31_timestep_without_time_is_not_femtoseconds() {
        let mut fr = load_fixture("tiny_cuh2.con");
        mark_v31(&mut fr);
        fr.header.metadata.remove("time");
        fr.header.set_timestep(10.0);
        fr.header
            .set_units(serde_json::json!({"length": "angstrom", "energy": "eV", "time": "  "}));
        let err = rescale_frame_units(
            &mut fr,
            &serde_json::json!({"length": "angstrom", "energy": "eV", "time": "ps"}),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("clock"), "{msg}");
        assert!((fr.header.timestep().unwrap() - 10.0).abs() < 1e-15);
    }

    #[test]
    fn v31_velocities_without_time_are_not_femtoseconds() {
        let mut fr = load_fixture("tiny_cuh2.convel");
        mark_v31(&mut fr);
        fr.header.metadata.remove("time");
        fr.header.set_units(serde_json::json!({
            "length": "angstrom",
            "energy": "eV",
            "mass": "amu"
        }));
        let vx = fr.velocities.as_f64_row(0)[0];
        let err = rescale_frame_units(
            &mut fr,
            &serde_json::json!({"length": "nm", "energy": "eV", "time": "ps"}),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("velocities"), "{msg}");
        assert!(msg.contains("no units.time"), "{msg}");
        assert!((fr.velocities.as_f64_row(0)[0] - vx).abs() < 1e-15);
    }

    #[test]
    fn v31_declared_time_still_converts() {
        let mut fr = load_fixture("tiny_cuh2.convel");
        mark_v31(&mut fr);
        fr.header.set_time(12.5);
        fr.header.set_units(serde_json::json!({
            "length": "angstrom",
            "energy": "eV",
            "mass": "amu",
            "time": "fs"
        }));
        let vx = fr.velocities.as_f64_row(0)[0];
        let tf = readcon_core::units::unit_conversion_factor("fs", "ps").unwrap();
        rescale_frame_units(
            &mut fr,
            &serde_json::json!({
                "length": "angstrom",
                "energy": "eV",
                "mass": "amu",
                "time": "ps"
            }),
        )
        .unwrap();
        assert!((fr.header.time().unwrap() - 12.5 * tf).abs() < 1e-12);
        assert!((fr.velocities.as_f64_row(0)[0] - vx / tf).abs() < 1e-9);
    }

    #[test]
    fn v30_clock_without_time_stays_femtoseconds() {
        let mut fr = load_fixture("tiny_cuh2.con");
        fr.header.spec_version = 3;
        fr.header.metadata.remove("con_spec_minor");
        fr.header.set_time(12.5);
        fr.header
            .set_units(serde_json::json!({"length": "angstrom", "energy": "eV"}));
        let tf = readcon_core::units::unit_conversion_factor("fs", "ps").unwrap();
        rescale_frame_units(
            &mut fr,
            &serde_json::json!({"length": "angstrom", "energy": "eV", "time": "ps"}),
        )
        .unwrap();
        assert!((fr.header.time().unwrap() - 12.5 * tf).abs() < 1e-12);
    }

    fn meta_of(value: serde_json::Value) -> BTreeMap<String, serde_json::Value> {
        value.as_object().unwrap().clone().into_iter().collect()
    }

    #[test]
    fn pre31_fs_clock_scales_velocities_into_fs() {
        let mut meta = meta_of(serde_json::json!({
            "time": 12.5,
            "timestep": 1.25
        }));
        let scale = align_written_v31_time(&mut meta, 2, true).unwrap();
        let expect = readcon_core::units::unit_conversion_factor("fs", NATURAL_TIME_UNIT).unwrap();
        assert!((scale - expect).abs() < 1e-15);
        assert_eq!(meta["units"]["time"], "fs");
        assert_eq!(meta["time"], 12.5);
        assert_eq!(meta["timestep"], 1.25);
        assert!(!meta.contains_key("con_spec_minor"));
    }

    #[test]
    fn pre31_clock_without_velocities_labels_fs() {
        let mut meta = meta_of(serde_json::json!({"time": 4.0}));
        let scale = align_written_v31_time(&mut meta, 2, false).unwrap();
        assert_eq!(scale, 1.0);
        assert_eq!(meta["units"]["time"], "fs");
        assert_eq!(meta["time"], 4.0);
        assert_eq!(meta["units"]["length"], "angstrom");
    }

    #[test]
    fn pre31_velocities_without_clock_keep_natural() {
        let mut meta = meta_of(serde_json::json!({}));
        let scale = align_written_v31_time(&mut meta, 2, true).unwrap();
        assert_eq!(scale, 1.0);
        assert_eq!(meta["units"]["time"], NATURAL_TIME_UNIT);
    }

    #[test]
    fn pre31_ps_clock_scales_velocities_into_ps() {
        let mut meta = meta_of(serde_json::json!({
            "time": 0.5,
            "units": {"length": "angstrom", "energy": "eV", "time": "ps"}
        }));
        let scale = align_written_v31_time(&mut meta, 2, true).unwrap();
        let expect = readcon_core::units::unit_conversion_factor("ps", NATURAL_TIME_UNIT).unwrap();
        assert!((scale - expect).abs() < 1e-12);
        assert_eq!(meta["units"]["time"], "ps");
        assert_eq!(meta["time"], 0.5);
        assert_eq!(meta["units"]["length"], "angstrom");
    }

    #[test]
    fn pre31_femtosecond_label_is_the_clock_unit() {
        let mut meta = meta_of(serde_json::json!({
            "time": 3.0,
            "units": {"length": "angstrom", "energy": "eV", "time": "femtosecond"}
        }));
        let scale = align_written_v31_time(&mut meta, 3, true).unwrap();
        assert_eq!(meta["units"]["time"], "fs");
        let expect = readcon_core::units::unit_conversion_factor("fs", NATURAL_TIME_UNIT).unwrap();
        assert!((scale - expect).abs() < 1e-15);
        assert_eq!(meta["time"], 3.0);
    }

    #[test]
    fn explicit_minor_zero_with_clock_becomes_minor_one() {
        let mut meta = meta_of(serde_json::json!({
            "con_spec_minor": 0,
            "time": 1.0
        }));
        align_written_v31_time(&mut meta, 3, true).unwrap();
        assert_eq!(meta["con_spec_minor"], 1);
        assert_eq!(meta["units"]["time"], "fs");
    }

    #[test]
    fn v31_round_trip_does_not_rescale() {
        let mut meta = meta_of(serde_json::json!({
            "con_spec_minor": 1,
            "time": 12.5,
            "timestep": 1.25,
            "units": {
                "length": "angstrom",
                "energy": "eV",
                "mass": "amu",
                "time": "ps"
            }
        }));
        let scale = align_written_v31_time(&mut meta, 3, true).unwrap();
        assert_eq!(scale, 1.0);
        assert_eq!(meta["units"]["time"], "ps");
        assert_eq!(meta["time"], 12.5);
        assert_eq!(meta["timestep"], 1.25);
    }

    #[test]
    fn v31_clock_without_time_is_not_relabelled_fs() {
        let mut meta = meta_of(serde_json::json!({
            "con_spec_minor": 1,
            "time": 12.5,
            "units": {"length": "angstrom", "energy": "eV"}
        }));
        let err = align_written_v31_time(&mut meta, 3, false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("clock"), "{msg}");
        assert!(msg.contains("no units.time"), "{msg}");
        assert!(meta["units"].get("time").is_none());
        assert_eq!(meta["time"], 12.5);
    }

    #[test]
    fn frame_without_clock_or_velocities_invents_no_time() {
        let mut meta = meta_of(serde_json::json!({}));
        let scale = align_written_v31_time(&mut meta, 2, false).unwrap();
        assert_eq!(scale, 1.0);
        assert!(!meta.contains_key("units"));
    }
}
