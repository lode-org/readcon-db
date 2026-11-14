//! High-byte and full-shuffle sizes on one column set.
//!
//! The columns are the same synthetic 100k-atom frame the lever test builds:
//! positions `(i * 0.125, 0.5, 1.0)` and forces `(x * 0.5, 0.25, 0.5)`.
//! Each side of a comparison uses that set, not forces on one side and
//! positions plus forces on the other. Compression is zstd level 3 with
//! window log 12 and long-distance matching off. Times are 20 rounds after
//! 3 untimed rounds.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn require_pushed_commit() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let status = Command::new("python3")
        .arg("-c")
        .arg("import db24_drive; db24_drive.require_pushed_commit()")
        .env("PYTHONPATH", repo.join("examples/benchmarks"))
        .current_dir(&repo)
        .status()
        .expect("require_pushed_commit");
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn type7(samples: &[u128], p: f64) -> f64 {
    let mut vals = samples.to_vec();
    vals.sort_unstable();
    let n = vals.len();
    if n == 1 {
        return vals[0] as f64;
    }
    let h = (n - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = h.ceil() as usize;
    let w = h - lo as f64;
    vals[lo] as f64 * (1.0 - w) + vals[hi] as f64 * w
}

fn summarize(samples: &[u128]) -> String {
    let mean = samples.iter().sum::<u128>() / samples.len() as u128;
    let iqr = type7(samples, 0.75) - type7(samples, 0.25);
    let list = samples
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("\"mean_us\":{mean},\"iqr_us\":{iqr:.0},\"samples_us\":[{list}]")
}

fn sample(body: &mut dyn FnMut()) -> Vec<u128> {
    for _ in 0..3 {
        body();
    }
    let mut out = Vec::with_capacity(20);
    for _ in 0..20 {
        let t0 = Instant::now();
        body();
        out.push(t0.elapsed().as_micros());
    }
    out
}

fn shuffle_column(values: &[f64]) -> Vec<u8> {
    let n = values.len();
    let mut shuffled = vec![0u8; n * 8];
    for (i, value) in values.iter().enumerate() {
        let bytes = value.to_le_bytes();
        for b in 0..8 {
            shuffled[b * n + i] = bytes[b];
        }
    }
    shuffled
}

fn new_compressor() -> zstd::bulk::Compressor<'static> {
    let mut compressor = zstd::bulk::Compressor::new(3).unwrap();
    compressor
        .set_parameter(zstd::zstd_safe::CParameter::WindowLog(12))
        .unwrap();
    compressor
        .set_parameter(zstd::zstd_safe::CParameter::EnableLongDistanceMatching(
            false,
        ))
        .unwrap();
    compressor
}

fn measure(name: &str, cols: &[Vec<u8>]) -> String {
    let n = cols[0].len() / 8;
    let uncompressed = cols.len() * n * 8;
    let mut compressor = new_compressor();
    let mut parts = Vec::new();
    let mut full_bytes = 0usize;
    for col in cols {
        let part = compressor.compress(col).unwrap();
        full_bytes += part.len();
        parts.push(part);
    }
    let mut decs: Vec<_> = (0..cols.len())
        .map(|_| zstd::bulk::Decompressor::new().unwrap())
        .collect();
    let mut bufs = vec![vec![0u8; n * 8]; cols.len()];
    let full = sample(&mut || {
        for i in 0..cols.len() {
            decs[i]
                .decompress_to_buffer(&parts[i], &mut bufs[i])
                .unwrap();
        }
        std::hint::black_box(bufs[0][0]);
    });

    let mut high_bytes = 0usize;
    let mut high_parts = Vec::new();
    let mut high_compressor = new_compressor();
    for col in cols {
        let mut high = vec![0u8; n * 4];
        high[..n].copy_from_slice(&col[4 * n..5 * n]);
        high[n..2 * n].copy_from_slice(&col[5 * n..6 * n]);
        high[2 * n..3 * n].copy_from_slice(&col[6 * n..7 * n]);
        high[3 * n..].copy_from_slice(&col[7 * n..]);
        let part = high_compressor.compress(&high).unwrap();
        high_bytes += part.len();
        high_parts.push(part);
    }
    let raw_low = cols.len() * n * 4;
    let mut dec = zstd::bulk::Decompressor::new().unwrap();
    let mut hbuf = vec![0u8; n * 4];
    let low = vec![0u8; n * 4];
    let mut sink = vec![0u8; n * 8];
    let high = sample(&mut || {
        for part in &high_parts {
            dec.decompress_to_buffer(part, &mut hbuf).unwrap();
            sink[..n * 4].copy_from_slice(&low);
            sink[n * 4..].copy_from_slice(&hbuf);
        }
        std::hint::black_box(sink[0]);
    });
    format!(
        "{{\"columns\":\"{name}\",\"n_atoms\":{n},\"n_columns\":{},\"uncompressed_bytes\":{uncompressed},\"full_shuffle_zstd_bytes\":{full_bytes},\"full_shuffle_decode\":{{{}}},\"high_four_zstd_bytes\":{high_bytes},\"low_four_raw_bytes\":{raw_low},\"high_four_stored_bytes\":{},\"high_four_decode_copy\":{{{}}}}}",
        cols.len(),
        summarize(&full),
        high_bytes + raw_low,
        summarize(&high)
    )
}

fn main() {
    require_pushed_commit();
    let n = 100_000usize;
    let mut pos = [Vec::new(), Vec::new(), Vec::new()];
    let mut frc = [Vec::new(), Vec::new(), Vec::new()];
    for i in 0..n {
        let x = i as f64 * 0.125;
        pos[0].push(x);
        pos[1].push(0.5);
        pos[2].push(1.0);
        frc[0].push(x * 0.5);
        frc[1].push(0.25);
        frc[2].push(0.5);
    }
    let pos: Vec<_> = pos.iter().map(|c| shuffle_column(c)).collect();
    let frc: Vec<_> = frc.iter().map(|c| shuffle_column(c)).collect();
    let mut both = pos;
    both.extend(frc.iter().cloned());
    let sets = [
        measure("positions_and_forces", &both),
        measure("forces", &frc),
    ];
    let commit = std::env::var("DB26_COMMIT").unwrap_or_else(|_| {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo)
            .output()
            .expect("git rev-parse");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    });
    let line = format!(
        "{{\"rounds\":20,\"untimed\":3,\"level\":3,\"window_log\":12,\"ldm\":false,\"commit\":\"{commit}\",\"sets\":[{}]}}",
        sets.join(",")
    );
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = repo.join("bench/db26/high_byte.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, line.as_bytes()).unwrap();
    println!("{line}");
}
