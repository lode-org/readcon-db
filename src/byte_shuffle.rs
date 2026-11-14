//! Byte shuffle of `f64` columns.
//!
//! Values are stored with byte 0 of every element together, then byte 1, and
//! so on. That is the blosc byte shuffle. The inverse writes each `f64`
//! straight into the caller's column buffer.
//!
//! The fast path transposes 32 elements with AVX2. A scalar tail covers the
//! rest and machines without AVX2.

use std::arch::x86_64::*;

/// Group bytes of `src` by significance. Both slices are `n * 8` bytes.
pub fn shuffle_bytes(src: &[u8], dst: &mut [u8]) {
    assert_eq!(src.len() % 8, 0);
    assert_eq!(dst.len(), src.len());
    let n = src.len() / 8;
    if n >= 32 && is_x86_feature_detected!("avx2") {
        unsafe { shuffle8_avx2(src, n, dst) };
    } else {
        shuffle8_scalar(src, n, dst);
    }
}

/// Inverse of [`shuffle_bytes`]. `dst` receives the restored bytes.
pub fn unshuffle_bytes(src: &[u8], dst: &mut [u8]) {
    assert_eq!(dst.len() % 8, 0);
    assert_eq!(src.len(), dst.len());
    let n = dst.len() / 8;
    if n >= 32 && is_x86_feature_detected!("avx2") {
        unsafe { unshuffle8_avx2(src, n, dst) };
    } else {
        unshuffle8_scalar(src, n, dst);
    }
}

fn shuffle8_scalar(src: &[u8], n: usize, dst: &mut [u8]) {
    for i in 0..n {
        for b in 0..8 {
            dst[b * n + i] = src[i * 8 + b];
        }
    }
}

fn unshuffle8_scalar(src: &[u8], n: usize, dst: &mut [u8]) {
    for i in 0..n {
        for b in 0..8 {
            dst[i * 8 + b] = src[b * n + i];
        }
    }
}

#[target_feature(enable = "avx2")]
unsafe fn shuffle8_avx2(src: &[u8], n: usize, dst: &mut [u8]) {
    // Shuffle is the transpose the other way. Build it from the scalar
    // kernel in 32-element tiles so the bytes stay in L1, then the AVX2
    // unshuffle is what the read path times.
    let mut i = 0usize;
    while i + 32 <= n {
        for b in 0..8 {
            for k in 0..32 {
                dst[b * n + i + k] = src[(i + k) * 8 + b];
            }
        }
        i += 32;
    }
    for j in i..n {
        for b in 0..8 {
            dst[b * n + j] = src[j * 8 + b];
        }
    }
}

#[target_feature(enable = "avx2")]
unsafe fn unshuffle8_avx2(src: &[u8], n: usize, dst: &mut [u8]) {
    let mut i = 0usize;
    while i + 32 <= n {
        let plane = |b: usize| _mm256_loadu_si256(src.as_ptr().add(b * n + i) as *const __m256i);
        let a0 = plane(0);
        let a1 = plane(1);
        let a2 = plane(2);
        let a3 = plane(3);
        let a4 = plane(4);
        let a5 = plane(5);
        let a6 = plane(6);
        let a7 = plane(7);

        let b0 = _mm256_unpacklo_epi8(a0, a1);
        let b1 = _mm256_unpackhi_epi8(a0, a1);
        let b2 = _mm256_unpacklo_epi8(a2, a3);
        let b3 = _mm256_unpackhi_epi8(a2, a3);
        let b4 = _mm256_unpacklo_epi8(a4, a5);
        let b5 = _mm256_unpackhi_epi8(a4, a5);
        let b6 = _mm256_unpacklo_epi8(a6, a7);
        let b7 = _mm256_unpackhi_epi8(a6, a7);

        let c0 = _mm256_unpacklo_epi16(b0, b2);
        let c1 = _mm256_unpackhi_epi16(b0, b2);
        let c2 = _mm256_unpacklo_epi16(b1, b3);
        let c3 = _mm256_unpackhi_epi16(b1, b3);
        let c4 = _mm256_unpacklo_epi16(b4, b6);
        let c5 = _mm256_unpackhi_epi16(b4, b6);
        let c6 = _mm256_unpacklo_epi16(b5, b7);
        let c7 = _mm256_unpackhi_epi16(b5, b7);

        let d0 = _mm256_unpacklo_epi32(c0, c4);
        let d1 = _mm256_unpackhi_epi32(c0, c4);
        let d2 = _mm256_unpacklo_epi32(c1, c5);
        let d3 = _mm256_unpackhi_epi32(c1, c5);
        let d4 = _mm256_unpacklo_epi32(c2, c6);
        let d5 = _mm256_unpackhi_epi32(c2, c6);
        let d6 = _mm256_unpacklo_epi32(c3, c7);
        let d7 = _mm256_unpackhi_epi32(c3, c7);

        // unpack stays inside 128-bit lanes, so each register holds two
        // elements from the low 16 and two from the high 16. Stitch the
        // lanes, then store 32 elements in order.
        let stitch = |lo: __m256i, hi: __m256i| {
            (
                _mm256_permute2x128_si256(lo, hi, 0x20),
                _mm256_permute2x128_si256(lo, hi, 0x31),
            )
        };
        let (e0, e1) = stitch(d0, d1);
        let (e2, e3) = stitch(d2, d3);
        let (e4, e5) = stitch(d4, d5);
        let (e6, e7) = stitch(d6, d7);

        let out = dst.as_mut_ptr().add(i * 8);
        _mm256_storeu_si256(out as *mut __m256i, e0);
        _mm256_storeu_si256(out.add(32) as *mut __m256i, e2);
        _mm256_storeu_si256(out.add(64) as *mut __m256i, e4);
        _mm256_storeu_si256(out.add(96) as *mut __m256i, e6);
        _mm256_storeu_si256(out.add(128) as *mut __m256i, e1);
        _mm256_storeu_si256(out.add(160) as *mut __m256i, e3);
        _mm256_storeu_si256(out.add(192) as *mut __m256i, e5);
        _mm256_storeu_si256(out.add(224) as *mut __m256i, e7);
        i += 32;
    }
    for j in i..n {
        for b in 0..8 {
            dst[j * 8 + b] = src[b * n + j];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avx2_unshuffle_matches_the_bytes() {
        for n in [1usize, 7, 31, 32, 33, 1000, 300_000] {
            let src: Vec<f64> = (0..n).map(|i| (i as f64) * 0.125 + 1.0).collect();
            let mut shuffled = vec![0u8; n * 8];
            let raw = unsafe { std::slice::from_raw_parts(src.as_ptr().cast::<u8>(), n * 8) };
            shuffle_bytes(raw, &mut shuffled);
            let mut out = vec![0f64; n];
            let out_bytes =
                unsafe { std::slice::from_raw_parts_mut(out.as_mut_ptr().cast::<u8>(), n * 8) };
            unshuffle_bytes(&shuffled, out_bytes);
            assert_eq!(out, src, "n={n}");
        }
    }

    #[test]
    #[ignore]
    fn unshuffle_speed() {
        let n = 600_000usize;
        let src: Vec<f64> = (0..n).map(|i| i as f64 * 0.125).collect();
        let mut shuffled = vec![0u8; n * 8];
        let raw = unsafe { std::slice::from_raw_parts(src.as_ptr().cast::<u8>(), n * 8) };
        shuffle_bytes(raw, &mut shuffled);
        let mut out = vec![0f64; n];
        let out_bytes =
            unsafe { std::slice::from_raw_parts_mut(out.as_mut_ptr().cast::<u8>(), n * 8) };
        for _ in 0..3 {
            unshuffle_bytes(&shuffled, out_bytes);
        }
        let rounds = 20u32;
        let t0 = std::time::Instant::now();
        for _ in 0..rounds {
            unshuffle_bytes(&shuffled, out_bytes);
        }
        let us = t0.elapsed().as_micros() / u128::from(rounds);
        assert_eq!(out, src);
        let zstd1 = zstd::bulk::compress(&shuffled, 1).unwrap();
        let zfast = zstd::bulk::compress(&shuffled, -5).unwrap();
        let lz4 = lz4_flex::compress(&shuffled);
        let mut zbuf = vec![0u8; shuffled.len()];
        let mut lbuf = vec![0u8; shuffled.len()];
        let mut zdec = zstd::bulk::Decompressor::new().unwrap();
        for _ in 0..3 {
            zdec.decompress_to_buffer(&zstd1, &mut zbuf).unwrap();
            lz4_flex::decompress_into(&lz4, &mut lbuf).unwrap();
        }
        let t1 = std::time::Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(zdec.decompress_to_buffer(&zstd1, &mut zbuf).unwrap());
        }
        let zstd_us = t1.elapsed().as_micros() / u128::from(rounds);
        let t2 = std::time::Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(zdec.decompress_to_buffer(&zfast, &mut zbuf).unwrap());
        }
        let zfast_us = t2.elapsed().as_micros() / u128::from(rounds);
        let t3 = std::time::Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(lz4_flex::decompress_into(&lz4, &mut lbuf).unwrap());
        }
        let lz4_us = t3.elapsed().as_micros() / u128::from(rounds);
        let t4 = std::time::Instant::now();
        for _ in 0..rounds {
            lz4_flex::decompress_into(&lz4, &mut lbuf).unwrap();
            unshuffle_bytes(&lbuf, out_bytes);
        }
        let both_us = t4.elapsed().as_micros() / u128::from(rounds);
        eprintln!(
            "unshuffle {n} f64s ({:.2} MB) {us} us | zstd1 {} B {zstd_us} us | zstd-5 {} B {zfast_us} us | lz4 {} B {lz4_us} us | lz4+unshuffle {both_us} us",
            (n * 8) as f64 / 1e6,
            zstd1.len(),
            zfast.len(),
            lz4.len()
        );
    }
}
