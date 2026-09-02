//! Bonded-topology identity via `seams fingerprint --format json`.
//!
//! Exact frame identity remains xxHash3 on CON bytes. The topology key is
//! coarser: the bonded graph up to relabelling (d-SEAMS frame HEX plus the
//! recorded cutoff / graph / hops / method).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Clear error when the engine binary cannot be resolved.
pub const SEAMS_MISSING: &str = "seams fingerprint: binary not found; set --seams, the SEAMS environment variable, or put the seams executable on PATH";

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Per-corpus fingerprint options. Cutoff is required; graph defaults to
/// `cutoff` and hops defaults to 2.
#[derive(Clone, Debug)]
pub struct AnnotateTopologyOpts {
    pub cutoff: f64,
    pub graph: String,
    pub hops: u32,
    pub seams: Option<PathBuf>,
}

impl AnnotateTopologyOpts {
    pub fn new(cutoff: f64) -> Self {
        Self {
            cutoff,
            graph: "cutoff".into(),
            hops: 2,
            seams: None,
        }
    }

    pub fn graph(mut self, graph: impl Into<String>) -> Self {
        self.graph = graph.into();
        self
    }

    pub fn hops(mut self, hops: u32) -> Self {
        self.hops = hops;
        self
    }

    pub fn seams(mut self, path: impl Into<PathBuf>) -> Self {
        self.seams = Some(path.into());
        self
    }
}

/// One parsed `seams fingerprint` object (frame HEX + method).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FingerprintRecord {
    pub frame: i64,
    pub key: String,
    pub method: String,
}

/// Recorded per-corpus topology parameters (every annotated traj must agree).
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyParams {
    pub cutoff: Option<f64>,
    pub graph: Option<String>,
    pub hops: Option<u32>,
    pub method: Option<String>,
}

impl TopologyParams {
    pub fn is_empty(&self) -> bool {
        self.cutoff.is_none()
            && self.graph.is_none()
            && self.hops.is_none()
            && self.method.is_none()
    }

    pub fn agrees(&self, other: &Self) -> bool {
        self.cutoff == other.cutoff
            && self.graph == other.graph
            && self.hops == other.hops
            && self.method == other.method
    }

    pub fn cutoff_or_err(&self) -> Result<f64> {
        self.cutoff.ok_or_else(|| {
            Error::Message("recorded topology parameters lack cutoff".into())
        })
    }

    pub fn graph_or_default(&self) -> String {
        self.graph.clone().unwrap_or_else(|| "cutoff".into())
    }

    pub fn hops_or_default(&self) -> u32 {
        self.hops.unwrap_or(2)
    }
}

#[derive(Deserialize)]
struct SeamsJson {
    command: Option<String>,
    frame: Option<i64>,
    status: i64,
    text: String,
}

/// Lowercase hex digits only; used as the `idx_topo` prefix.
pub fn normalize_topo_hex(raw: &str) -> Result<String> {
    let s = raw.trim();
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Message(format!("invalid topology key {raw}")));
    }
    Ok(s.to_ascii_lowercase())
}

/// Strip CSI / OSC color sequences so token scans stay stable.
pub fn strip_ansi(s: &str) -> String {
    if !s.contains('\u{1b}') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                i += 2;
                while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                continue;
            }
            if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                i += 2;
                while i < bytes.len() && bytes[i] != 0x07 && bytes[i] != b'\\' {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                continue;
            }
        }
        let rest = &s[i..];
        match rest.chars().next() {
            Some(ch) => {
                out.push(ch);
                i += ch.len_utf8();
            }
            None => break,
        }
    }
    out
}

/// Token after `key` is the frame HEX; token after `method` is nauty or wl.
pub fn parse_fingerprint_text(text: &str) -> Result<(String, String)> {
    let cleaned = strip_ansi(text);
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    let mut key = None;
    let mut method = None;
    let mut i = 0;
    while i + 1 < tokens.len() {
        match tokens[i] {
            "key" => {
                key = Some(tokens[i + 1].to_owned());
                i += 2;
            }
            "method" => {
                method = Some(tokens[i + 1].to_owned());
                i += 2;
            }
            _ => i += 1,
        }
    }
    let key = key.ok_or_else(|| {
        Error::Message(format!(
            "seams fingerprint text missing key token: {cleaned}"
        ))
    })?;
    let method = method.ok_or_else(|| {
        Error::Message(format!(
            "seams fingerprint text missing method token: {cleaned}"
        ))
    })?;
    let key = normalize_topo_hex(&key)?;
    Ok((key, method))
}

/// One JSON object from `seams fingerprint --format json`.
pub fn parse_fingerprint_json_line(line: &str) -> Result<FingerprintRecord> {
    let rec: SeamsJson = serde_json::from_str(line.trim()).map_err(|e| {
        Error::Message(format!("seams fingerprint json: {e}: {line}"))
    })?;
    if rec.status != 0 {
        return Err(Error::Message(format!(
            "seams fingerprint status {}: {}",
            rec.status, rec.text
        )));
    }
    if let Some(cmd) = rec.command.as_deref() {
        if cmd != "fingerprint" {
            return Err(Error::Message(format!(
                "seams fingerprint unexpected command {cmd}"
            )));
        }
    }
    let (key, method) = parse_fingerprint_text(&rec.text)?;
    Ok(FingerprintRecord {
        frame: rec.frame.unwrap_or(0),
        key,
        method,
    })
}

/// Parse every JSON object in `seams fingerprint --format json` stdout.
pub fn parse_fingerprint_json_stdout(stdout: &str) -> Result<Vec<FingerprintRecord>> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        out.push(parse_fingerprint_json_line(line)?);
    }
    if out.is_empty() {
        return Err(Error::Message(
            "seams fingerprint produced no JSON objects".into(),
        ));
    }
    Ok(out)
}

fn search_path(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let p = PathBuf::from(name);
        return p.is_file().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// Resolve the engine from `--seams`, `SEAMS`, or `PATH`.
pub fn resolve_seams_binary(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        return Err(Error::Message(format!(
            "{SEAMS_MISSING} (not a file: {})",
            p.display()
        )));
    }
    if let Ok(val) = std::env::var("SEAMS") {
        if !val.is_empty() {
            let p = PathBuf::from(&val);
            if p.is_file() {
                return Ok(p);
            }
            if let Some(found) = search_path(&val) {
                return Ok(found);
            }
            return Err(Error::Message(SEAMS_MISSING.into()));
        }
    }
    search_path("seams").ok_or_else(|| Error::Message(SEAMS_MISSING.into()))
}

struct TempCon {
    path: PathBuf,
}

impl Drop for TempCon {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn write_temp_con(bytes: &[u8]) -> Result<TempCon> {
    let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "readcon-db-topo-{}-{id}.con",
        std::process::id()
    ));
    std::fs::write(&path, bytes)?;
    Ok(TempCon { path })
}

/// Run `seams fingerprint FILE --format json --cutoff C --graph G --hops H`.
pub fn run_seams_fingerprint(
    seams: &Path,
    file: &Path,
    cutoff: f64,
    graph: &str,
    hops: u32,
) -> Result<Vec<FingerprintRecord>> {
    let file_s = file.to_str().ok_or(Error::Nul)?;
    let cutoff_s = cutoff.to_string();
    let hops_s = hops.to_string();
    let output = Command::new(seams)
        .args([
            "fingerprint",
            file_s,
            "--format",
            "json",
            "--cutoff",
            &cutoff_s,
            "--graph",
            graph,
            "--hops",
            &hops_s,
        ])
        .output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        return Err(Error::Message(format!(
            "seams fingerprint failed (exit {:?}): {err}{out}",
            output.status.code()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_fingerprint_json_stdout(&stdout)
}

/// Fingerprint CON bytes by writing a temp file for the engine.
pub fn fingerprint_con_bytes(
    seams: &Path,
    bytes: &[u8],
    cutoff: f64,
    graph: &str,
    hops: u32,
) -> Result<Vec<FingerprintRecord>> {
    let tmp = write_temp_con(bytes)?;
    run_seams_fingerprint(seams, &tmp.path, cutoff, graph, hops)
}

/// Mixed-parameter error used by annotate and reindex.
pub fn mixed_topo_error(a: &TopologyParams, b: &TopologyParams) -> Error {
    Error::Message(format!(
        "mixed topology parameters: do not mix cutoff/graph/hops/method (have cutoff={:?} graph={:?} hops={:?} method={:?}; saw cutoff={:?} graph={:?} hops={:?} method={:?})",
        a.cutoff, a.graph, a.hops, a.method, b.cutoff, b.graph, b.hops, b.method
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_text_key_and_method() {
        let text =
            "nop 4 graph cutoff hops 2 method nauty key abcdef0123 classes 2 rings 3:0 4:1 top abc=2";
        let (key, method) = parse_fingerprint_text(text).unwrap();
        assert_eq!(key, "abcdef0123");
        assert_eq!(method, "nauty");
    }

    #[test]
    fn parse_text_strips_ansi_and_normalizes_hex() {
        let text = "nop 4 \u{1b}[1;36mmethod\u{1b}[0m WL \u{1b}[1;36mkey\u{1b}[0m DEADBEEF";
        let (key, method) = parse_fingerprint_text(text).unwrap();
        assert_eq!(key, "deadbeef");
        assert_eq!(method, "WL");
    }

    #[test]
    fn parse_json_object_per_frame() {
        let line = r#"{"schema":"dseams.cli/v1","command":"fingerprint","frame":1,"status":0,"text":"nop 4 graph cutoff hops 2 method nauty key 00ff classes 1 rings 3:0 top 00ff=4"}"#;
        let rec = parse_fingerprint_json_line(line).unwrap();
        assert_eq!(rec.frame, 1);
        assert_eq!(rec.key, "00ff");
        assert_eq!(rec.method, "nauty");
    }

    #[test]
    fn parse_json_rejects_nonzero_status() {
        let line = r#"{"schema":"dseams.cli/v1","command":"fingerprint","frame":1,"status":2,"text":"boom"}"#;
        let err = parse_fingerprint_json_line(line).unwrap_err().to_string();
        assert!(err.contains("status 2"), "{err}");
    }

    #[test]
    fn parse_json_stdout_two_frames() {
        let stdout = r#"{"schema":"dseams.cli/v1","command":"fingerprint","frame":1,"status":0,"text":"nop 1 graph cutoff hops 2 method nauty key aa classes 1 rings 3:0 top aa=1"}
{"schema":"dseams.cli/v1","command":"fingerprint","frame":2,"status":0,"text":"nop 1 graph cutoff hops 2 method nauty key bb classes 1 rings 3:0 top bb=1"}
"#;
        let recs = parse_fingerprint_json_stdout(stdout).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].key, "aa");
        assert_eq!(recs[1].key, "bb");
    }

    #[test]
    fn normalize_hex_rejects_garbage() {
        assert!(normalize_topo_hex("not-hex").is_err());
        assert!(normalize_topo_hex("").is_err());
        assert_eq!(normalize_topo_hex("AbC").unwrap(), "abc");
    }

    #[test]
    fn resolve_missing_seams_names_command() {
        let err = resolve_seams_binary(Some(Path::new("/no/such/seams-binary")))
            .unwrap_err()
            .to_string();
        assert!(err.contains("seams fingerprint"), "{err}");
    }

    #[test]
    fn params_agree_requires_all_fields() {
        let a = TopologyParams {
            cutoff: Some(3.0),
            graph: Some("cutoff".into()),
            hops: Some(2),
            method: Some("nauty".into()),
        };
        let mut b = a.clone();
        assert!(a.agrees(&b));
        b.method = Some("wl".into());
        assert!(!a.agrees(&b));
    }
}
