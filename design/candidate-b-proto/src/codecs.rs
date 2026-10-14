//! Lossless transforms on f64 coordinate and force blocks.
//!
//! Floating subtraction is not a reversible transform: `(a - b) + b` is not
//! always `a`. The predictors below operate on the IEEE bit pattern (XOR, or
//! wrapping subtraction of the u64 bits) so the frame can be restored exactly.

use crate::native::{self, BloscMode};
use pco::ChunkConfig;
use pco::standalone::{simple_compress, simple_decompress};

pub fn pack(pos: &[f64], frc: &[f64]) -> Vec<f64> {
    let mut v = Vec::with_capacity(pos.len() + frc.len());
    v.extend_from_slice(pos);
    v.extend_from_slice(frc);
    v
}

pub fn f64_le(xs: &[f64]) -> Vec<u8> {
    let mut o = Vec::with_capacity(xs.len() * 8);
    for x in xs {
        o.extend_from_slice(&x.to_le_bytes());
    }
    o
}

pub fn f64_from_le(b: &[u8]) -> Vec<f64> {
    assert_eq!(b.len() % 8, 0);
    b.chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

pub fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Byte shuffle: byte 0 of every f64, then byte 1, ... (HDF5 / Blosc shuffle).
pub fn shuffle8(src: &[u8]) -> Vec<u8> {
    assert_eq!(src.len() % 8, 0);
    let n = src.len() / 8;
    let mut out = vec![0u8; src.len()];
    for i in 0..n {
        for b in 0..8 {
            out[b * n + i] = src[i * 8 + b];
        }
    }
    out
}

pub fn unshuffle8(src: &[u8]) -> Vec<u8> {
    assert_eq!(src.len() % 8, 0);
    let n = src.len() / 8;
    let mut out = vec![0u8; src.len()];
    for i in 0..n {
        for b in 0..8 {
            out[i * 8 + b] = src[b * n + i];
        }
    }
    out
}

pub fn xor_bits(cur: &[f64], prev: &[f64]) -> Vec<f64> {
    cur.iter()
        .zip(prev)
        .map(|(a, b)| f64::from_bits(a.to_bits() ^ b.to_bits()))
        .collect()
}

pub fn wrapping_sub_bits(cur: &[f64], prev: &[f64]) -> Vec<f64> {
    cur.iter()
        .zip(prev)
        .map(|(a, b)| f64::from_bits(a.to_bits().wrapping_sub(b.to_bits())))
        .collect()
}

pub fn wrapping_add_bits(residual: &[f64], prev: &[f64]) -> Vec<f64> {
    residual
        .iter()
        .zip(prev)
        .map(|(d, b)| f64::from_bits(d.to_bits().wrapping_add(b.to_bits())))
        .collect()
}

/// How often a true f64 delta fails to invert. This is why the stored residual
/// is the bit pattern, not `a - b`.
pub fn f64_sub_irreversible(frames: &[Vec<f64>]) -> (u64, u64) {
    let mut bad = 0u64;
    let mut tot = 0u64;
    for w in frames.windows(2) {
        for (a, b) in w[1].iter().zip(w[0].iter()) {
            tot += 1;
            let d = a - b;
            if (b + d).to_bits() != a.to_bits() {
                bad += 1;
            }
        }
    }
    (bad, tot)
}

pub fn zstd_compress(data: &[u8], level: i32) -> Vec<u8> {
    zstd::bulk::compress(data, level).expect("zstd compress")
}

pub fn zstd_decompress(data: &[u8], cap: usize) -> Vec<u8> {
    zstd::bulk::decompress(data, cap).expect("zstd decompress")
}

pub fn shuffle_zstd(xs: &[f64], level: i32) -> Vec<u8> {
    zstd_compress(&shuffle8(&f64_le(xs)), level)
}

pub fn unshuffle_zstd(blob: &[u8], n_f64: usize) -> Vec<f64> {
    let raw = zstd_decompress(blob, n_f64 * 8);
    f64_from_le(&unshuffle8(&raw))
}

pub fn pco_f64(xs: &[f64]) -> Vec<u8> {
    simple_compress(xs, &ChunkConfig::default()).expect("pcodec")
}

pub fn unpco_f64(blob: &[u8]) -> Vec<f64> {
    simple_decompress(blob).expect("pcodec decompress")
}

/// Pcodec integer mode on the XOR residual, viewed as i64 (sign bit is clear
/// when the two floats share a sign, which they do along a continuous path).
pub fn pco_xor(cur: &[f64], prev: &[f64]) -> Vec<u8> {
    let ints: Vec<i64> = cur
        .iter()
        .zip(prev)
        .map(|(a, b)| (a.to_bits() ^ b.to_bits()) as i64)
        .collect();
    simple_compress(&ints, &ChunkConfig::default()).expect("pcodec xor")
}

pub fn unpco_xor(blob: &[u8], prev: &[f64]) -> Vec<f64> {
    let ints: Vec<i64> = simple_decompress(blob).expect("pcodec xor decompress");
    ints.into_iter()
        .zip(prev)
        .map(|(d, b)| f64::from_bits((d as u64) ^ b.to_bits()))
        .collect()
}

fn glue(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(4 + a.len() + b.len());
    o.extend_from_slice(&(a.len() as u32).to_le_bytes());
    o.extend_from_slice(a);
    o.extend_from_slice(b);
    o
}

fn unglue(blob: &[u8]) -> (&[u8], &[u8]) {
    let n = u32::from_le_bytes(blob[..4].try_into().unwrap()) as usize;
    let a = &blob[4..4 + n];
    let b = &blob[4 + n..];
    (a, b)
}

/// fpzip lossless on each block as ny×3 (xyz contiguous). One stored value.
pub fn fpzip_frame(pos: &[f64], frc: &[f64]) -> Vec<u8> {
    let n = (pos.len() / 3) as u32;
    let a = fpzip_rs::compress_f64(pos, 3, n, 1, 1).expect("fpzip pos");
    let b = fpzip_rs::compress_f64(frc, 3, n, 1, 1).expect("fpzip frc");
    glue(&a, &b)
}

pub fn unfpzip_frame(blob: &[u8]) -> (Vec<f64>, Vec<f64>) {
    let (a, b) = unglue(blob);
    (
        fpzip_rs::decompress_f64(a).expect("fpzip pos"),
        fpzip_rs::decompress_f64(b).expect("fpzip frc"),
    )
}

pub fn zfp_frame(pos: &[f64], frc: &[f64]) -> Vec<u8> {
    let n = pos.len() / 3;
    let a = native::zfp_compress_2d(pos, 3, n);
    let b = native::zfp_compress_2d(frc, 3, n);
    glue(&a, &b)
}

pub fn unzfp_frame(blob: &[u8], natoms: usize) -> (Vec<f64>, Vec<f64>) {
    let (a, b) = unglue(blob);
    (
        native::zfp_decompress_2d(a, 3, natoms),
        native::zfp_decompress_2d(b, 3, natoms),
    )
}

pub fn blosc_frame(mode: &BloscMode, xs: &[f64]) -> Vec<u8> {
    mode.compress(&f64_le(xs))
}

pub fn unblosc_frame(mode: &BloscMode, blob: &[u8], n_f64: usize) -> Vec<f64> {
    let mut raw = vec![0u8; n_f64 * 8];
    mode.decompress(blob, &mut raw);
    f64_from_le(&raw)
}

/// Time-major layout: for each scalar channel, the full frame series.
/// Pcodec's predictor then sees a trajectory instead of x,y,z of one atom.
pub fn time_major(frames: &[Vec<f64>]) -> Vec<f64> {
    let t = frames.len();
    let n = frames[0].len();
    let mut o = Vec::with_capacity(t * n);
    for c in 0..n {
        for frame in frames {
            o.push(frame[c]);
        }
    }
    o
}

pub fn untime_major(flat: &[f64], t: usize, n: usize) -> Vec<Vec<f64>> {
    assert_eq!(flat.len(), t * n);
    let mut frames = vec![vec![0f64; n]; t];
    for c in 0..n {
        for (ti, frame) in frames.iter_mut().enumerate() {
            frame[c] = flat[c * t + ti];
        }
    }
    frames
}

pub fn sum_len(blobs: &[Vec<u8>]) -> u64 {
    blobs.iter().map(|b| b.len() as u64).sum()
}

/// Absolute keyframe every `k` frames, XOR residual otherwise, then `codec`.
pub fn keyframe_xor<F>(frames: &[Vec<f64>], k: usize, mut codec: F) -> Vec<Vec<u8>>
where
    F: FnMut(&[f64]) -> Vec<u8>,
{
    let mut out = Vec::with_capacity(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        if i % k == 0 {
            out.push(codec(frame));
        } else {
            let residual = xor_bits(frame, &frames[i - 1]);
            out.push(codec(&residual));
        }
    }
    out
}

pub fn decode_keyframe_xor<D>(blobs: &[Vec<u8>], k: usize, index: usize, mut dec: D) -> Vec<f64>
where
    D: FnMut(&[u8]) -> Vec<f64>,
{
    let start = index - (index % k);
    let mut cur = dec(&blobs[start]);
    for j in (start + 1)..=index {
        let residual = dec(&blobs[j]);
        cur = xor_bits(&residual, &cur);
    }
    cur
}
