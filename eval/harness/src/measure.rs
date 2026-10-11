//! Oracle, ingest, query, scan, and dedup measurements.
//!
//! Timers wrap the call and a check of the hit count and the sum of frame
//! indexes. A mismatch is a failed measurement, not a silent number.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::Instant;

use readcon_core::index_proj::FrameIndexProjection;
use readcon_core::iterators::ConFrameIterator;
use readcon_db::{hash_frame_bytes, ConCorpus, FrameKey, Select};
use serde_json::{json, Value};

pub fn oracle(chunks: &Path, all: &Path) -> Result<Value, String> {
    let files = con_files(chunks)?;
    if files.is_empty() {
        return Err("no CON files".into());
    }
    concat_matches(all, &files)?;

    let mut formula_counts: Vec<(String, u64)> = Vec::new();
    let mut energies: Vec<(u64, f64)> = Vec::new();
    let mut fmaxes: Vec<(u64, f64, f64)> = Vec::new();
    let mut hashes: BTreeSet<[u8; 16]> = BTreeSet::new();
    let mut n_frames: u64 = 0;
    let mut raw_bytes: u64 = 0;

    for path in &files {
        let text = fs::read_to_string(path).map_err(|e| scrub(&e.to_string()))?;
        raw_bytes += text.len() as u64;
        let mut it = ConFrameIterator::new(&text);
        loop {
            let (frame, span) = match it.next_with_raw_span(&text) {
                None => break,
                Some(Err(e)) => return Err(format!("parse: {e}")),
                Some(Ok(pair)) => pair,
            };
            let proj = FrameIndexProjection::from_frame(&frame);
            let idx = n_frames;
            n_frames += 1;
            hashes.insert(hash_frame_bytes(span.as_bytes()).to_bytes());
            bump_formula(&mut formula_counts, &proj.formula);
            if let Some(e) = proj.energy {
                energies.push((idx, e));
            }
            if let (Some(fr), Some(fa)) = (proj.fmax, fmax_sum_of_squares(&frame)) {
                fmaxes.push((idx, fr, fa));
            }
        }
    }
    if n_frames == 0 {
        return Err("zero frames".into());
    }

    formula_counts.sort_by(|a, b| a.0.cmp(&b.0));
    let composition = choose_formula(&formula_counts, n_frames)?;
    let (comp_n, comp_sum) = tally_formula(&files, &composition.formula)?;
    let counted = formula_counts
        .iter()
        .find(|(f, _)| f == &composition.formula)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    if counted != comp_n {
        return Err(format!("composition tally {comp_n} != count {counted}"));
    }

    let energy = energy_window(&energies)?;
    let fmax = fmax_window(&fmaxes)?;

    Ok(json!({
        "n_frames": n_frames,
        "n_files": files.len(),
        "raw_bytes": raw_bytes,
        "n_unique_hashes": hashes.len(),
        "composition": {
            "formula": composition.formula,
            "counts": composition.counts,
            "n": comp_n,
            "index_sum": comp_sum,
        },
        "energy": energy,
        "fmax": fmax,
    }))
}

struct ChosenFormula {
    formula: String,
    counts: Vec<(String, u32)>,
}

fn choose_formula(counts: &[(String, u64)], n_frames: u64) -> Result<ChosenFormula, String> {
    let mut best: Option<(i64, String, u64)> = None;
    let target = (n_frames as f64 * 0.4).round() as i64;
    for (formula, n) in counts {
        if formula.is_empty() {
            continue;
        }
        let frac = *n as f64 / n_frames as f64;
        if !(0.05..0.85).contains(&frac) {
            continue;
        }
        let dist = (*n as i64 - target).abs();
        let replace = match &best {
            None => true,
            Some((d, name, _)) => dist < *d || (dist == *d && formula.as_str() < name.as_str()),
        };
        if replace {
            best = Some((dist, formula.clone(), *n));
        }
    }
    let Some((_, formula, _)) = best else {
        return Err("no composition is selective enough to query".into());
    };
    let spec = parse_formula(&formula)?;
    Ok(ChosenFormula {
        formula,
        counts: spec,
    })
}

fn parse_formula(formula: &str) -> Result<Vec<(String, u32)>, String> {
    let mut out = Vec::new();
    for part in formula.split('|') {
        let (sym, n) = part
            .split_once(':')
            .ok_or_else(|| format!("bad formula token {part}"))?;
        let n: u32 = n
            .parse()
            .map_err(|_| format!("bad count in {part}"))?;
        out.push((sym.to_string(), n));
    }
    Ok(out)
}

fn bump_formula(counts: &mut Vec<(String, u64)>, formula: &str) {
    if let Some((_, n)) = counts.iter_mut().find(|(f, _)| f == formula) {
        *n += 1;
    } else {
        counts.push((formula.to_string(), 1));
    }
}

fn tally_formula(files: &[std::path::PathBuf], formula: &str) -> Result<(u64, u64), String> {
    let mut n = 0u64;
    let mut sum = 0u64;
    let mut idx = 0u64;
    for path in files {
        let text = fs::read_to_string(path).map_err(|e| scrub(&e.to_string()))?;
        let mut it = ConFrameIterator::new(&text);
        while let Some(item) = it.next() {
            let frame = item.map_err(|e| format!("parse: {e}"))?;
            let proj = FrameIndexProjection::from_frame(&frame);
            if proj.formula == formula {
                n += 1;
                sum += idx;
            }
            idx += 1;
        }
    }
    Ok((n, sum))
}

fn energy_window(rows: &[(u64, f64)]) -> Result<Value, String> {
    if rows.len() < 8 {
        return Err("too few energies for a range query".into());
    }
    let mut vals: Vec<f64> = rows.iter().map(|(_, e)| *e).collect();
    vals.sort_by(|a, b| a.total_cmp(b));
    let (lo, hi) = gapped_bounds(&vals)?;
    let mut n = 0u64;
    let mut sum = 0u64;
    for (idx, e) in rows {
        if *e >= lo && *e <= hi {
            n += 1;
            sum += *idx;
        }
    }
    if n == 0 || n == rows.len() as u64 {
        return Err(format!("energy window hit {n} of {} finite energies", rows.len()));
    }
    Ok(json!({
        "min_bits": bits(lo),
        "max_bits": bits(hi),
        "n": n,
        "index_sum": sum,
        "n_finite": rows.len(),
    }))
}

fn fmax_window(rows: &[(u64, f64, f64)]) -> Result<Value, String> {
    if rows.len() < 8 {
        return Err("too few forces for a max-force query".into());
    }
    let mut vals: Vec<f64> = rows.iter().map(|(_, fr, _)| *fr).collect();
    vals.sort_by(|a, b| a.total_cmp(b));
    let (lo, hi) = gapped_bounds(&vals)?;
    let mut n = 0u64;
    let mut sum = 0u64;
    let mut disagree = 0u64;
    for (idx, fr, fa) in rows {
        let in_r = *fr >= lo && *fr <= hi;
        let in_a = *fa >= lo && *fa <= hi;
        if in_r != in_a {
            disagree += 1;
        }
        if in_r {
            n += 1;
            sum += *idx;
        }
    }
    if disagree != 0 {
        return Err(format!(
            "fmax window disagrees between per-atom norms and sqrt(max sum of squares) on {disagree} frames"
        ));
    }
    if n == 0 || n == rows.len() as u64 {
        return Err(format!("fmax window hit {n} of {} frames with forces", rows.len()));
    }
    Ok(json!({
        "min_bits": bits(lo),
        "max_bits": bits(hi),
        "n": n,
        "index_sum": sum,
        "n_with_forces": rows.len(),
        "definition_disagreements": disagree,
    }))
}

/// Inclusive bounds that sit in a gap between sorted samples, so a 1 ulp
/// difference between two fmax reductions cannot flip membership.
fn gapped_bounds(sorted: &[f64]) -> Result<(f64, f64), String> {
    let n = sorted.len();
    let mut i = n * 2 / 5;
    let mut j = n * 3 / 5;
    if j <= i {
        j = (i + 1).min(n - 1);
    }
    while i > 0 && sorted[i] <= sorted[i - 1] {
        i -= 1;
    }
    if i == 0 {
        i = 1;
    }
    while j + 1 < n && sorted[j] >= sorted[j + 1] {
        j += 1;
    }
    if j + 1 >= n {
        j = n - 2;
    }
    if !(sorted[i - 1] < sorted[i] && sorted[j] < sorted[j + 1] && i <= j) {
        return Err("could not place query bounds in a gap between values".into());
    }
    let lo = midpoint(sorted[i - 1], sorted[i]);
    let hi = midpoint(sorted[j], sorted[j + 1]);
    if !(lo < hi) {
        return Err("query bounds collapsed".into());
    }
    Ok((lo, hi))
}

fn midpoint(a: f64, b: f64) -> f64 {
    a * 0.5 + b * 0.5
}

fn fmax_sum_of_squares(frame: &readcon_core::types::ConFrame) -> Option<f64> {
    let mut max_ss = 0.0f64;
    let mut any = false;
    for atom in &frame.atom_data {
        if let Some(f) = atom.force {
            let ss = f[0] * f[0] + f[1] * f[1] + f[2] * f[2];
            if ss.is_finite() && ss > max_ss {
                max_ss = ss;
            }
            if ss.is_finite() {
                any = true;
            }
        }
    }
    any.then(|| max_ss.sqrt())
}

pub fn ingest(con: &Path, dest: &Path, oracle: &Value) -> Result<Value, String> {
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| scrub(&e.to_string()))?;
    }
    let raw_bytes = fs::metadata(con).map_err(|e| scrub(&e.to_string()))?.len();
    let expected_n = oracle["n_frames"].as_u64().ok_or("oracle n_frames")?;
    let formula = oracle["composition"]["formula"]
        .as_str()
        .ok_or("oracle formula")?;
    let expect_hits = oracle["composition"]["n"].as_u64().ok_or("oracle comp n")?;
    let expect_sum = oracle["composition"]["index_sum"]
        .as_u64()
        .ok_or("oracle comp sum")?;

    let db = ConCorpus::open(dest).map_err(|e| scrub(&e.to_string()))?;
    let t0 = Instant::now();
    let n = db
        .append_trajectory_path(1, con)
        .map_err(|e| scrub(&e.to_string()))?;
    let keys = db
        .select(&Select::new().exact_composition(formula))
        .map_err(|e| scrub(&e.to_string()))?;
    let hits = keys.len() as u64;
    let sum = index_sum(&keys);
    if n as u64 != expected_n || hits != expect_hits || sum != expect_sum {
        return Err(format!(
            "ingest check failed frames {n} expected {expected_n} hits {hits} expected {expect_hits} sum {sum} expected {expect_sum}"
        ));
    }
    let seconds = t0.elapsed().as_secs_f64();
    let sizes = corpus_sizes(dest)?;
    Ok(json!({
        "ok": true,
        "seconds": seconds,
        "frames": n,
        "raw_bytes": raw_bytes,
        "frames_per_s": n as f64 / seconds,
        "mb_per_s": (raw_bytes as f64 / 1.0e6) / seconds,
        "hits": hits,
        "index_sum": sum,
        "apparent_bytes": sizes.0,
        "allocated_bytes": sizes.1,
    }))
}

pub fn query(corpus: &Path, oracle: &Value, only: Option<&str>) -> Result<Value, String> {
    let db = ConCorpus::open_readonly(corpus).map_err(|e| scrub(&e.to_string()))?;
    let cold = time_queries(&db, oracle, only)?;
    let warm = time_queries(&db, oracle, only)?;
    Ok(json!({"ok": true, "cold": cold, "warm": warm}))
}

fn time_queries(db: &ConCorpus, oracle: &Value, only: Option<&str>) -> Result<Value, String> {
    let mut out = serde_json::Map::new();
    for name in selected(only)? {
        let sel = match name {
            "composition" => composition_select(oracle)?,
            "energy" => energy_select(oracle)?,
            "fmax" => fmax_select(oracle)?,
            other => return Err(format!("unknown query {other}")),
        };
        out.insert(name.to_string(), time_one(db, oracle, name, sel)?);
    }
    Ok(Value::Object(out))
}

fn time_one(db: &ConCorpus, oracle: &Value, name: &str, sel: Select) -> Result<Value, String> {
    let expect_n = oracle[name]["n"].as_u64().ok_or("n")?;
    let expect_sum = oracle[name]["index_sum"].as_u64().ok_or("sum")?;
    let t0 = Instant::now();
    let keys = db.select(&sel).map_err(|e| scrub(&e.to_string()))?;
    let hits = keys.len() as u64;
    let sum = index_sum(&keys);
    if hits != expect_n || sum != expect_sum {
        return Err(format!(
            "{name} check failed hits {hits} expected {expect_n} sum {sum} expected {expect_sum}"
        ));
    }
    let seconds = t0.elapsed().as_secs_f64();
    Ok(json!({"seconds": seconds, "hits": hits, "index_sum": sum}))
}

pub fn scan(chunks: &Path, oracle: &Value, only: Option<&str>) -> Result<Value, String> {
    let files = con_files(chunks)?;
    let cold = scan_each(&files, oracle, only)?;
    let warm = scan_each(&files, oracle, only)?;
    Ok(json!({"ok": true, "cold": cold, "warm": warm}))
}

fn scan_each(
    files: &[std::path::PathBuf],
    oracle: &Value,
    only: Option<&str>,
) -> Result<Value, String> {
    let mut out = serde_json::Map::new();
    for name in selected(only)? {
        let pred = match name {
            "composition" => Pred::Composition,
            "energy" => Pred::Energy,
            "fmax" => Pred::Fmax,
            other => return Err(format!("unknown query {other}")),
        };
        out.insert(name.to_string(), scan_predicate(files, oracle, pred)?);
    }
    Ok(Value::Object(out))
}

fn selected(only: Option<&str>) -> Result<Vec<&str>, String> {
    match only {
        None => Ok(vec!["composition", "energy", "fmax"]),
        Some(s @ ("composition" | "energy" | "fmax")) => Ok(vec![s]),
        Some(other) => Err(format!("unknown query {other}")),
    }
}

enum Pred {
    Composition,
    Energy,
    Fmax,
}

fn scan_predicate(files: &[std::path::PathBuf], oracle: &Value, pred: Pred) -> Result<Value, String> {
    let formula = oracle["composition"]["formula"].as_str().ok_or("formula")?;
    let elo = from_bits(oracle["energy"]["min_bits"].as_str().ok_or("elo")?)?;
    let ehi = from_bits(oracle["energy"]["max_bits"].as_str().ok_or("ehi")?)?;
    let flo = from_bits(oracle["fmax"]["min_bits"].as_str().ok_or("flo")?)?;
    let fhi = from_bits(oracle["fmax"]["max_bits"].as_str().ok_or("fhi")?)?;
    let (name, expect_n, expect_sum) = match pred {
        Pred::Composition => (
            "composition",
            oracle["composition"]["n"].as_u64().unwrap(),
            oracle["composition"]["index_sum"].as_u64().unwrap(),
        ),
        Pred::Energy => (
            "energy",
            oracle["energy"]["n"].as_u64().unwrap(),
            oracle["energy"]["index_sum"].as_u64().unwrap(),
        ),
        Pred::Fmax => (
            "fmax",
            oracle["fmax"]["n"].as_u64().unwrap(),
            oracle["fmax"]["index_sum"].as_u64().unwrap(),
        ),
    };
    let t0 = Instant::now();
    let mut idx = 0u64;
    let mut n = 0u64;
    let mut sum = 0u64;
    for path in files {
        let text = fs::read_to_string(path).map_err(|e| scrub(&e.to_string()))?;
        let mut it = ConFrameIterator::new(&text);
        while let Some(item) = it.next() {
            let frame = item.map_err(|e| format!("parse: {e}"))?;
            let proj = FrameIndexProjection::from_frame(&frame);
            let hit = match pred {
                Pred::Composition => proj.formula == formula,
                Pred::Energy => proj.energy.is_some_and(|e| e >= elo && e <= ehi),
                Pred::Fmax => proj.fmax.is_some_and(|f| f >= flo && f <= fhi),
            };
            if hit {
                n += 1;
                sum += idx;
            }
            idx += 1;
        }
    }
    if n != expect_n || sum != expect_sum {
        return Err(format!(
            "{name} scan check failed hits {n} expected {expect_n} sum {sum} expected {expect_sum}"
        ));
    }
    let seconds = t0.elapsed().as_secs_f64();
    Ok(json!({"seconds": seconds, "hits": n, "index_sum": sum}))
}

pub fn dedup(con: &Path, dest: &Path) -> Result<Value, String> {
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| scrub(&e.to_string()))?;
    }
    let raw_bytes = fs::metadata(con).map_err(|e| scrub(&e.to_string()))?.len();
    let db = ConCorpus::open(dest).map_err(|e| scrub(&e.to_string()))?;
    let n = db
        .append_trajectory_path(1, con)
        .map_err(|e| scrub(&e.to_string()))? as u64;
    let keys = db.list_frame_keys().map_err(|e| scrub(&e.to_string()))?;
    let uniq = db
        .unique_frame_keys(&Select::new())
        .map_err(|e| scrub(&e.to_string()))?;
    let mut seen: BTreeSet<[u8; 16]> = BTreeSet::new();
    let mut blob_bytes = 0u64;
    let mut unique_blob_bytes = 0u64;
    let mut first_texts: Vec<String> = Vec::new();
    for key in &keys {
        let text = db.get_frame_text(*key).map_err(|e| scrub(&e.to_string()))?;
        let h = db.frame_hash(*key).map_err(|e| scrub(&e.to_string()))?;
        if h != hash_frame_bytes(text.as_bytes()) {
            return Err("stored hash is not xxHash3-128 of the CON bytes".into());
        }
        blob_bytes += text.len() as u64;
        if seen.insert(h.to_bytes()) {
            unique_blob_bytes += text.len() as u64;
            first_texts.push(text);
        }
    }
    let sizes = corpus_sizes(dest)?;
    drop(db);

    let unique_dir = dest.with_file_name("dedup-unique-only");
    if unique_dir.exists() {
        fs::remove_dir_all(&unique_dir).map_err(|e| scrub(&e.to_string()))?;
    }
    let unique_con = dest.with_file_name("dedup-unique.con");
    {
        let mut joined = String::new();
        for t in &first_texts {
            joined.push_str(t);
        }
        fs::write(&unique_con, &joined).map_err(|e| scrub(&e.to_string()))?;
    }
    let udb = ConCorpus::open(&unique_dir).map_err(|e| scrub(&e.to_string()))?;
    let un = udb
        .append_trajectory_path(1, &unique_con)
        .map_err(|e| scrub(&e.to_string()))? as u64;
    drop(udb);
    let unique_sizes = corpus_sizes(&unique_dir)?;
    let _ = fs::remove_dir_all(&unique_dir);
    let _ = fs::remove_file(&unique_con);

    Ok(json!({
        "ok": true,
        "n_frames": n,
        "n_keys": keys.len(),
        "n_unique_index": uniq.len(),
        "n_unique_hashes": seen.len(),
        "unique_reingest_frames": un,
        "raw_bytes": raw_bytes,
        "stored_blob_bytes": blob_bytes,
        "unique_blob_bytes": unique_blob_bytes,
        "logical_unique_over_stored": unique_blob_bytes as f64 / blob_bytes as f64,
        "corpus_apparent_bytes": sizes.0,
        "corpus_allocated_bytes": sizes.1,
        "unique_corpus_apparent_bytes": unique_sizes.0,
        "unique_corpus_allocated_bytes": unique_sizes.1,
    }))
}

fn composition_select(oracle: &Value) -> Result<Select, String> {
    let formula = oracle["composition"]["formula"]
        .as_str()
        .ok_or("formula")?;
    Ok(Select::new().exact_composition(formula))
}

fn energy_select(oracle: &Value) -> Result<Select, String> {
    let lo = from_bits(oracle["energy"]["min_bits"].as_str().ok_or("elo")?)?;
    let hi = from_bits(oracle["energy"]["max_bits"].as_str().ok_or("ehi")?)?;
    Ok(Select::new().energy_range(lo, hi))
}

fn fmax_select(oracle: &Value) -> Result<Select, String> {
    let lo = from_bits(oracle["fmax"]["min_bits"].as_str().ok_or("flo")?)?;
    let hi = from_bits(oracle["fmax"]["max_bits"].as_str().ok_or("fhi")?)?;
    Ok(Select::new().fmax_range(lo, hi))
}

fn index_sum(keys: &[FrameKey]) -> u64 {
    keys.iter().map(|k| k.frame_idx as u64).sum()
}

fn bits(x: f64) -> String {
    format!("{:016x}", x.to_bits())
}

fn from_bits(hex: &str) -> Result<f64, String> {
    let v = u64::from_str_radix(hex, 16).map_err(|_| format!("bad bits {hex}"))?;
    Ok(f64::from_bits(v))
}

fn corpus_sizes(dir: &Path) -> Result<(u64, u64), String> {
    let mdb = dir.join("data.mdb");
    let meta = fs::metadata(&mdb).map_err(|e| scrub(&e.to_string()))?;
    Ok((meta.len(), meta.blocks() * 512))
}

fn con_files(dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    let rd = fs::read_dir(dir).map_err(|e| scrub(&e.to_string()))?;
    for ent in rd {
        let ent = ent.map_err(|e| scrub(&e.to_string()))?;
        let path = ent.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.ends_with(".con") || name.ends_with(".convel") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn concat_matches(all: &Path, files: &[std::path::PathBuf]) -> Result<(), String> {
    let mut all_f = File::open(all).map_err(|e| scrub(&e.to_string()))?;
    for path in files {
        let data = fs::read(path).map_err(|e| scrub(&e.to_string()))?;
        let mut buf = vec![0u8; data.len()];
        all_f
            .read_exact(&mut buf)
            .map_err(|_| "concatenated CON does not match the chunk files".to_string())?;
        if buf != data {
            return Err("concatenated CON does not match the chunk files".into());
        }
    }
    let mut extra = [0u8; 1];
    let n = all_f.read(&mut extra).map_err(|e| scrub(&e.to_string()))?;
    if n != 0 {
        return Err("concatenated CON has trailing bytes".into());
    }
    Ok(())
}

fn scrub(s: &str) -> String {
    s.split_whitespace()
        .map(|w| if w.starts_with('/') { "PATH" } else { w })
        .collect::<Vec<_>>()
        .join(" ")
}
