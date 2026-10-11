//! Behavioral checks against the public corpus API.
//!
//! Each check records pass or fail. A failure is reported; it is not repaired.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use readcon_core::index_proj::{frame_byte_spans_skip, frame_fmax, FrameIndexProjection};
use readcon_core::iterators::ConFrameIterator;
use readcon_core::types::{ConFrame, ConFrameBuilder};
use readcon_core::writer::ConFrameWriter;
use readcon_db::{hash_frame_bytes, ConCorpus, FrameKey, Select};
use serde_json::{json, Value};

pub fn run(work: &Path) -> Value {
    let _ = fs::remove_dir_all(work);
    fs::create_dir_all(work).expect("work dir");
    let mut checks: Vec<Value> = Vec::new();

    let corpus_dir = work.join("corpus");
    let text = build_feature_text();
    let db = ConCorpus::open(&corpus_dir).expect("open");
    let n1 = db
        .append_trajectory_str(1, &text, "feature")
        .expect("append feature");
    let n2 = db
        .append_trajectory_str(2, &text, "feature-copy")
        .expect("append copy");
    assert_eq!(n1, n2);

    checks.push(check_round_trip(&db, &text, n1));
    checks.push(check_dedup(&db, n1));
    checks.push(check_one_byte(&db, &text));
    checks.push(check_missing_sections(&db));
    checks.push(check_periodicity(&db));

    let queries_before = query_snapshot(&db);
    let dump_before = db.sorted_logical_dump().expect("dump");
    let n_re = db.reindex().expect("reindex");
    let dump_after = db.sorted_logical_dump().expect("dump after");
    let queries_after = query_snapshot(&db);
    checks.push(check_rebuild(n_re, &dump_before, &dump_after, &queries_before, &queries_after));

    let dump_third = {
        db.reindex().expect("reindex 2");
        db.sorted_logical_dump().expect("dump 3")
    };
    checks.push(json_check(
        "reindex_twice",
        dump_third == dump_after,
        if dump_third == dump_after {
            "second reindex left the logical dump unchanged".into()
        } else {
            format!(
                "second reindex changed the dump ({})",
                diff_summary(&dump_after, &dump_third)
            )
        },
    ));

    checks.push(check_cooked(&db, &dump_after, &queries_after));
    drop(db);

    checks.push(check_hash_survives_rewrite(work));

    let n_pass = checks.iter().filter(|c| c["pass"] == true).count();
    json!({
        "n_checks": checks.len(),
        "n_pass": n_pass,
        "n_fail": checks.len() - n_pass,
        "checks": checks,
    })
}

fn json_check(name: &str, pass: bool, detail: String) -> Value {
    json!({"name": name, "pass": pass, "detail": detail})
}

fn build_feature_text() -> String {
    let specs = feature_specs();
    let mut frames = Vec::new();
    for (i, spec) in specs.iter().enumerate() {
        frames.push(build_frame(i, spec));
    }
    write_frames(&frames)
}

struct Spec {
    symbols: &'static [(&'static str, f64)],
    energy: Option<f64>,
    pbc: Option<[bool; 3]>,
    forces: bool,
    velocities: bool,
}

fn feature_specs() -> Vec<Spec> {
    let cu = &[("Cu", 63.546), ("Cu", 63.546)][..];
    let hh = &[("H", 1.008), ("H", 1.008)][..];
    vec![
        Spec { symbols: cu, energy: Some(-3.5), pbc: None, forces: false, velocities: false },
        Spec { symbols: cu, energy: Some(-3.4), pbc: None, forces: true, velocities: false },
        Spec { symbols: cu, energy: None, pbc: None, forces: false, velocities: true },
        Spec { symbols: cu, energy: Some(-3.2), pbc: None, forces: true, velocities: true },
        Spec { symbols: cu, energy: Some(-3.1), pbc: Some([true, true, false]), forces: true, velocities: false },
        Spec { symbols: cu, energy: Some(-3.0), pbc: Some([true, true, true]), forces: true, velocities: false },
        Spec { symbols: cu, energy: Some(-2.9), pbc: Some([false, false, false]), forces: true, velocities: false },
        Spec { symbols: hh, energy: None, pbc: None, forces: false, velocities: false },
        Spec { symbols: cu, energy: None, pbc: None, forces: true, velocities: false },
    ]
}

fn build_frame(index: usize, spec: &Spec) -> ConFrame {
    let mut meta = r#"{"units":{"length":"angstrom","energy":"eV","mass":"amu","time":"fs"}}"#.to_string();
    if let Some(p) = spec.pbc {
        meta = format!(
            r#"{{"units":{{"length":"angstrom","energy":"eV","mass":"amu","time":"fs"}},"pbc":[{},{},{}]}}"#,
            p[0], p[1], p[2]
        );
    }
    let mut b = ConFrameBuilder::new([12.0, 11.0, 10.0], [90.0, 90.0, 90.0]);
    b.prebox_header("cpc-eval");
    b.set_metadata_json(&meta).expect("metadata");
    let shift = index as f64 * 0.17;
    for (i, (sym, mass)) in spec.symbols.iter().enumerate() {
        let x = shift + i as f64 * 2.2;
        b.add_atom(sym, x, 0.1 * i as f64, 0.0, [false; 3], i as u64, *mass);
        if spec.forces {
            let f = if i == 0 {
                [0.3, 0.0, 0.0]
            } else {
                [-0.1, 0.4, 0.0]
            };
            b.with_force(f);
        }
        if spec.velocities {
            let v = if i == 0 {
                [0.01, 0.0, 0.0]
            } else {
                [-0.01, 0.02, 0.0]
            };
            b.with_velocity(v);
        }
    }
    if let Some(e) = spec.energy {
        b.set_energy(e);
    }
    b.build().expect("build frame")
}

fn write_frames(frames: &[ConFrame]) -> String {
    let mut buf = Vec::new();
    {
        let mut w = ConFrameWriter::with_precision(&mut buf, 12).canonical(true);
        for fr in frames {
            w.write_frame(fr).expect("write");
        }
    }
    String::from_utf8(buf).expect("utf8")
}

fn check_round_trip(db: &ConCorpus, text: &str, n: u32) -> Value {
    let spans = match frame_byte_spans_skip(text) {
        Ok(s) => s,
        Err(e) => {
            return json_check("round_trip", false, format!("span table: {e}"));
        }
    };
    if spans.len() != n as usize {
        return json_check(
            "round_trip",
            false,
            format!("span count {} != stored frames {n}", spans.len()),
        );
    }
    let mut bad = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        let source = span.slice(text).unwrap_or("");
        let stored = db
            .get_frame_text(FrameKey {
                traj_id: 1,
                frame_idx: i as u32,
            })
            .expect("text");
        if stored != source {
            bad.push(format!("frame {i} stored bytes differ from the source span"));
            continue;
        }
        let mut it = ConFrameIterator::new(&stored);
        match it.next_with_raw_span(&stored) {
            Some(Ok((_frame, raw))) if raw == stored && it.next().is_none() => {}
            _ => bad.push(format!(
                "frame {i} did not round-trip as one readcon-core span"
            )),
        }
    }
    json_check(
        "round_trip",
        bad.is_empty(),
        if bad.is_empty() {
            format!("{n} stored frames match readcon-core spans byte for byte")
        } else {
            bad.join("; ")
        },
    )
}

fn check_dedup(db: &ConCorpus, n: u32) -> Value {
    let mut notes = Vec::new();
    let mut ok = true;
    for i in 0..n {
        let k1 = FrameKey { traj_id: 1, frame_idx: i };
        let k2 = FrameKey { traj_id: 2, frame_idx: i };
        let t1 = db.get_frame_text(k1).unwrap();
        let t2 = db.get_frame_text(k2).unwrap();
        if t1 != t2 {
            ok = false;
            notes.push(format!("frame {i} copies differ"));
            continue;
        }
        let h1 = db.frame_hash(k1).unwrap();
        let h2 = db.frame_hash(k2).unwrap();
        if h1 != h2 || h1 != hash_frame_bytes(t1.as_bytes()) {
            ok = false;
            notes.push(format!("frame {i} hash mismatch"));
        }
        if db.find_by_hash(h1).unwrap() != Some(k1) {
            ok = false;
            notes.push(format!("frame {i} find_by_hash did not keep the first key"));
        }
        let selected = db
            .select(&Select::new().exact_hash(h1.to_bytes()))
            .unwrap();
        if selected != vec![k1] {
            ok = false;
            notes.push(format!("frame {i} exact_hash returned {selected:?}"));
        }
    }
    let uniq = db.unique_frame_keys(&Select::new()).unwrap();
    let keys = db.list_frame_keys().unwrap();
    // Traj 3 is added later; at this point only the two copies exist.
    if keys.len() != (n as usize) * 2 {
        ok = false;
        notes.push(format!("stored keys {} expected {}", keys.len(), n * 2));
    }
    if uniq.len() != n as usize {
        ok = false;
        notes.push(format!(
            "unique_frame_keys {} expected {n} (index collapses, blobs do not)",
            uniq.len()
        ));
    }
    if ok {
        notes.push(format!(
            "{n} duplicated frames: one hash-index entry each, {n} stored texts kept twice"
        ));
    }
    json_check("dedup_identical_bytes", ok, notes.join("; "))
}

fn check_one_byte(db: &ConCorpus, original: &str) -> Value {
    let spans = frame_byte_spans_skip(original).expect("spans");
    let frame0 = spans[0].slice(original).unwrap();
    let edited = match flip_one_digit(frame0) {
        Some(s) => s,
        None => {
            return json_check("one_byte_change", false, "could not find a coordinate digit".into());
        }
    };
    let n_diff = frame0
        .bytes()
        .zip(edited.bytes())
        .filter(|(a, b)| a != b)
        .count()
        + frame0.len().abs_diff(edited.len());
    if n_diff != 1 {
        return json_check(
            "one_byte_change",
            false,
            format!("edit changed {n_diff} bytes, expected 1"),
        );
    }
    if ConFrameIterator::new(&edited).next().is_none() {
        return json_check("one_byte_change", false, "edited frame did not parse".into());
    }
    let h_old = hash_frame_bytes(frame0.as_bytes());
    let h_new = hash_frame_bytes(edited.as_bytes());
    if h_old == h_new {
        return json_check("one_byte_change", false, "xxHash3-128 did not change".into());
    }
    db.append_trajectory_str(3, &edited, "one-byte").expect("append edit");
    let k3 = FrameKey { traj_id: 3, frame_idx: 0 };
    let stored_h = db.frame_hash(k3).unwrap();
    let found_new = db.find_by_hash(h_new).unwrap();
    let found_old = db.find_by_hash(h_old).unwrap();
    let pass = stored_h == h_new
        && found_new == Some(k3)
        && found_old == Some(FrameKey { traj_id: 1, frame_idx: 0 });
    json_check(
        "one_byte_change",
        pass,
        format!(
            "one coordinate digit changed the digest; find_by_hash(old)={found_old:?} find_by_hash(new)={found_new:?}"
        ),
    )
}

fn flip_one_digit(text: &str) -> Option<String> {
    let marker = "0.000000000000";
    let coord = text.find("Coordinates")?;
    let pos = text[coord..].find(marker)? + coord;
    let mut bytes = text.as_bytes().to_vec();
    // Flip the first decimal digit: 0.000... -> 0.100...
    let digit_at = pos + 2;
    if bytes.get(digit_at) != Some(&b'0') {
        return None;
    }
    bytes[digit_at] = b'1';
    String::from_utf8(bytes).ok()
}

fn check_missing_sections(db: &ConCorpus) -> Value {
    let forces = db.select(&Select::new().require_forces()).unwrap();
    let vels = db.select(&Select::new().require_velocities()).unwrap();
    let energy = db
        .select(&Select::new().energy_range(-1.0e3, 1.0e3))
        .unwrap();
    let fmax = db.select(&Select::new().fmax_range(0.0, 1.0e3)).unwrap();
    let both = db
        .select(&Select::new().require_forces().require_velocities())
        .unwrap();

    let force_ids = ids(&forces);
    let vel_ids = ids(&vels);
    let energy_ids = ids_traj(&energy, 1);
    let fmax_ids = ids_traj(&fmax, 1);
    let both_ids = ids(&both);

    // Frame index -> sections, from feature_specs order on traj 1.
    // 0 none, 1 forces, 2 vel, 3 both, 4-6 forces, 7 none, 8 forces no energy.
    let expect_forces = vec![1u32, 3, 4, 5, 6, 8];
    let expect_vel = vec![2u32, 3];
    let expect_energy = vec![0u32, 1, 3, 4, 5, 6];
    let mut notes = Vec::new();
    let mut ok = true;
    if force_ids != expect_forces {
        ok = false;
        notes.push(format!("require_forces {force_ids:?} expected {expect_forces:?}"));
    }
    if vel_ids != expect_vel {
        ok = false;
        notes.push(format!("require_velocities {vel_ids:?} expected {expect_vel:?}"));
    }
    if both_ids != vec![3] {
        ok = false;
        notes.push(format!("forces+velocities {both_ids:?} expected [3]"));
    }
    if energy_ids != expect_energy {
        ok = false;
        notes.push(format!("energy_range {energy_ids:?} expected {expect_energy:?}"));
    }
    if fmax_ids != expect_forces {
        ok = false;
        notes.push(format!("fmax_range {fmax_ids:?} expected {expect_forces:?}"));
    }
    // A wide fmax window must not pull in the velocity-only or bare frames.
    for missing in [0u32, 2, 7] {
        if fmax_ids.contains(&missing) || force_ids.contains(&missing) {
            ok = false;
            notes.push(format!("frame {missing} without forces was selected"));
        }
    }
    if !vel_ids.contains(&2) {
        ok = false;
        notes.push("velocity-only frame was not selected by require_velocities".into());
    }
    if force_ids.contains(&2) {
        ok = false;
        notes.push("velocity-only frame was selected by require_forces".into());
    }
    let fr = db
        .get_frame(FrameKey { traj_id: 1, frame_idx: 1 })
        .unwrap();
    let fm = frame_fmax(&fr).unwrap();
    let tight = db
        .select(&Select::new().fmax_range(fm, fm))
        .unwrap();
    if !tight.iter().any(|k| k.traj_id == 1 && k.frame_idx == 1) {
        ok = false;
        notes.push("exact fmax of the forces-only frame was not a hit".into());
    }
    if ok {
        notes.push(
            "frames without forces miss force and fmax indexes; frames without velocities miss the velocity flag; missing energy misses the energy index"
                .into(),
        );
    }
    json_check("missing_forces_or_velocities", ok, notes.join("; "))
}

fn check_periodicity(db: &ConCorpus) -> Value {
    let ttf = ids_traj(&db.select(&Select::new().pbc([true, true, false])).unwrap(), 1);
    let ttt = ids_traj(&db.select(&Select::new().pbc([true, true, true])).unwrap(), 1);
    let fff = ids_traj(&db.select(&Select::new().pbc([false, false, false])).unwrap(), 1);
    let mut ok = ttf == vec![4] && ttt == vec![5] && fff == vec![6];
    let mut notes = vec![format!("TTF={ttf:?} TTT={ttt:?} FFF={fff:?}")];
    // Frames 0,1,2,3,7,8 have a cell and no pbc key. They must miss every mask.
    for mask in [[true, true, true], [true, true, false], [false, false, false], [true, false, false]] {
        let hits = ids_traj(&db.select(&Select::new().pbc(mask)).unwrap(), 1);
        for bare in [0u32, 1, 2, 3, 7, 8] {
            if hits.contains(&bare) {
                ok = false;
                notes.push(format!(
                    "frame {bare} has a cell and no pbc key but matched {mask:?}"
                ));
            }
        }
    }
    if ok {
        notes.push(
            "periodicity comes only from metadata pbc; a box without that key matches no mask"
                .into(),
        );
    }
    json_check("periodicity_not_inferred", ok, notes.join("; "))
}

fn query_snapshot(db: &ConCorpus) -> BTreeMap<String, Vec<(u64, u32)>> {
    let mut out = BTreeMap::new();
    let queries = [
        ("cu2", Select::new().exact_composition("Cu:2")),
        ("h2", Select::new().exact_composition("H:2")),
        ("forces", Select::new().require_forces()),
        ("vel", Select::new().require_velocities()),
        ("energy", Select::new().energy_range(-1.0e3, 1.0e3)),
        ("fmax", Select::new().fmax_range(0.0, 1.0e3)),
        ("pbc_ttf", Select::new().pbc([true, true, false])),
        ("pbc_ttt", Select::new().pbc([true, true, true])),
        ("pbc_fff", Select::new().pbc([false, false, false])),
    ];
    for (name, sel) in queries {
        let keys = db.select(&sel).expect(name);
        out.insert(
            name.into(),
            keys.into_iter().map(|k| (k.traj_id, k.frame_idx)).collect(),
        );
    }
    out
}

fn check_rebuild(
    n_re: u32,
    before: &[String],
    after: &[String],
    q_before: &BTreeMap<String, Vec<(u64, u32)>>,
    q_after: &BTreeMap<String, Vec<(u64, u32)>>,
) -> Value {
    let dump_same = before == after;
    let query_same = q_before == q_after;
    let pass = dump_same && query_same && n_re > 0;
    let detail = if pass {
        format!(
            "reindex rebuilt {n_re} frames; logical dump and query answers match the pre-reindex corpus"
        )
    } else {
        format!(
            "reindex frames={n_re} dump_same={dump_same} query_same={query_same} {}",
            if dump_same {
                String::new()
            } else {
                diff_summary(before, after)
            }
        )
    };
    json_check("rebuild_from_text", pass, detail)
}

fn check_cooked(db: &ConCorpus, dump_before_cook: &[String], queries_before: &BTreeMap<String, Vec<(u64, u32)>>) -> Value {
    let key = FrameKey { traj_id: 1, frame_idx: 1 };
    let text_before = db.get_frame_text(key).unwrap();
    let hash_before = db.frame_hash(key).unwrap();
    let n_cooked = db.recook_all().expect("recook");
    let text_after = db.get_frame_text(key).unwrap();
    let hash_after = db.frame_hash(key).unwrap();
    let soa = db.get_cooked_soa_bytes(key).unwrap();
    let queries_mid = query_snapshot(db);
    let dump_mid = db.sorted_logical_dump().expect("dump cooked");
    db.reindex().expect("reindex cooked");
    let hash_re = db.frame_hash(key).unwrap();
    let text_re = db.get_frame_text(key).unwrap();
    let soa_re = db.get_cooked_soa_bytes(key).unwrap();
    let queries_re = query_snapshot(db);
    let dump_re = db.sorted_logical_dump().expect("dump reindex cooked");

    let idx_before = strip_soa(dump_before_cook);
    let idx_mid = strip_soa(&dump_mid);
    let idx_re = strip_soa(&dump_re);
    let mut notes = Vec::new();
    let mut ok = true;
    if text_after != text_before || text_re != text_before {
        ok = false;
        notes.push("CON text changed when the numeric cache was written or rebuilt".into());
    }
    if hash_after != hash_before || hash_re != hash_before {
        ok = false;
        notes.push("frame hash changed when the numeric cache was written or rebuilt".into());
    }
    if hash_before != hash_frame_bytes(text_before.as_bytes()) {
        ok = false;
        notes.push("frame_hash is not xxHash3-128 of the CON bytes".into());
    }
    match &soa {
        Some(b) if !b.is_empty() && b != text_before.as_bytes() => {}
        _ => {
            ok = false;
            notes.push("cooked payload missing or identical to the CON text".into());
        }
    }
    if soa_re.as_ref().map(|b| b.len()) != soa.as_ref().map(|b| b.len()) {
        ok = false;
        notes.push("reindex dropped or rewrote the numeric cache".into());
    }
    if idx_mid != idx_before {
        ok = false;
        notes.push(format!(
            "indexes changed when cooking ({})",
            diff_summary(&idx_before, &idx_mid)
        ));
    }
    if idx_re != idx_before {
        ok = false;
        notes.push(format!(
            "reindex after cook changed indexes ({})",
            diff_summary(&idx_before, &idx_re)
        ));
    }
    if &queries_mid != queries_before || &queries_re != queries_before {
        ok = false;
        notes.push("query answers changed after cook or the following reindex".into());
    }
    db.delete_cooked_soa(key).expect("delete one cooked");
    if db.frame_hash(key).unwrap() != hash_before || db.get_frame_text(key).unwrap() != text_before {
        ok = false;
        notes.push("deleting the numeric cache changed the hash or the text".into());
    }
    if ok {
        notes.push(format!(
            "cooked {n_cooked} frames ({soa_len} bytes on one frame); hash, CON text, secondary indexes, and query answers stayed put across cook and reindex",
            soa_len = soa.as_ref().map(|b| b.len()).unwrap_or(0)
        ));
    }
    json_check("numeric_cache_ignored", ok, notes.join("; "))
}

fn strip_soa(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| l.split('\t').next() != Some("frames_soa"))
        .cloned()
        .collect()
}

fn check_hash_survives_rewrite(work: &Path) -> Value {
    let dir = work.join("rewrite");
    let db = ConCorpus::open(&dir).expect("open rewrite");
    let text = {
        let frame = build_frame(0, &feature_specs()[0]);
        write_frames(&[frame])
    };
    db.append_trajectory_str(1, &text, "a").unwrap();
    db.append_trajectory_str(2, &text, "b").unwrap();
    let k1 = FrameKey { traj_id: 1, frame_idx: 0 };
    let k2 = FrameKey { traj_id: 2, frame_idx: 0 };
    let h = db.frame_hash(k1).unwrap();
    let rewrite = db.set_trajectory_units(
        1,
        serde_json::json!({
            "length": "bohr",
            "energy": "eV",
            "mass": "amu",
            "time": "fs"
        }),
    );
    let rewrite_ok = match &rewrite {
        Ok(_) => true,
        Err(e) => {
            return json_check(
                "hash_index_survives_rewrite_of_duplicate",
                false,
                format!("set_trajectory_units failed: {}", scrub(&e.to_string())),
            );
        }
    };
    let _ = rewrite_ok;
    let h1 = db.frame_hash(k1).unwrap();
    let h2 = db.frame_hash(k2).unwrap();
    let t2 = db.get_frame_text(k2).unwrap();
    let found = db.find_by_hash(h).unwrap();
    let text_unchanged = t2 == text && h2 == h && h2 == hash_frame_bytes(t2.as_bytes());
    let survivor = found == Some(k2);
    let first_changed = h1 != h;
    // reindex should be able to restore a first-key posting from the surviving text
    db.reindex().unwrap();
    let found_after = db.find_by_hash(h).unwrap();
    let pass = text_unchanged && survivor && first_changed && found_after == Some(k2);
    let detail = format!(
        "after rewriting traj 1, identical traj 2 still stored={text_unchanged}, find_by_hash returns {found:?} (expected the surviving key), after reindex {found_after:?}"
    );
    json_check("hash_index_survives_rewrite_of_duplicate", pass, detail)
}

fn ids(keys: &[FrameKey]) -> Vec<u32> {
    let mut v: Vec<u32> = keys
        .iter()
        .filter(|k| k.traj_id == 1)
        .map(|k| k.frame_idx)
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn ids_traj(keys: &[FrameKey], traj: u64) -> Vec<u32> {
    let mut v: Vec<u32> = keys
        .iter()
        .filter(|k| k.traj_id == traj)
        .map(|k| k.frame_idx)
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn diff_summary(a: &[String], b: &[String]) -> String {
    let ca = counts(a);
    let cb = counts(b);
    let mut names: BTreeMap<&str, ()> = BTreeMap::new();
    for k in ca.keys().chain(cb.keys()) {
        names.insert(k, ());
    }
    let mut parts = Vec::new();
    for name in names.keys() {
        let na = ca.get(*name).copied().unwrap_or(0);
        let nb = cb.get(*name).copied().unwrap_or(0);
        if na != nb {
            parts.push(format!("{name} {na}->{nb}"));
        }
    }
    if parts.is_empty() {
        format!("same per-database counts, {} lines differ", a.len().max(b.len()))
    } else {
        parts.join(", ")
    }
}

fn counts(lines: &[String]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for line in lines {
        let name = line.split('\t').next().unwrap_or("").to_string();
        *m.entry(name).or_insert(0) += 1;
    }
    m
}

fn scrub(s: &str) -> String {
    s.split_whitespace()
        .map(|w| if w.starts_with('/') { "PATH" } else { w })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Silence unused-import if a projection helper is only used as a compile check.
#[allow(dead_code)]
fn _proj(frame: &ConFrame) -> FrameIndexProjection {
    FrameIndexProjection::from_frame(frame)
}
