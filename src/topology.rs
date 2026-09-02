//! d-SEAMS topology fingerprint: parse `seams fingerprint --format json` and resolve the binary.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Default bonded-graph name passed to `seams fingerprint --graph`.
pub const DEFAULT_TOPO_GRAPH: &str = "cutoff";
/// Default hop count passed to `seams fingerprint --hops`.
pub const DEFAULT_TOPO_HOPS: u32 = 2;

/// Per-corpus knobs recorded on every annotated trajectory.
#[derive(Clone, Debug, PartialEq)]
pub struct TopoParams {
    pub cutoff: f64,
    pub graph: String,
    pub hops: u32,
    pub method: String,
}

/// Options for [`crate::ConCorpus::annotate_topology`].
#[derive(Clone, Debug, Default)]
pub struct AnnotateTopologyOpts {
    /// Neighbour cutoff in angstrom (required; per-corpus).
    pub cutoff: f64,
    /// Bond graph (`cutoff`, `knn`, `knn-union`). Default [`DEFAULT_TOPO_GRAPH`].
    pub graph: Option<String>,
    /// Bonds from the centre in each local key. Default [`DEFAULT_TOPO_HOPS`].
    pub hops: Option<u32>,
    /// Explicit `seams` binary. Else `SEAMS`, else `PATH`.
    pub seams: Option<PathBuf>,
}

impl AnnotateTopologyOpts {
    pub fn new(cutoff: f64) -> Self {
        Self {
            cutoff,
            graph: None,
            hops: None,
            seams: None,
        }
    }

    pub fn resolved_graph(&self) -> &str {
        self.graph.as_deref().unwrap_or(DEFAULT_TOPO_GRAPH)
    }

    pub fn resolved_hops(&self) -> u32 {
        self.hops.unwrap_or(DEFAULT_TOPO_HOPS)
    }
}

/// One frame from `seams fingerprint --format json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FingerprintRecord {
    pub frame: u32,
    pub key: String,
    pub method: String,
}

#[derive(Deserialize)]
struct SeamsJson {
    #[serde(default)]
    command: String,
    #[serde(default)]
    frame: u32,
    #[serde(default)]
    status: i32,
    #[serde(default)]
    text: String,
}

/// Token after `key` is the frame HEX; token after `method` is `nauty` or `wl`.
pub fn parse_fingerprint_text(text: &str) -> Result<(String, String)> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let method = token_after(&tokens, "method")
        .ok_or_else(|| Error::Message("seams fingerprint text missing method token".into()))?;
    let key = token_after(&tokens, "key")
        .ok_or_else(|| Error::Message("seams fingerprint text missing key token".into()))?;
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Message(format!(
            "seams fingerprint key is not hex: {key}"
        )));
    }
    if method != "nauty" && method != "wl" {
        return Err(Error::Message(format!(
            "seams fingerprint method must be nauty or wl, got {method}"
        )));
    }
    Ok((key.to_ascii_lowercase(), method.to_string()))
}

fn token_after<'a>(tokens: &[&'a str], label: &str) -> Option<&'a str> {
    tokens
        .iter()
        .position(|t| *t == label)
        .and_then(|i| tokens.get(i + 1).copied())
}

/// Parse one JSON object. `text` holds the CLI line.
#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_fingerprint_object(json: &str) -> Result<FingerprintRecord> {
    let obj: SeamsJson = serde_json::from_str(json.trim())
        .map_err(|e| Error::Message(format!("seams fingerprint JSON: {e}")))?;
    if !obj.command.is_empty() && obj.command != "fingerprint" {
        return Err(Error::Message(format!(
            "seams JSON command is {}, expected fingerprint",
            obj.command
        )));
    }
    if obj.status != 0 {
        return Err(Error::Message(format!(
            "seams fingerprint status {}: {}",
            obj.status,
            obj.text.trim()
        )));
    }
    let (key, method) = parse_fingerprint_text(&obj.text)?;
    Ok(FingerprintRecord {
        frame: obj.frame,
        key,
        method,
    })
}

/// Parse one JSON object per frame (NDJSON or concatenated objects).
pub fn parse_fingerprint_stdout(stdout: &str) -> Result<Vec<FingerprintRecord>> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(Error::Message(
            "seams fingerprint produced no JSON (empty stdout)".into(),
        ));
    }
    let mut out = Vec::new();
    let stream = serde_json::Deserializer::from_str(trimmed).into_iter::<SeamsJson>();
    for item in stream {
        let obj = item.map_err(|e| Error::Message(format!("seams fingerprint JSON: {e}")))?;
        out.push(parse_one_obj(obj)?);
    }
    if out.is_empty() {
        return Err(Error::Message(
            "seams fingerprint produced no parseable JSON objects".into(),
        ));
    }
    Ok(out)
}

fn parse_one_obj(obj: SeamsJson) -> Result<FingerprintRecord> {
    if !obj.command.is_empty() && obj.command != "fingerprint" {
        return Err(Error::Message(format!(
            "seams JSON command is {}, expected fingerprint",
            obj.command
        )));
    }
    if obj.status != 0 {
        return Err(Error::Message(format!(
            "seams fingerprint status {}: {}",
            obj.status,
            obj.text.trim()
        )));
    }
    let (key, method) = parse_fingerprint_text(&obj.text)?;
    Ok(FingerprintRecord {
        frame: obj.frame,
        key,
        method,
    })
}

const SEAMS_MISSING: &str = "seams fingerprint not found; install d-SEAMS, put `seams` on PATH, set SEAMS to the binary, or pass --seams PATH";

/// Resolve the `seams` binary: explicit path, then `SEAMS`, then `PATH`.
pub fn resolve_seams(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return check_seams_binary(p);
    }
    if let Ok(raw) = env::var("SEAMS") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return check_seams_binary(Path::new(trimmed));
        }
    }
    if let Some(p) = search_path("seams") {
        return check_seams_binary(&p);
    }
    Err(Error::Message(SEAMS_MISSING.into()))
}

fn check_seams_binary(p: &Path) -> Result<PathBuf> {
    if p.is_file() {
        return Ok(p.to_path_buf());
    }
    Err(Error::Message(format!(
        "{SEAMS_MISSING} (not a file: {})",
        p.display()
    )))
}

fn search_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn write_temp_con(bytes: &[u8]) -> Result<PathBuf> {
    let mut path = env::temp_dir();
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    path.push(format!("readcon-db-topo-{}-{seq}.con", std::process::id()));
    let mut f = fs::File::create(&path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(path)
}

fn remove_temp(path: &Path) {
    let _ = fs::remove_file(path);
}

/// Run `seams fingerprint FILE --format json --cutoff C --graph G --hops H`.
pub fn fingerprint_file(
    seams: &Path,
    file: &Path,
    cutoff: f64,
    graph: &str,
    hops: u32,
) -> Result<Vec<FingerprintRecord>> {
    let output = Command::new(seams)
        .arg("fingerprint")
        .arg(file)
        .arg("--format")
        .arg("json")
        .arg("--cutoff")
        .arg(cutoff.to_string())
        .arg("--graph")
        .arg(graph)
        .arg("--hops")
        .arg(hops.to_string())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Message(SEAMS_MISSING.into())
            } else {
                Error::Message(format!("failed to run seams fingerprint: {e}"))
            }
        })?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        return Err(Error::Message(format!(
            "seams fingerprint exited {}: {} {}",
            output.status.code().unwrap_or(-1),
            err.trim(),
            out.trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_fingerprint_stdout(&stdout)
}

/// Fingerprint CON bytes via a temp file.
pub fn fingerprint_bytes(
    seams: &Path,
    bytes: &[u8],
    cutoff: f64,
    graph: &str,
    hops: u32,
) -> Result<Vec<FingerprintRecord>> {
    let path = write_temp_con(bytes)?;
    let result = fingerprint_file(seams, &path, cutoff, graph, hops);
    remove_temp(&path);
    result
}

/// Compare recorded per-traj params; `None` means unannotated.
pub fn params_agree(a: &TopoParams, b: &TopoParams) -> bool {
    a.graph == b.graph
        && a.hops == b.hops
        && a.method == b.method
        && a.cutoff.to_bits() == b.cutoff.to_bits()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"schema":"dseams.cli/v1","command":"fingerprint","frame":1,"status":0,"text":"nop 4 graph cutoff hops 2 method wl key f80da44101b1d018 classes 2 rings 3:0 4:0 5:0 6:0 7:0 top 15d364f5917bbc23=2 a5e2e97711ca4c64=2\n"}"#;

    #[test]
    fn parse_sample_json_object() {
        let rec = parse_fingerprint_object(SAMPLE).unwrap();
        assert_eq!(rec.frame, 1);
        assert_eq!(rec.key, "f80da44101b1d018");
        assert_eq!(rec.method, "wl");
    }

    #[test]
    fn parse_text_tokens() {
        let (k, m) =
            parse_fingerprint_text("nop 4 graph cutoff hops 2 method nauty key DEADBEEF classes 1")
                .unwrap();
        assert_eq!(k, "deadbeef");
        assert_eq!(m, "nauty");
    }

    #[test]
    fn parse_ndjson_two_frames() {
        let two = format!("{SAMPLE}\n{SAMPLE}");
        let recs = parse_fingerprint_stdout(&two).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].key, recs[1].key);
    }

    #[test]
    fn missing_key_errors() {
        let e = parse_fingerprint_text("nop 0 graph cutoff hops 2 method wl").unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("key"), "{msg}");
    }

    #[test]
    fn missing_seams_names_command() {
        let err = resolve_seams(Some(Path::new("/no/such/seams-binary")))
            .unwrap_err()
            .to_string();
        assert!(err.contains("seams fingerprint"), "{err}");
    }
}
