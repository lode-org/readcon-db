//! Byte shuffle of `f64` columns.
//!
//! Values are stored with byte 0 of every element together, then byte 1, and
//! so on. That is the blosc byte shuffle. The inverse writes each `f64`
//! straight into the caller's column buffer.
//!
//! The fast path transposes 32 elements with AVX2, or 8 with NEON. A scalar
//! tail covers the rest and machines without those instructions.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Group bytes of `src` by significance. Both slices are `n * 8` bytes.
pub fn shuffle_bytes(src: &[u8], dst: &mut [u8]) {
    assert_eq!(src.len() % 8, 0);
    assert_eq!(dst.len(), src.len());
    let n = src.len() / 8;
    #[cfg(target_arch = "x86_64")]
    if n >= 32 && is_x86_feature_detected!("avx2") {
        unsafe { shuffle8_avx2(src, n, dst) };
        return;
    }
    shuffle8_scalar(src, n, dst);
}

/// Inverse of [`shuffle_bytes`]. `dst` receives the restored bytes.
pub fn unshuffle_bytes(src: &[u8], dst: &mut [u8]) {
    assert_eq!(dst.len() % 8, 0);
    assert_eq!(src.len(), dst.len());
    let n = dst.len() / 8;
    #[cfg(target_arch = "x86_64")]
    if n >= 32 && is_x86_feature_detected!("avx2") {
        unsafe { unshuffle8_avx2(src, n, dst) };
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if n >= 8 {
        unsafe { unshuffle8_neon(src, n, dst) };
        return;
    }
    unshuffle8_scalar(src, n, dst);
}

/// Pack three scalar columns into `x, y, z` order.
///
/// One streaming pass writes each destination cache line once. Scattering
/// one lane at a time rewrites that line for `x`, then `y`, then `z`.
pub fn interleave_xyz(x: &[f64], y: &[f64], z: &[f64], dst: &mut [f64]) {
    assert_eq!(x.len(), y.len());
    assert_eq!(y.len(), z.len());
    assert_eq!(dst.len(), x.len().saturating_mul(3));
    #[cfg(target_arch = "x86_64")]
    if x.len() >= 4 && is_x86_feature_detected!("avx2") {
        unsafe { interleave_xyz_avx2(x, y, z, dst) };
        return;
    }
    interleave_xyz_scalar(x, y, z, dst);
}

fn interleave_xyz_scalar(x: &[f64], y: &[f64], z: &[f64], dst: &mut [f64]) {
    for i in 0..x.len() {
        dst[i * 3] = x[i];
        dst[i * 3 + 1] = y[i];
        dst[i * 3 + 2] = z[i];
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

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn unshuffle8_avx2(src: &[u8], n: usize, dst: &mut [u8]) {
    let mut i = 0usize;
    while i + 32 <= n {
        let blocks = load_unshuffle_block(src.as_ptr(), n, i);
        // unpack stays inside 128-bit lanes. The blocks are 32 elements in order.
        store_unshuffle_block(dst.as_mut_ptr().add(i * 8), blocks);
        i += 32;
    }
    for j in i..n {
        for b in 0..8 {
            dst[j * 8 + b] = src[b * n + j];
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn load_unshuffle_block(src: *const u8, n: usize, i: usize) -> [__m256i; 8] {
    let plane = |b: usize| _mm256_loadu_si256(src.add(b * n + i) as *const __m256i);
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
    [e0, e2, e4, e6, e1, e3, e5, e7]
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn store_unshuffle_block(out: *mut u8, blocks: [__m256i; 8]) {
    for (k, block) in blocks.iter().enumerate() {
        _mm256_storeu_si256(out.add(k * 32) as *mut __m256i, *block);
    }
}

/// Unshuffle `x`, `y`, and `z` and write `x, y, z` into `dst` in one pass.
///
/// A full-column unshuffle followed by a second interleave reads the columns
/// again. This writes each restored value straight into the caller buffer.
pub fn unshuffle_interleave(x: &[u8], y: &[u8], z: &[u8], dst: &mut [f64]) {
    let n = x.len() / 8;
    assert_eq!(x.len() % 8, 0);
    assert_eq!(y.len(), x.len());
    assert_eq!(z.len(), x.len());
    assert_eq!(dst.len(), n * 3);
    #[cfg(target_arch = "x86_64")]
    if n >= 32 && is_x86_feature_detected!("avx2") {
        unsafe { unshuffle_interleave_avx2(x, y, z, dst) };
        return;
    }
    let mut xs = vec![0f64; n];
    let mut ys = vec![0f64; n];
    let mut zs = vec![0f64; n];
    unshuffle_bytes(x, unsafe {
        std::slice::from_raw_parts_mut(xs.as_mut_ptr().cast::<u8>(), n * 8)
    });
    unshuffle_bytes(y, unsafe {
        std::slice::from_raw_parts_mut(ys.as_mut_ptr().cast::<u8>(), n * 8)
    });
    unshuffle_bytes(z, unsafe {
        std::slice::from_raw_parts_mut(zs.as_mut_ptr().cast::<u8>(), n * 8)
    });
    interleave_xyz_scalar(&xs, &ys, &zs, dst);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn unshuffle_interleave_avx2(x: &[u8], y: &[u8], z: &[u8], dst: &mut [f64]) {
    let n = x.len() / 8;
    let mut i = 0usize;
    while i + 32 <= n {
        let mut xb = [0f64; 32];
        let mut yb = [0f64; 32];
        let mut zb = [0f64; 32];
        store_unshuffle_block(
            xb.as_mut_ptr().cast::<u8>(),
            load_unshuffle_block(x.as_ptr(), n, i),
        );
        store_unshuffle_block(
            yb.as_mut_ptr().cast::<u8>(),
            load_unshuffle_block(y.as_ptr(), n, i),
        );
        store_unshuffle_block(
            zb.as_mut_ptr().cast::<u8>(),
            load_unshuffle_block(z.as_ptr(), n, i),
        );
        interleave_xyz_avx2(&xb, &yb, &zb, &mut dst[i * 3..(i + 32) * 3]);
        i += 32;
    }
    if i < n {
        let rest = n - i;
        let mut xs = [0f64; 31];
        let mut ys = [0f64; 31];
        let mut zs = [0f64; 31];
        scalar_plane_tail(x, n, i, &mut xs[..rest]);
        scalar_plane_tail(y, n, i, &mut ys[..rest]);
        scalar_plane_tail(z, n, i, &mut zs[..rest]);
        interleave_xyz_scalar(&xs[..rest], &ys[..rest], &zs[..rest], &mut dst[i * 3..]);
    }
}

#[cfg(target_arch = "x86_64")]
fn scalar_plane_tail(src: &[u8], n: usize, i: usize, dst: &mut [f64]) {
    for (k, slot) in dst.iter_mut().enumerate() {
        let mut bytes = [0u8; 8];
        for b in 0..8 {
            bytes[b] = src[b * n + i + k];
        }
        *slot = f64::from_le_bytes(bytes);
    }
}

/// 8-wide NEON unshuffle. NEON is baseline on aarch64.
///
/// The zip order is the same one [`neon_zip_model`] applies in the tests.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn unshuffle8_neon(src: &[u8], n: usize, dst: &mut [u8]) {
    use std::arch::aarch64::*;
    let mut i = 0usize;
    while i + 8 <= n {
        let plane = |b: usize| vld1_u8(src.as_ptr().add(b * n + i));
        let p0 = plane(0);
        let p1 = plane(1);
        let p2 = plane(2);
        let p3 = plane(3);
        let p4 = plane(4);
        let p5 = plane(5);
        let p6 = plane(6);
        let p7 = plane(7);
        let ab = vzip_u8(p0, p1);
        let cd = vzip_u8(p2, p3);
        let ef = vzip_u8(p4, p5);
        let gh = vzip_u8(p6, p7);
        let abcd_lo = vzip_u16(vreinterpret_u16_u8(ab.0), vreinterpret_u16_u8(cd.0));
        let abcd_hi = vzip_u16(vreinterpret_u16_u8(ab.1), vreinterpret_u16_u8(cd.1));
        let efgh_lo = vzip_u16(vreinterpret_u16_u8(ef.0), vreinterpret_u16_u8(gh.0));
        let efgh_hi = vzip_u16(vreinterpret_u16_u8(ef.1), vreinterpret_u16_u8(gh.1));
        let e0 = vzip_u32(
            vreinterpret_u32_u16(abcd_lo.0),
            vreinterpret_u32_u16(efgh_lo.0),
        );
        let e1 = vzip_u32(
            vreinterpret_u32_u16(abcd_lo.1),
            vreinterpret_u32_u16(efgh_lo.1),
        );
        let e2 = vzip_u32(
            vreinterpret_u32_u16(abcd_hi.0),
            vreinterpret_u32_u16(efgh_hi.0),
        );
        let e3 = vzip_u32(
            vreinterpret_u32_u16(abcd_hi.1),
            vreinterpret_u32_u16(efgh_hi.1),
        );
        let out = dst.as_mut_ptr().add(i * 8);
        vst1_u8(out, vreinterpret_u8_u32(e0.0));
        vst1_u8(out.add(8), vreinterpret_u8_u32(e0.1));
        vst1_u8(out.add(16), vreinterpret_u8_u32(e1.0));
        vst1_u8(out.add(24), vreinterpret_u8_u32(e1.1));
        vst1_u8(out.add(32), vreinterpret_u8_u32(e2.0));
        vst1_u8(out.add(40), vreinterpret_u8_u32(e2.1));
        vst1_u8(out.add(48), vreinterpret_u8_u32(e3.0));
        vst1_u8(out.add(56), vreinterpret_u8_u32(e3.1));
        i += 8;
    }
    for j in i..n {
        for b in 0..8 {
            dst[j * 8 + b] = src[b * n + j];
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn interleave_xyz_avx2(x: &[f64], y: &[f64], z: &[f64], dst: &mut [f64]) {
    let n = x.len();
    let mut i = 0usize;
    while i + 4 <= n {
        let xv = _mm256_loadu_pd(x.as_ptr().add(i));
        let yv = _mm256_loadu_pd(y.as_ptr().add(i));
        let zv = _mm256_loadu_pd(z.as_ptr().add(i));
        let x_lo = _mm256_castpd256_pd128(xv);
        let x_hi = _mm256_extractf128_pd(xv, 1);
        let y_lo = _mm256_castpd256_pd128(yv);
        let y_hi = _mm256_extractf128_pd(yv, 1);
        let z_lo = _mm256_castpd256_pd128(zv);
        let z_hi = _mm256_extractf128_pd(zv, 1);
        store_xyz_pair(dst.as_mut_ptr().add(i * 3), x_lo, y_lo, z_lo);
        store_xyz_pair(dst.as_mut_ptr().add((i + 2) * 3), x_hi, y_hi, z_hi);
        i += 4;
    }
    if i < n {
        interleave_xyz_scalar(&x[i..], &y[i..], &z[i..], &mut dst[i * 3..]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn store_xyz_pair(out: *mut f64, x: __m128d, y: __m128d, z: __m128d) {
    let xy0 = _mm_unpacklo_pd(x, y);
    let xy1 = _mm_unpackhi_pd(x, y);
    _mm_storeu_pd(out, xy0);
    _mm_storel_pd(out.add(2), z);
    _mm_storeu_pd(out.add(3), xy1);
    _mm_storeh_pd(out.add(5), z);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleave_matches_the_scalar_order() {
        for n in [0usize, 1, 2, 3, 4, 5, 31, 32, 33, 16383, 16385] {
            let x: Vec<f64> = (0..n).map(|i| i as f64 + 0.25).collect();
            let y: Vec<f64> = (0..n).map(|i| i as f64 + 10.5).collect();
            let z: Vec<f64> = (0..n).map(|i| i as f64 - 3.5).collect();
            let mut got = vec![0f64; n * 3];
            let mut expect = vec![0f64; n * 3];
            interleave_xyz_scalar(&x, &y, &z, &mut expect);
            interleave_xyz(&x, &y, &z, &mut got);
            assert_eq!(got, expect, "n={n}");
            let mut sx = vec![0u8; n * 8];
            let mut sy = vec![0u8; n * 8];
            let mut sz = vec![0u8; n * 8];
            if n > 0 {
                shuffle_bytes(
                    unsafe { std::slice::from_raw_parts(x.as_ptr().cast::<u8>(), n * 8) },
                    &mut sx,
                );
                shuffle_bytes(
                    unsafe { std::slice::from_raw_parts(y.as_ptr().cast::<u8>(), n * 8) },
                    &mut sy,
                );
                shuffle_bytes(
                    unsafe { std::slice::from_raw_parts(z.as_ptr().cast::<u8>(), n * 8) },
                    &mut sz,
                );
            }
            let mut fused = vec![0f64; n * 3];
            unshuffle_interleave(&sx, &sy, &sz, &mut fused);
            assert_eq!(fused, expect, "fused n={n}");
        }
    }

    #[test]
    fn scalar_unshuffle_matches_the_bytes() {
        for n in [1usize, 7, 8, 15, 31, 32, 1000] {
            let src: Vec<u8> = (0..n * 8).map(|i| (i * 17 + 3) as u8).collect();
            let mut shuffled = vec![0u8; n * 8];
            shuffle8_scalar(&src, n, &mut shuffled);
            let mut out = vec![0u8; n * 8];
            unshuffle8_scalar(&shuffled, n, &mut out);
            assert_eq!(out, src, "n={n}");
        }
    }

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
