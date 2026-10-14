//! Candidate B measurement driver. Does not modify readcon-db.
//!
//! Trajectory: Zenodo 13741551 (configurations.xyz, forces_revPBE0-D3.xyz,
//! energies_revPBE0-D3.txt). NEB band is generated in `neb`.

mod codecs;
mod mdload;
mod native;
mod neb;

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use heed::types::Bytes;
use heed::{Database, EnvFlags, EnvOpenOptions};
use readcon_core::iterators::ConFrameIterator;
use readcon_core::types::{ConFrame, ConFrameBuilder};
use readcon_core::writer::ConFrameWriter;
use readcon_db::{ConCorpus, FrameKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use codecs::{
    bits_equal, blosc_frame, decode_keyframe_xor, f64_sub_irreversible, fpzip_frame, keyframe_xor,
    pack, pco_f64, pco_xor, shuffle_zstd, sum_len, time_major, unblosc_frame, unfpzip_frame,
    unpco_f64, unpco_xor, unshuffle_zstd, untime_major, unzfp_frame, wrapping_add_bits,
    wrapping_sub_bits, zfp_frame, zstd_compress, zstd_decompress,
};

fn arg(flag: &str, default: &str) -> String {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == flag {
            return args.next().unwrap_or_else(|| default.to_string());
        }
    }
    default.to_string()
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let d = Sha256::digest(&bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn write_text(fr: &ConFrame) -> String {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = ConFrameWriter::new(&mut buf);
        w.write_frame(fr).expect("write frame");
    }
    String::from_utf8(buf.into_inner()).expect("utf8")
}

fn parse_one(text: &str) -> ConFrame {
    let mut it = ConFrameIterator::new(text);
    it.next().expect("frame").expect("parse")
}

fn arrays_of(fr: &ConFrame) -> (Vec<f64>, Vec<f64>) {
    let mut p = Vec::with_capacity(fr.atom_data.len() * 3);
    let mut f = Vec::with_capacity(fr.atom_data.len() * 3);
    for a in &fr.atom_data {
        p.extend_from_slice(&[a.x, a.y, a.z]);
        let force = a.force.expect("force");
        f.extend_from_slice(&force);
    }
    (p, f)
}

fn mass_of(symbol: &str) -> f64 {
    match symbol {
        "O" => 15.999,
        "H" => 1.00784,
        "Ar" => 39.948,
        _ => panic!("mass {symbol}"),
    }
}

fn rebuild(fr: &ConFrame) -> ConFrame {
    let mut b = ConFrameBuilder::new(fr.header.boxl, fr.header.angles);
    b.prebox_header(fr.header.prebox_header.user.clone());
    b.postbox_header(fr.header.postbox_header.clone());
    let mut meta = serde_json::Map::new();
    for (k, v) in &fr.header.metadata {
        if matches!(
            k.as_str(),
            "energy" | "frame_index" | "neb_bead" | "neb_band" | "con_spec_version" | "con_spec_minor" | "sections"
        ) {
            continue;
        }
        meta.insert(k.clone(), v.clone());
    }
    b.set_metadata_json(&serde_json::Value::Object(meta).to_string())
        .expect("metadata");
    if let Some(e) = fr.header.energy() {
        b.set_energy(e);
    }
    if let Some(i) = fr.header.frame_index() {
        b.set_frame_index(i);
    }
    if let Some(bead) = fr.header.neb_bead() {
        b.set_neb_bead(bead);
    }
    if let Some(band) = fr.header.neb_band() {
        b.set_neb_band(band);
    }
    let mut type_i = 0usize;
    let mut prev = String::new();
    for a in &fr.atom_data {
        if a.symbol.as_ref() != prev.as_str() {
            prev = a.symbol.to_string();
            type_i += 1;
        }
        let mass = fr.header.masses_per_type[type_i - 1];
        b.add_atom(
            a.symbol.as_ref(),
            a.x,
            a.y,
            a.z,
            a.fixed,
            a.atom_id,
            mass,
        );
        if let Some(force) = a.force {
            b.with_force(force);
        }
    }
    b.build().expect("rebuild")
}

struct Set {
    pos: Vec<Vec<f64>>,
    frc: Vec<Vec<f64>>,
    texts: Vec<String>,
    /// Energies passed to the writer, before any text round trip.
    energy: Vec<f64>,
}

fn build_md(traj: &mdload::XyzTraj) -> Set {
    let cell = [12.42, 12.42, 12.42];
    let mut pos = Vec::with_capacity(traj.pos.len());
    let mut frc = Vec::with_capacity(traj.pos.len());
    let mut texts = Vec::with_capacity(traj.pos.len());
    let mut energy = Vec::with_capacity(traj.pos.len());
    for (i, (p, f)) in traj.pos.iter().zip(&traj.frc).enumerate() {
        let mut b = ConFrameBuilder::new(cell, [90.0; 3]);
        b.prebox_header("proton-water revPBE0-D3");
        b.postbox_header([String::new(), String::new()]);
        b.set_metadata_json(
            r#"{"units":{"energy":"eV","length":"angstrom"},"pbc":[true,true,true],"potential":{"type":"revPBE0-D3","params":{}}}"#,
        )
        .unwrap();
        b.set_energy(traj.energy[i]);
        b.set_frame_index(i as u64);
        for a in 0..traj.natoms {
            let sym = &traj.symbols[a];
            b.add_atom(
                sym,
                p[a * 3],
                p[a * 3 + 1],
                p[a * 3 + 2],
                [false; 3],
                a as u64,
                mass_of(sym),
            );
            b.with_force([f[a * 3], f[a * 3 + 1], f[a * 3 + 2]]);
        }
        let fr = b.build().unwrap();
        let (gp, gf) = arrays_of(&fr);
        texts.push(write_text(&fr));
        pos.push(gp);
        frc.push(gf);
        energy.push(traj.energy[i]);
    }
    Set {
        pos,
        frc,
        texts,
        energy,
    }
}

fn build_neb(band: &neb::Band) -> Set {
    let mut pos = Vec::new();
    let mut frc = Vec::new();
    let mut texts = Vec::new();
    let mut energy = Vec::new();
    for i in 0..band.pos.len() {
        let mut b = ConFrameBuilder::new(band.cell, [90.0; 3]);
        b.prebox_header("LJ fcc111 Ar adatom CI-NEB");
        b.postbox_header([String::new(), String::new()]);
        b.set_metadata_json(
            r#"{"units":{"energy":"eV","length":"angstrom"},"pbc":[true,true,false],"potential":{"type":"LJ","params":{}}}"#,
        )
        .unwrap();
        b.set_energy(band.energy[i]);
        b.set_frame_index(i as u64);
        b.set_neb_band(0);
        b.set_neb_bead(i as u64);
        for a in 0..band.symbols.len() {
            b.add_atom(
                &band.symbols[a],
                band.pos[i][a * 3],
                band.pos[i][a * 3 + 1],
                band.pos[i][a * 3 + 2],
                band.fixed[a],
                a as u64,
                band.masses[a],
            );
            b.with_force([
                band.frc[i][a * 3],
                band.frc[i][a * 3 + 1],
                band.frc[i][a * 3 + 2],
            ]);
        }
        let fr = b.build().unwrap();
        let (gp, gf) = arrays_of(&fr);
        texts.push(write_text(&fr));
        pos.push(gp);
        frc.push(gf);
        energy.push(band.energy[i]);
    }
    Set {
        pos,
        frc,
        texts,
        energy,
    }
}

fn text_bytes(texts: &[String]) -> u64 {
    texts.iter().map(|s| s.len() as u64).sum()
}

fn bit_exact(set: &Set) -> Value {
    let n = set.texts.len();
    let mut idem_ok = 0u64;
    let mut rebuild_src_ok = 0u64;
    let mut rebuild_parsed_ok = 0u64;
    let mut pos_bits = 0u64;
    let mut pos_bad = 0u64;
    let mut frc_bits = 0u64;
    let mut frc_bad = 0u64;
    let mut energy_bad = 0u64;
    let mut first_idem = None;
    let mut first_rebuild = None;
    for i in 0..n {
        let parsed = parse_one(&set.texts[i]);
        let again = write_text(&parsed);
        if again == set.texts[i] {
            idem_ok += 1;
        } else if first_idem.is_none() {
            first_idem = Some(diff_at(&set.texts[i], &again));
        }
        let (pp, pf) = arrays_of(&parsed);
        for (a, b) in set.pos[i].iter().zip(&pp) {
            pos_bits += 1;
            if a.to_bits() != b.to_bits() {
                pos_bad += 1;
            }
        }
        for (a, b) in set.frc[i].iter().zip(&pf) {
            frc_bits += 1;
            if a.to_bits() != b.to_bits() {
                frc_bad += 1;
            }
        }
        // Compare the energy the parser recovered with the f64 the writer was given.
        if parsed.header.energy().map(f64::to_bits) != Some(set.energy[i].to_bits()) {
            energy_bad += 1;
        }
        // Rebuild from the stored (pre-parse) f64s by grafting them onto a
        // parsed frame's non-numeric fields, then also from the parsed f64s.
        let mut owned = parsed.clone();
        for (a, slot) in owned.atom_data.iter_mut().zip(set.pos[i].chunks_exact(3)) {
            a.x = slot[0];
            a.y = slot[1];
            a.z = slot[2];
        }
        for (a, slot) in owned.atom_data.iter_mut().zip(set.frc[i].chunks_exact(3)) {
            a.force = Some([slot[0], slot[1], slot[2]]);
        }
        let regen_src = write_text(&rebuild(&owned));
        if regen_src == set.texts[i] {
            rebuild_src_ok += 1;
        } else if first_rebuild.is_none() {
            first_rebuild = Some(diff_at(&set.texts[i], &regen_src));
        }
        let regen_parsed = write_text(&rebuild(&parsed));
        if regen_parsed == set.texts[i] {
            rebuild_parsed_ok += 1;
        }
    }
    json!({
        "frames": n,
        "writer_idempotent_frames": idem_ok,
        "rebuild_from_stored_f64_frames": rebuild_src_ok,
        "rebuild_from_parsed_f64_frames": rebuild_parsed_ok,
        "position_values": pos_bits,
        "position_bits_lost": pos_bad,
        "force_values": frc_bits,
        "force_bits_lost": frc_bad,
        "energy_bits_lost": energy_bad,
        "first_idempotence_mismatch": first_idem,
        "first_rebuild_mismatch": first_rebuild,
        "sample_coordinate_line": set.texts[0].lines().nth(11).unwrap_or("").to_string(),
    })
}

fn diff_at(a: &str, b: &str) -> String {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let n = ab.len().min(bb.len());
    let mut at = n;
    for i in 0..n {
        if ab[i] != bb[i] {
            at = i;
            break;
        }
    }
    let lo = at.saturating_sub(20);
    let hi = (at + 40).min(ab.len());
    let bh = (at + 40).min(bb.len());
    format!(
        "len {} vs {} first delta at {at} left={:?} right={:?}",
        ab.len(),
        bb.len(),
        String::from_utf8_lossy(&ab[lo..hi]),
        String::from_utf8_lossy(&bb[lo..bh])
    )
}

fn legacy_roundtrip(dir: &Path) -> Value {
    let mut files = Vec::new();
    for ent in fs::read_dir(dir).unwrap() {
        let ent = ent.unwrap();
        let path = ent.path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !(name.ends_with(".con") || name.ends_with(".convel")) {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let mut it = ConFrameIterator::new(&text);
        let mut frames = 0u64;
        let mut exact = 0u64;
        let mut note = None;
        loop {
            let Some(item) = it.next_with_raw_span(&text) else { break };
            let (frame, span) = item.expect("span");
            frames += 1;
            let again = write_text(&frame);
            if again == span {
                exact += 1;
            } else if note.is_none() {
                note = Some(diff_at(span, &again));
            }
        }
        files.push(json!({
            "file": name,
            "frames": frames,
            "bit_exact": exact,
            "first_mismatch": note,
        }));
    }
    files.sort_by(|a, b| a["file"].as_str().unwrap().cmp(b["file"].as_str().unwrap()));
    Value::Array(files)
}

fn pct(mut xs: Vec<f64>) -> (f64, f64, f64) {
    xs.sort_by(|a, b| a.total_cmp(b));
    let n = xs.len().max(1);
    let mean = xs.iter().sum::<f64>() / n as f64;
    let med = xs[n / 2];
    let p99 = xs[((n - 1) as f64 * 0.99).round() as usize];
    (mean, med, p99)
}

fn sample_idxs(n: usize, k: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    let mut s = 0x1234_5678_u64;
    for i in (1..n).rev() {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (s as usize) % (i + 1);
        idx.swap(i, j);
    }
    idx.truncate(k.min(n));
    idx
}

fn time_us(idxs: &[usize], mut body: impl FnMut(usize)) -> (f64, f64, f64) {
    for &i in idxs.iter().take(8) {
        body(i);
    }
    let mut samples = Vec::with_capacity(idxs.len());
    for &i in idxs {
        let t = Instant::now();
        body(i);
        let us = t.elapsed().as_secs_f64() * 1e6;
        samples.push(us);
    }
    pct(samples)
}

fn row(name: &str, bytes: u64, text: u64, raw: u64, access: &str, mean: f64, med: f64, p99: f64, lossless: bool) -> Value {
    json!({
        "name": name,
        "bytes": bytes,
        "vs_text": bytes as f64 / text as f64,
        "vs_f64": bytes as f64 / raw as f64,
        "access": access,
        "decode_mean_us": mean,
        "decode_median_us": med,
        "decode_p99_us": p99,
        "lossless": lossless,
    })
}

fn packed(set: &Set) -> Vec<Vec<f64>> {
    set.pos
        .iter()
        .zip(&set.frc)
        .map(|(p, f)| pack(p, f))
        .collect()
}

fn measure_numeric(set: &Set, blosc: &[native::BloscMode]) -> Vec<Value> {
    let frames = packed(set);
    let n = frames.len();
    let natoms = set.pos[0].len() / 3;
    let n_f64 = frames[0].len();
    let text = text_bytes(&set.texts);
    let raw = (n_f64 * 8 * n) as u64;
    let idxs = sample_idxs(n, 250.min(n));
    let mut rows = Vec::new();

    let raw_bytes = raw;
    let (mean, med, p99) = time_us(&idxs, |i| {
        std::hint::black_box(frames[i][0].to_bits());
    });
    rows.push(row(
        "f64-le-uncompressed",
        raw_bytes,
        text,
        raw,
        "one value, no decode",
        mean,
        med,
        p99,
        true,
    ));

    let z3: Vec<Vec<u8>> = frames.iter().map(|f| zstd_compress(&codecs::f64_le(f), 3)).collect();
    let lossless = z3.iter().zip(&frames).all(|(b, f)| {
        bits_equal(&codecs::f64_from_le(&zstd_decompress(b, n_f64 * 8)), f)
    });
    let mut dec = zstd::bulk::Decompressor::new().unwrap();
    let (mean, med, p99) = time_us(&idxs, |i| {
        let out = dec.decompress(&z3[i], n_f64 * 8).unwrap();
        std::hint::black_box(out.len());
    });
    rows.push(row("zstd3-raw-f64", sum_len(&z3), text, raw, "one value", mean, med, p99, lossless));

    let sh: Vec<Vec<u8>> = frames.iter().map(|f| shuffle_zstd(f, 3)).collect();
    let lossless = sh.iter().zip(&frames).all(|(b, f)| bits_equal(&unshuffle_zstd(b, n_f64), f));
    let (mean, med, p99) = time_us(&idxs, |i| {
        let out = unshuffle_zstd(&sh[i], n_f64);
        std::hint::black_box(out[0].to_bits());
    });
    rows.push(row(
        "shuffle-zstd3",
        sum_len(&sh),
        text,
        raw,
        "one value",
        mean,
        med,
        p99,
        lossless,
    ));

    for mode in blosc {
        let blobs: Vec<Vec<u8>> = frames.iter().map(|f| blosc_frame(mode, f)).collect();
        let lossless = blobs.iter().zip(&frames).all(|(b, f)| {
            bits_equal(&unblosc_frame(mode, b, n_f64), f)
        });
        let (mean, med, p99) = time_us(&idxs, |i| {
            let out = unblosc_frame(mode, &blobs[i], n_f64);
            std::hint::black_box(out[0].to_bits());
        });
        rows.push(row(mode.name(), sum_len(&blobs), text, raw, "one value", mean, med, p99, lossless));
    }

    let pc: Vec<Vec<u8>> = frames.iter().map(|f| pco_f64(f)).collect();
    let lossless = pc.iter().zip(&frames).all(|(b, f)| bits_equal(&unpco_f64(b), f));
    let (mean, med, p99) = time_us(&idxs, |i| {
        let out = unpco_f64(&pc[i]);
        std::hint::black_box(out[0].to_bits());
    });
    rows.push(row("pcodec-f64", sum_len(&pc), text, raw, "one value", mean, med, p99, lossless));

    let fz: Vec<Vec<u8>> = set.pos.iter().zip(&set.frc).map(|(p, f)| fpzip_frame(p, f)).collect();
    let lossless = fz.iter().enumerate().all(|(i, b)| {
        let (p, f) = unfpzip_frame(b);
        bits_equal(&p, &set.pos[i]) && bits_equal(&f, &set.frc[i])
    });
    let (mean, med, p99) = time_us(&idxs, |i| {
        let (p, _) = unfpzip_frame(&fz[i]);
        std::hint::black_box(p[0].to_bits());
    });
    rows.push(row("fpzip-lossless-2d", sum_len(&fz), text, raw, "one value", mean, med, p99, lossless));

    let zp: Vec<Vec<u8>> = set.pos.iter().zip(&set.frc).map(|(p, f)| zfp_frame(p, f)).collect();
    let lossless = zp.iter().enumerate().all(|(i, b)| {
        let (p, f) = unzfp_frame(b, natoms);
        bits_equal(&p, &set.pos[i]) && bits_equal(&f, &set.frc[i])
    });
    let (mean, med, p99) = time_us(&idxs, |i| {
        let (p, _) = unzfp_frame(&zp[i], natoms);
        std::hint::black_box(p[0].to_bits());
    });
    rows.push(row(
        "zfp-reversible-2d",
        sum_len(&zp),
        text,
        raw,
        "one value",
        mean,
        med,
        p99,
        lossless,
    ));

    for k in [8usize, 32] {
        if n < k {
            continue;
        }
        let blobs = keyframe_xor(&frames, k, |f| shuffle_zstd(f, 3));
        let ok = (0..n).all(|i| {
            let got = decode_keyframe_xor(&blobs, k, i, |b| unshuffle_zstd(b, n_f64));
            bits_equal(&got, &frames[i])
        });
        let (mean, med, p99) = time_us(&idxs, |i| {
            let got = decode_keyframe_xor(&blobs, k, i, |b| unshuffle_zstd(b, n_f64));
            std::hint::black_box(got[0].to_bits());
        });
        rows.push(row(
            &format!("xor-prev-shuffle-zstd3-k{k}"),
            sum_len(&blobs),
            text,
            raw,
            "chain to previous keyframe",
            mean,
            med,
            p99,
            ok,
        ));
    }

    if n > 8 {
        let mut blobs = Vec::with_capacity(n);
        for (i, frame) in frames.iter().enumerate() {
            if i % 8 == 0 {
                blobs.push(shuffle_zstd(frame, 3));
            } else {
                blobs.push(shuffle_zstd(&wrapping_sub_bits(frame, &frames[i - 1]), 3));
            }
        }
        let decode_at = |index: usize| {
            let start = index - (index % 8);
            let mut cur = unshuffle_zstd(&blobs[start], n_f64);
            for j in (start + 1)..=index {
                let residual = unshuffle_zstd(&blobs[j], n_f64);
                cur = wrapping_add_bits(&residual, &cur);
            }
            cur
        };
        let ok = (0..n).all(|i| bits_equal(&decode_at(i), &frames[i]));
        let (mean, med, p99) = time_us(&idxs, |i| {
            std::hint::black_box(decode_at(i)[0].to_bits());
        });
        rows.push(row(
            "wrapsub-prev-shuffle-zstd3-k8",
            sum_len(&blobs),
            text,
            raw,
            "chain to previous keyframe",
            mean,
            med,
            p99,
            ok,
        ));
    }

    if n > 1 {
        let mut xor_pc = Vec::with_capacity(n);
        xor_pc.push(pco_f64(&frames[0]));
        for i in 1..n {
            xor_pc.push(pco_xor(&frames[i], &frames[i - 1]));
        }
        // Full-chain decode is the size experiment; latency uses k-style walk
        // only as a size row here (k = n, one keyframe).
        let bytes = sum_len(&xor_pc);
        let mut cur = unpco_f64(&xor_pc[0]);
        let mut ok = bits_equal(&cur, &frames[0]);
        for i in 1..n {
            cur = unpco_xor(&xor_pc[i], &cur);
            ok &= bits_equal(&cur, &frames[i]);
        }
        rows.push(row(
            "xor-prev-pcodec-full-chain",
            bytes,
            text,
            raw,
            "must replay from frame 0",
            0.0,
            0.0,
            0.0,
            ok,
        ));
    }

    // Whole-series ceilings. Decode time is the full blob, reported per frame.
    let pos_all: Vec<f64> = set.pos.iter().flatten().copied().collect();
    let frc_all: Vec<f64> = set.frc.iter().flatten().copied().collect();
    let fp = fpzip_rs::compress_f64(&pos_all, 3, natoms as u32, n as u32, 1).unwrap();
    let ff = fpzip_rs::compress_f64(&frc_all, 3, natoms as u32, n as u32, 1).unwrap();
    let back_p = fpzip_rs::decompress_f64(&fp).unwrap();
    let back_f = fpzip_rs::decompress_f64(&ff).unwrap();
    let ok = bits_equal(&back_p, &pos_all) && bits_equal(&back_f, &frc_all);
    let t = Instant::now();
    let _ = fpzip_rs::decompress_f64(&fp).unwrap();
    let _ = fpzip_rs::decompress_f64(&ff).unwrap();
    let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    rows.push(row(
        "fpzip-lossless-3d-whole",
        (fp.len() + ff.len()) as u64,
        text,
        raw,
        "whole trajectory, not one get",
        us,
        us,
        us,
        ok,
    ));

    let zp = native::zfp_compress_3d(&pos_all, 3, natoms, n);
    let zf = native::zfp_compress_3d(&frc_all, 3, natoms, n);
    let bp = native::zfp_decompress_3d(&zp, 3, natoms, n);
    let bf = native::zfp_decompress_3d(&zf, 3, natoms, n);
    let ok = bits_equal(&bp, &pos_all) && bits_equal(&bf, &frc_all);
    let t = Instant::now();
    std::hint::black_box(native::zfp_decompress_3d(&zp, 3, natoms, n).len());
    std::hint::black_box(native::zfp_decompress_3d(&zf, 3, natoms, n).len());
    let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    rows.push(row(
        "zfp-reversible-3d-whole",
        (zp.len() + zf.len()) as u64,
        text,
        raw,
        "whole trajectory, not one get",
        us,
        us,
        us,
        ok,
    ));

    let tm = time_major(&frames);
    let pc = pco_f64(&tm);
    let back = unpco_f64(&pc);
    let restored = untime_major(&back, n, n_f64);
    let ok = restored.iter().zip(&frames).all(|(a, b)| bits_equal(a, b));
    let t = Instant::now();
    std::hint::black_box(unpco_f64(&pc).len());
    let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    rows.push(row(
        "pcodec-time-major-whole",
        pc.len() as u64,
        text,
        raw,
        "whole trajectory, not one get",
        us,
        us,
        us,
        ok,
    ));

    rows
}

fn measure_text(texts: &[String]) -> Vec<Value> {
    let text = text_bytes(texts);
    let n = texts.len();
    let idxs = sample_idxs(n, 250.min(n));
    let mut rows = Vec::new();
    let (mean, med, p99) = time_us(&idxs, |i| {
        std::hint::black_box(texts[i].len());
    });
    rows.push(row("con-text", text, text, text, "one value, memcpy", mean, med, p99, true));

    let z3: Vec<Vec<u8>> = texts.iter().map(|s| zstd_compress(s.as_bytes(), 3)).collect();
    let cap = texts.iter().map(|s| s.len()).max().unwrap();
    let ok = z3.iter().zip(texts).all(|(b, s)| zstd_decompress(b, cap) == s.as_bytes());
    let mut dec = zstd::bulk::Decompressor::new().unwrap();
    let (mean, med, p99) = time_us(&idxs, |i| {
        let out = dec.decompress(&z3[i], cap).unwrap();
        std::hint::black_box(out.len());
    });
    rows.push(row("zstd3-text", sum_len(&z3), text, text, "one value", mean, med, p99, ok));

    let z19: Vec<Vec<u8>> = texts.iter().map(|s| zstd_compress(s.as_bytes(), 19)).collect();
    let ok = z19.iter().zip(texts).all(|(b, s)| zstd_decompress(b, cap) == s.as_bytes());
    rows.push(row("zstd19-text", sum_len(&z19), text, text, "one value, size only", 0.0, 0.0, 0.0, ok));

    // A long trajectory trains on every 20th frame. A short band uses every
    // frame. The dict cap stays under the training bytes; zstd rejects a
    // dictionary larger than its samples.
    let stride = if texts.len() >= 40 { 20 } else { 1 };
    let train: Vec<&[u8]> = texts.iter().step_by(stride).map(|s| s.as_bytes()).collect();
    let train_bytes: usize = train.iter().map(|s| s.len()).sum();
    let dict_cap = (32 * 1024).min(train_bytes / 8).max(256);
    let dict = zstd::dict::from_samples(&train, dict_cap).expect("dict");
    let mut comp = zstd::bulk::Compressor::with_dictionary(3, &dict).unwrap();
    let mut dict_blobs = Vec::with_capacity(n);
    for s in texts {
        dict_blobs.push(comp.compress(s.as_bytes()).unwrap());
    }
    let mut ddec = zstd::bulk::Decompressor::with_dictionary(&dict).unwrap();
    let ok = dict_blobs.iter().zip(texts).all(|(b, s)| ddec.decompress(b, cap).unwrap() == s.as_bytes());
    let (mean, med, p99) = time_us(&idxs, |i| {
        let out = ddec.decompress(&dict_blobs[i], cap).unwrap();
        std::hint::black_box(out.len());
    });
    rows.push(row(
        "zstd3-dict-text",
        sum_len(&dict_blobs) + dict.len() as u64,
            text,
        text,
        &format!("one value + shared dict of {} bytes, trained stride {stride}", dict.len()),
        mean,
        med,
        p99,
        ok,
    ));
    let _dict_len = dict.len();

    let all = texts.concat();
    let whole3 = zstd_compress(all.as_bytes(), 3);
    let whole19 = zstd_compress(all.as_bytes(), 19);
    let back = zstd_decompress(&whole3, all.len());
    rows.push(row(
        "zstd3-text-whole-file",
        whole3.len() as u64,
        text,
        text,
        "no single-frame get",
        0.0,
        0.0,
        0.0,
        back == all.as_bytes(),
    ));
    // vs_f64 for text rows uses the text size as the denominator.
    let back19 = zstd_decompress(&whole19, all.len());
    rows.push(row(
        "zstd19-text-whole-file",
        whole19.len() as u64,
        text,
        text,
        "no single-frame get",
        0.0,
        0.0,
        0.0,
        back19 == all.as_bytes(),
    ));
    let _ = _dict_len;
    rows
}

fn chord_predict(set: &Set) -> Value {
    let n = set.pos.len();
    let end = n - 1;
    let mut pos_bytes = 0u64;
    let mut frc_bytes = 0u64;
    let mut ok = true;
    // Endpoints stored with shuffle+zstd. Middle images are XOR against the
    // chord (positions) or the previous image (forces).
    let p0 = shuffle_zstd(&set.pos[0], 3);
    let pn = shuffle_zstd(&set.pos[end], 3);
    pos_bytes += (p0.len() + pn.len()) as u64;
    let f0 = shuffle_zstd(&set.frc[0], 3);
    let fn_ = shuffle_zstd(&set.frc[end], 3);
    frc_bytes += (f0.len() + fn_.len()) as u64;
    let npos = set.pos[0].len();
    let mut residuals_p = Vec::new();
    let mut residuals_f = Vec::new();
    for i in 1..end {
        let t = i as f64 / end as f64;
        let pred: Vec<f64> = set.pos[0]
            .iter()
            .zip(&set.pos[end])
            .map(|(a, b)| (1.0 - t) * a + t * b)
            .collect();
        let res = codecs::xor_bits(&set.pos[i], &pred);
        let blob = shuffle_zstd(&res, 3);
        pos_bytes += blob.len() as u64;
        residuals_p.push((pred, blob));
        let fres = codecs::xor_bits(&set.frc[i], &set.frc[i - 1]);
        let fblob = shuffle_zstd(&fres, 3);
        frc_bytes += fblob.len() as u64;
        residuals_f.push(fblob);
    }
    let got0 = unshuffle_zstd(&p0, npos);
    ok &= bits_equal(&got0, &set.pos[0]);
    for (i, (pred, blob)) in residuals_p.iter().enumerate() {
        let res = unshuffle_zstd(blob, npos);
        let got = codecs::xor_bits(&res, pred);
        ok &= bits_equal(&got, &set.pos[i + 1]);
    }
    // Cold read of a middle image: two endpoint decompresses + one residual.
    let mid = (n / 2).max(1).min(end - 1);
    let t = Instant::now();
    let a = unshuffle_zstd(&p0, npos);
    let b = unshuffle_zstd(&pn, npos);
    let tt = mid as f64 / end as f64;
    let pred: Vec<f64> = a.iter().zip(&b).map(|(x, y)| (1.0 - tt) * x + tt * y).collect();
    let res = unshuffle_zstd(&residuals_p[mid - 1].1, npos);
    let got = codecs::xor_bits(&res, &pred);
    std::hint::black_box(got[0].to_bits());
    let cold_us = t.elapsed().as_secs_f64() * 1e6;
    let text = text_bytes(&set.texts);
    let raw = (npos * 8 * n * 2) as u64;
    json!({
        "pos_chord_bytes": pos_bytes,
        "frc_prev_xor_bytes": frc_bytes,
        "total_bytes": pos_bytes + frc_bytes,
        "vs_text": (pos_bytes + frc_bytes) as f64 / text as f64,
        "vs_f64": (pos_bytes + frc_bytes) as f64 / raw as f64,
        "cold_middle_image_us": cold_us,
        "lossless": ok,
        "images": n,
    })
}

fn lmdb_study(texts: &[String], numeric_blobs: &[Vec<u8>], text_zstd: &[Vec<u8>]) -> Value {
    let root = std::env::temp_dir().join(format!("candb-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let con_path = root.join("traj.con");
    {
        let mut f = fs::File::create(&con_path).unwrap();
        for t in texts {
            f.write_all(t.as_bytes()).unwrap();
        }
    }
    let corpus = root.join("corpus");
    let db = ConCorpus::open(&corpus).unwrap();
    let n = db.append_trajectory_path(1, &con_path).unwrap();
    assert_eq!(n as usize, texts.len());
    let stored = db.get_frame_text(FrameKey { traj_id: 1, frame_idx: 0 }).unwrap();
    assert_eq!(stored, texts[0], "LMDB blob is the CON span");
    let idxs = sample_idxs(texts.len(), 200.min(texts.len()));
    let (mean, med, p99) = time_us(&idxs, |i| {
        let s = db
            .get_frame_text(FrameKey {
                traj_id: 1,
                frame_idx: i as u32,
            })
            .unwrap();
        std::hint::black_box(s.len());
    });
    let t = Instant::now();
    {
        let keys: Vec<FrameKey> = idxs
            .iter()
            .map(|&i| FrameKey {
                traj_id: 1,
                frame_idx: i as u32,
            })
            .collect();
        let got = db.get_frame_texts(&keys).unwrap();
        std::hint::black_box(got.len());
    }
    let batch_us = t.elapsed().as_secs_f64() * 1e6 / idxs.len() as f64;
    let text_only = fs::metadata(corpus.join("data.mdb")).unwrap().len();
    db.recook_all().unwrap();
    let with_soa = fs::metadata(corpus.join("data.mdb")).unwrap().len();
    let mut soa_bytes = 0u64;
    let (smean, smed, sp99) = time_us(&idxs, |i| {
        let b = db
            .get_cooked_soa_bytes(FrameKey {
                traj_id: 1,
                frame_idx: i as u32,
            })
            .unwrap()
            .unwrap();
        soa_bytes += b.len() as u64;
        std::hint::black_box(b.len());
    });
    // soa_bytes above only summed the timed sample. Sum all frames once.
    let mut soa_all = 0u64;
    for i in 0..texts.len() as u32 {
        soa_all += db
            .get_cooked_soa_bytes(FrameKey { traj_id: 1, frame_idx: i })
            .unwrap()
            .unwrap()
            .len() as u64;
    }
    db.close();

    let value_plain = lmdb_values(&root.join("values-plain"), texts.iter().map(|s| s.as_bytes().to_vec()).collect());
    let value_zstd = lmdb_values(&root.join("values-zstd-text"), text_zstd.to_vec());
    let value_num = lmdb_values(&root.join("values-numeric"), numeric_blobs.to_vec());

    let raw_text = text_bytes(texts);
    let _ = fs::remove_dir_all(&root);
    json!({
        "frames": texts.len(),
        "con_text_bytes": raw_text,
        "data_mdb_text_and_indexes": text_only,
        "data_mdb_over_text": text_only as f64 / raw_text as f64,
        "data_mdb_after_recook": with_soa,
        "frames_soa_bytes": soa_all,
        "get_frame_text_median_us": med,
        "get_frame_text_mean_us": mean,
        "get_frame_text_p99_us": p99,
        "get_frame_texts_per_frame_us": batch_us,
        "get_cooked_soa_median_us": smed,
        "get_cooked_soa_mean_us": smean,
        "get_cooked_soa_p99_us": sp99,
        "value_only_plain_mdb": value_plain,
        "value_only_zstd_text_mdb": value_zstd,
        "value_only_numeric_mdb": value_num,
    })
}

fn lmdb_values(dir: &Path, blobs: Vec<Vec<u8>>) -> u64 {
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(dir).unwrap();
    let mut opts = EnvOpenOptions::new();
    opts.map_size(512 * 1024 * 1024);
    opts.max_dbs(4);
    let env = unsafe { opts.open(dir).unwrap() };
    {
        let mut wtxn = env.write_txn().unwrap();
        let db: Database<Bytes, Bytes> = env.create_database(&mut wtxn, Some("v")).unwrap();
        for (i, blob) in blobs.iter().enumerate() {
            let key = FrameKey {
                traj_id: 1,
                frame_idx: i as u32,
            }
            .to_bytes();
            db.put(&mut wtxn, &key[..], blob).unwrap();
        }
        wtxn.commit().unwrap();
    }
    env.force_sync().ok();
    drop(env);
    fs::metadata(dir.join("data.mdb")).unwrap().len()
}

fn neb_neighbor(set: &Set) -> Value {
    let frames = packed(set);
    let n = frames.len();
    let n_f64 = frames[0].len();
    let blobs = keyframe_xor(&frames, n.max(1), |f| shuffle_zstd(f, 3));
    // k = n means only frame 0 is a keyframe: pure neighbour chain.
    // Also a chain with every image independent is already in measure_numeric.
    let ok = (0..n).all(|i| {
        bits_equal(
            &decode_keyframe_xor(&blobs, n, i, |b| unshuffle_zstd(b, n_f64)),
            &frames[i],
        )
    });
    let idxs = sample_idxs(n, n);
    let (mean, med, p99) = time_us(&idxs, |i| {
        let got = decode_keyframe_xor(&blobs, n, i, |b| unshuffle_zstd(b, n_f64));
        std::hint::black_box(got[0].to_bits());
    });
    let text = text_bytes(&set.texts);
    let raw = (n_f64 * 8 * n) as u64;
    row(
        "neb-xor-previous-image-shuffle-zstd3",
        sum_len(&blobs),
        text,
        raw,
        "chain from image 0",
        mean,
        med,
        p99,
        ok,
    )
}

fn main() {
    native::self_test();
    eprintln!("native codecs ok");
    let conf = PathBuf::from(arg(
        "--conf",
        "/tmp/traj/configurations.xyz",
    ));
    let forces = PathBuf::from(arg("--forces", "/tmp/traj/forces_revPBE0.xyz"));
    let energies = PathBuf::from(arg("--energies", "/tmp/traj/energies.txt"));
    let legacy = PathBuf::from(arg("--legacy", "/workspace/resources/test"));
    let out = PathBuf::from(arg(
        "--out",
        "/workspace/design/candidate-b-proto/results.json",
    ));

    let t0 = Instant::now();
    let traj = mdload::load(&conf, &forces, &energies);
    eprintln!("loaded {} frames in {:.1}s", traj.pos.len(), t0.elapsed().as_secs_f64());
    let (rmsd_mean, rmsd_med, rmsd_max) = mdload::rmsd_report(&traj.pos, 12.42);
    let mut jumps = 0u64;
    // reuse report's definition inline for the jump count
    {
        let n = traj.natoms;
        for w in traj.pos.windows(2) {
            let mut s = 0.0;
            for i in 0..n {
                for k in 0..3 {
                    let mut d = w[1][i * 3 + k] - w[0][i * 3 + k];
                    d -= 12.42 * (d / 12.42).round();
                    s += d * d;
                }
            }
            if (s / n as f64).sqrt() > 1.0 {
                jumps += 1;
            }
        }
    }
    eprintln!("writing CON");
    let md = build_md(&traj);
    eprintln!("CON {} bytes, bit-exact pass", text_bytes(&md.texts));
    let exact = bit_exact(&md);
    eprintln!("legacy files");
    let legacy_rep = legacy_roundtrip(&legacy);
    let (bad_sub, tot_sub) = f64_sub_irreversible(&packed(&md));

    let blosc = native::blosc_modes();
    eprintln!("md codecs");
    let numeric = measure_numeric(&md, &blosc);
    eprintln!("md text codecs");
    let text_rows = measure_text(&md.texts);

    eprintln!("neb");
    let band = neb::generate();
    let neb_set = build_neb(&band);
    let neb_exact = bit_exact(&neb_set);
    let neb_numeric = measure_numeric(&neb_set, &blosc);
    let neb_text = measure_text(&neb_set.texts);
    let neb_chain = neb_neighbor(&neb_set);
    let neb_chord = chord_predict(&neb_set);

    eprintln!("lmdb");
    let zstd_text: Vec<Vec<u8>> = md.texts.iter().map(|s| zstd_compress(s.as_bytes(), 3)).collect();
    let numeric_blobs: Vec<Vec<u8>> = packed(&md).iter().map(|f| shuffle_zstd(f, 3)).collect();
    let lmdb = lmdb_study(&md.texts, &numeric_blobs, &zstd_text);

    let n_o = traj.symbols.iter().filter(|s| s.as_str() == "O").count();
    let n_h = traj.symbols.iter().filter(|s| s.as_str() == "H").count();
    let report = json!({
        "md": {
            "source": "https://zenodo.org/records/13741551",
            "description": "time-ordered revPBE0-D3 configurations of an excess proton in bulk water",
            "sha256_configurations": sha256_file(&conf),
            "sha256_forces": sha256_file(&forces),
            "sha256_energies": sha256_file(&energies),
            "n_frames": traj.pos.len(),
            "natoms": traj.natoms,
            "n_O": n_o,
            "n_H": n_h,
            "cell_A": 12.42,
            "consecutive_rmsd_A": {"mean": rmsd_mean, "median": rmsd_med, "max": rmsd_max},
            "jumps_rmsd_gt_1A": jumps,
            "con_text_bytes": text_bytes(&md.texts),
            "f64_pos_force_bytes": (packed(&md)[0].len() * 8 * md.texts.len()) as u64,
            "f64_sub_irreversible": bad_sub,
            "f64_sub_pairs": tot_sub,
            "bit_exact": exact,
            "text_codecs": text_rows,
            "numeric_codecs": numeric,
        },
        "legacy_con_rewrite": legacy_rep,
        "neb": {
            "model": "LJ fcc(111) Ar adatom, fcc hollow to hcp hollow, CI-NEB",
            "sigma_A": 3.405,
            "epsilon_eV": 0.1,
            "cutoff_A": 2.5 * 3.405,
            "k_spring_eV_per_A2": band.k_spring,
            "n_images": band.pos.len(),
            "natoms": band.symbols.len(),
            "cell_A": band.cell,
            "relax_steps": band.relax_steps,
            "neb_steps": band.neb_steps,
            "endpoint_fmax_eV_A": band.endpoint_fmax,
            "neb_fmax_eV_A": band.neb_fmax,
            "energies_eV": band.energy,
            "con_text_bytes": text_bytes(&neb_set.texts),
            "f64_pos_force_bytes": (packed(&neb_set)[0].len() * 8 * neb_set.texts.len()) as u64,
            "bit_exact": neb_exact,
            "text_codecs": neb_text,
            "numeric_codecs": neb_numeric,
            "neighbor_chain": neb_chain,
            "chord": neb_chord,
        },
        "lmdb": lmdb,
        "codecs": {
            "zfp": "1.0.1 reversible",
            "blosc2": "2.15.2 zstd level 3, 1 thread",
            "pcodec": "pco 1.x ChunkConfig::default",
            "fpzip": "fpzip-rs 0.1 lossless precision 64",
            "zstd": "0.13",
        }
    });
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    eprintln!("wrote {}", out.display());
}

// silence unused import if force_sync path changes
#[allow(dead_code)]
fn _flags(opts: &mut EnvOpenOptions) {
    unsafe {
        opts.flags(EnvFlags::empty());
    }
}
