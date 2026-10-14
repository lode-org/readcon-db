# Candidate B: lossless numeric storage beside canonical CON text

readcon-db 0.1.8 should keep the CON text as the stored authority, compress that value per frame, and replace the uncompressed `frames_soa` table with a self-contained per-frame numeric codec. On the trajectory measured here, that numeric codec is pcodec. Cross-frame deltas, byte shuffle, fpzip, zfp reversible, and blosc2 did not beat it for single-frame reads.

The prototype lives in `design/candidate-b-proto`. It does not change the library.

## What 0.1.8 stores

`frames` is `Database<Bytes, Str>`. `put_index_puts` writes the CON span with one `frames.put` (`src/corpus.rs`). The key is a 12-byte `FrameKey` (big-endian trajectory id, big-endian frame index). `hash_frame_bytes` is xxh3-128 of those stored bytes (`src/keys.rs`), and that hash is what dedup uses. `get_frame_text` opens a read transaction and does one get. Nothing in that path compresses the span.

`frames_soa` is a second database of RCSO blobs (`src/cooked_soa.rs`). The header is 24 bytes (`RCSO`, version, natoms, flags, f64 dtype, reserved). The payload is little-endian f64 positions and, when present, forces and velocities. The file states that this blob is not authoritative: it has no symbols, cell, masses, JSON, or exact text, and it is not the input to xxh3, reindex, or export.

The production observation taken as given is that `data.mdb` is 1.24× the raw CON. The ratio measured on this corpus is reported separately below.

## f64 subtraction is not a residual

A lossless residual has to invert to the same bits. IEEE subtraction does not. On the packed position and force series of the MD trajectory, `(a - b) + b` differed in bits from `a` for 454,676 of 2,532,546 consecutive pairs (18.0%). The prototype therefore stores either the absolute f64s or a reversible integer residual: XOR of `to_bits()`, or wrapping `u64` subtraction of those bits. The NEB chord uses the same XOR. The chord itself is recomputed on decode from the two endpoints, so a rounded `(1-t)*R0 + t*RN` is still lossless: both sides evaluate the same expression.

## Bit-exact text

Two different questions got measured.

**Writer-owned frames.** Every MD frame and every NEB image was written with `ConFrameWriter` (`FloatFormat::RoundTrip`, which calls `write!("{value:e}")` in readcon-core 0.17.0) and then parsed back.

| check | MD (2188 frames) | NEB (11 images) |
|---|---:|---:|
| parse then rewrite equals the stored text | 2188 | 11 |
| rebuild through `ConFrameBuilder` from the f64s that were written | 2188 | 11 |
| rebuild from the f64s the parser returned | 2188 | 11 |
| position values whose bits changed | 0 / 1,266,852 | 0 / 4,785 |
| force values whose bits changed | 0 / 1,266,852 | 0 / 4,785 |
| energies whose bits changed | 0 | 0 |

On rustc 1.99.0, `{value:e}` kept the bits of a separate 100,000-value sample as well (`0.1` prints as `1e-1`, `1/3` prints as `3.333333333333333e-1`). Energy goes out through serde_json, and those bits also survived. For a frame this writer just emitted, a binary record plus the builder can reprint the text.

**Ingested legacy spans.** The same writer, applied to every `.con` and `.convel` in `resources/test`, matched none of the 7 files (8 frames). The first differences are spelling: `20.000000` against `2e1`, tabs against spaces, and JSON key order (`sections` and `energy` move). Those files are the bytes xxh3 would have stored. Regenerating them from coordinates produces a different blob, a different hash, and a different export.

The text tier stays the authority because dedup, reindex, and export are defined on the ingested span. A binary tier that can reprint writer-owned frames does not reprint an arbitrary span already in the database.

## Trajectory

Zenodo record 13741551: time-ordered revPBE0-D3 configurations of an excess proton in bulk water (`configurations.xyz`, `forces_revPBE0-D3.xyz`, `energies_revPBE0-D3.txt`).

- 2188 frames, 193 atoms (64 O, 129 H), cubic cell 12.42 Å, species order stable.
- SHA-256 configurations `a62ed76e281e9e3dae733c9218ccf8d5e9f0a45139ac73f45a6b95bbe6de83f9`, forces `3c6de4118d117d0f8a9764bf09bf7f709549252c7cd0d60dcc1f67fb50b57890`, energies `7829b97c08e84296511b8b5493da84af4d7f02651e5ef7a6b6ee00e97827f2fe`.
- Minimum-image consecutive RMSD: mean 0.355 Å, median 0.387 Å, max 5.588 Å. Four jumps above 1 Å, at frame pairs (1333, 5.588 Å), (1688, 4.841 Å), (2136, 1.004 Å), (2141, 1.145 Å). Deltas were still taken across those jumps.
- A separate pass over the same XYZ, every 10th pair: lag-1 median 0.385 Å, lag-10 0.791 Å, lag-50 1.284 Å, lag-200 2.469 Å, against a 200-pair random median of 3.970 Å. The series is time-ordered, and distant frames are not small residuals of each other.

CON text written for these frames is 31,584,938 bytes. Raw f64 positions plus forces are 20,269,632 bytes.

## MD codecs

Every row below decoded back to the same f64 bits, or the same text bytes, for every frame. Decode times are in process, after the bytes are already in memory: 250 frames drawn from a fixed shuffle, 8 warmup calls, then median and p99. They do not include an LMDB get. A zero latency means that row was sized and checked and not timed. Whole-trajectory rows report one full decode divided by 2188; a random frame still pays the full decode.

### CON text

| codec | bytes | vs text | median µs | p99 µs | access |
|---|---:|---:|---:|---:|---|
| plain CON | 31,584,938 | 1.000 |  |  | one value |
| zstd level 3 | 14,217,637 | 0.450 | 17.4 | 25.5 | one value |
| zstd level 19 | 12,506,021 | 0.396 |  |  | one value, size only |
| zstd level 3 + dict | 12,742,318 | 0.403 | 22.2 | 28.3 | one value, dict shared |
| zstd level 3, whole file | 12,303,515 | 0.390 |  |  | no per-frame get |
| zstd level 19, whole file | 9,589,201 | 0.304 |  |  | no per-frame get |

The dictionary is 32,768 bytes, counted once in the 12,742,318. It was trained on every 20th frame of this same trajectory, so a production dictionary trained once per topology and reused would be the fair comparison; this number is optimistic.

### Positions and forces

| codec | bytes | vs raw f64 | vs CON text | median µs | p99 µs | access |
|---|---:|---:|---:|---:|---:|---|
| f64 little-endian | 20,269,632 | 1.000 | 0.642 |  |  | one value |
| zstd-3 on the raw bytes | 17,650,054 | 0.871 | 0.559 | 12.0 | 16.3 | one value |
| byte shuffle + zstd-3 | 18,757,116 | 0.925 | 0.594 | 7.7 | 10.1 | one value |
| blosc2 shuffle + zstd-3 | 18,454,942 | 0.910 | 0.584 | 7.0 | 7.7 | one value |
| blosc2 bitshuffle + zstd-3 | 19,339,341 | 0.954 | 0.612 | 2.3 | 2.8 | one value |
| blosc2 shuffle, bytedelta, zstd-3 | 18,613,519 | 0.918 | 0.589 | 7.7 | 11.5 | one value |
| blosc2 delta, shuffle, zstd-3 | 18,596,055 | 0.917 | 0.589 | 8.9 | 12.5 | one value |
| pcodec, per frame | 7,434,922 | 0.367 | 0.235 | 8.9 | 12.2 | one value |
| fpzip lossless, per frame | 19,102,320 | 0.942 | 0.605 | 63.3 | 80.9 | one value |
| zfp reversible, per frame | 25,869,552 | 1.276 | 0.819 | 89.0 | 114.0 | one value |
| XOR previous, shuffle+zstd-3, keyframe 8 | 18,384,813 | 0.907 | 0.582 | 30.5 | 73.7 | chain to keyframe |
| XOR previous, shuffle+zstd-3, keyframe 32 | 18,346,959 | 0.905 | 0.581 | 121.0 | 279.2 | chain to keyframe |
| wrapping-sub previous, shuffle+zstd-3, keyframe 8 | 18,586,943 | 0.917 | 0.588 | 40.9 | 103.1 | chain to keyframe |
| XOR previous, pcodec, one chain | 17,638,938 | 0.870 | 0.558 |  |  | replay from frame 0 |
| fpzip lossless, whole series | 17,983,531 | 0.887 | 0.569 | 62.1 amortized |  | whole blob |
| zfp reversible, whole series | 26,179,440 | 1.292 | 0.829 | 73.4 amortized |  | whole blob |
| pcodec, time-major, whole series | 6,149,738 | 0.303 | 0.195 | 9.2 amortized |  | whole blob |

Byte shuffle and the blosc2 filter chain (shuffle, bitshuffle, shuffle-then-bytedelta, delta-then-shuffle, all with zstd level 3, one thread, typesize 8) land next to plain zstd of the raw f64 bytes, and slightly larger. fpzip at precision 64, whether one frame or the whole series, does the same. zfp reversible is larger than the uncompressed f64 array: the field is 3-wide (xyz contiguous), and the reversible stream header dominates.

pcodec (`pco` 1.0.4, `ChunkConfig::default`) is the outlier. Per frame it is 7.43 MB, 0.37 of the raw f64s and about half of zstd-3 text, with a median decode of 8.9 µs. XOR-ing the previous frame and then giving the residual to pcodec expands it to 17.64 MB: the XOR bit pattern is a worse pcodec input than the floats themselves. A single time-major pcodec of all frames reaches 6.15 MB. That is the sequential ceiling. Reading one frame decompresses the whole series (about 20 ms), so it does not replace a point get.

Keyframe XOR against the previous frame saves a few percent on shuffle+zstd (18.76 MB down to 18.38 MB at a keyframe every 8) and multiplies latency by the number of steps back to the keyframe. Wrapping subtraction is in the same place. On a trajectory whose lag-200 RMSD is several angstroms, the previous frame is a weak predictor.

## NEB band

Generated CI-NEB, not a downloaded band. Lennard-Jones fcc(111) argon, σ = 3.405 Å, ε = 0.1 eV, cutoff 2.5σ, spring 0.5 eV/Å². Slab 6×6×4 plus one adatom (145 atoms), orthogonal cell 22.932 × 19.860 × 27.362 Å, periodic in x and y, bottom two layers fixed. Endpoints are an fcc hollow and an hcp hollow. FIRE on the endpoints stopped at fmax 0.020 eV/Å after 423 steps. The band has 11 images. FIRE on the band ran the full 250 steps and stopped at nudged fmax 0.524 eV/Å, short of the 0.05 eV/Å tolerance. Stored forces are the true potential forces.

Energies (eV), image 0 through 10: −65.693, −65.692, −71.101, −71.384, −70.433, −71.135, −71.780, −71.566, −71.095, −71.421, −71.318. Images 0 and 1 sit about 5.4 eV above the rest, so this is a neighbour series with one hard step, not a converged barrier. Treat the ratios as a measurement of that series.

CON text is 172,475 bytes. Raw f64 positions plus forces are 76,560 bytes. Writer round trip was bit-exact on all 11 images, as in the table above.

| codec | bytes | vs raw f64 | median µs | access |
|---|---:|---:|---:|---|
| zstd-3 text | 64,258 |  | 13.0 | one value |
| zstd-3 text + dict (21,559 bytes, trained on every image) | 64,281 |  | 10.6 | one value |
| zstd-19 text, whole file | 44,279 |  |  | no per-frame get |
| pcodec, per image | 38,843 | 0.507 | 8.1 | one value |
| shuffle + zstd-3, per image | 46,170 | 0.603 | 6.1 | one value |
| XOR previous image, shuffle + zstd-3, from image 0 | 36,004 | 0.470 | 35.9 | chain |
| wrapping-sub, shuffle + zstd-3, keyframe 8 | 37,900 | 0.495 | 24.2 | chain |
| XOR previous, pcodec, from image 0 | 35,718 | 0.467 |  | replay from image 0 |
| chord positions + XOR forces vs previous image | 37,231 | 0.486 | 22.5 cold | endpoints + one residual |
| fpzip lossless, whole band | 37,110 | 0.485 | 29.2 amortized | whole blob |
| pcodec, time-major, whole band | 40,180 | 0.525 | 2.7 amortized | whole blob |
| zfp reversible, per image | 66,220 | 0.865 | 41.3 | one value |

The dictionary does not pay for itself on 11 images. Neighbour prediction does help here, unlike the MD series: shuffle+zstd falls from 46,170 to 36,004 bytes when each image is an XOR against the previous one. pcodec per image (38,843) stays the best one-get numeric form. Time-major pcodec of the whole band is larger than per-image pcodec; eleven frames is too short for that layout.

The chord stores the two endpoint position blocks independently and each middle image as the XOR of its position bits against `(1-t)*R0 + t*RN`. Forces are an XOR against the previous image, so a force read can still walk. A cold middle image was 22.5 µs (both endpoints, the chord, one residual). With the endpoints cached it is one residual. The chain from image 0 costs a decompress per step and measured 35.9 µs median across the band. Size is close: 37,231 bytes for the chord scheme, 36,004 for the pure neighbour chain.

## LMDB on this corpus

`ConCorpus::append_trajectory_path` of the 31,584,938-byte CON file, then `recook_all`.

| quantity | bytes | ratio |
|---|---:|---:|
| raw CON text | 31,584,938 | 1 |
| `data.mdb` after ingest (text and indexes) | 37,314,560 | 1.181× text |
| `data.mdb` after `recook_all` | 64,303,104 | 2.036× text |
| sum of `frames_soa` values | 20,322,144 | raw f64 plus 24×2188 header bytes |

The stated production ratio of 1.24× is a different corpus. This one lands at 1.18× with indexes and before the SoA table. The SoA table then roughly doubles the file. Its 20,322,144 bytes are the 20,269,632 raw f64 bytes plus a 24-byte header on each of 2188 frames.

`get_frame_text` (a new transaction per call) median 4.0 µs, p99 8.1 µs. The same keys under one transaction, `get_frame_texts`, 2.1 µs per frame. `get_cooked_soa_bytes` median 3.4 µs.

Three value-only environments (12-byte `FrameKey`, one `Database<Bytes, Bytes>`, no secondary indexes, map 512 MiB, synced) separate page overhead from the index databases:

| values | `data.mdb` | vs the values | vs raw CON |
|---|---:|---:|---:|
| plain CON text | 35,934,208 | 1.138× | 1.138× |
| zstd-3 text | 18,010,112 | 1.267× | 0.570× |
| shuffle + zstd-3 numerics | 26,972,160 | 1.438× | 0.854× |

Page overhead stays after compression. It is larger, as a fraction, on the smaller values. zstd-3 text inside LMDB is still 0.57 of the raw CON file.

An in-memory zstd-3 text decode is 17 µs median against a 4 µs LMDB get of the plain span, so a compressed text value remains one get and the decode is the larger term. pcodec's 8.9 µs sits in the same range.

## Recommendation

Store each `frames` value as zstd of the exact CON bytes, and hash the bytes after decompression. Level 3 is the measured operating point (14.22 MB, 17 µs). A dictionary trained per topology is worth it when the frame is large and the dictionary is reused: here it moved the level-3 total from 14.22 MB to 12.74 MB, dictionary included. Level 19 per frame reaches 12.51 MB without a dictionary; the whole-file level-19 size of 9.59 MB is the ceiling you only get by giving up the point get. Dedup, reindex, and export keep seeing the original span.

Leave regeneration as an export of writer-owned frames. It was bit-exact on every frame this study wrote, and it was not bit-exact on any legacy file in `resources/test`. Making binary the authority would change the xxh3 identity of spans the database already stores.

Replace RCSO with one pcodec blob per frame, positions and forces together, still marked non-authoritative. On this trajectory that is 7.43 MB instead of 20.32 MB, one get, 8.9 µs to decode, and the same f64 bits the text parses back to. Shuffle, blosc2, fpzip, and zfp reversible do not earn a place in that slot on data that actually moves.

Keep a keyframe XOR, or the NEB chord, as an option for bands and for trajectories whose consecutive RMSD is a small fraction of the random-pair RMSD. On this MD set the keyframe chain bought about 2% over independent shuffle+zstd and lost to independent pcodec by more than half. On the generated band the chord matches the neighbour chain in size and keeps a middle image to a constant number of reads.

## Reproduction

The measurement driver is `design/candidate-b-proto`. It links two static libraries that are not in the repo and are not part of readcon-db:

- zfp 1.0.1, built from `src/*.c` with gcc. The cmake C++ probe could not link libstdc++ in this environment.
- c-blosc2 2.15.2, cmake static library, plugins on, so `blosc2_init` registers bytedelta. Filters run from index 0. The shuffle-then-bytedelta mode is filters `[0,0,0,0, shuffle, bytedelta]` with metadata 8 on the bytedelta slot.

Both were installed under `/tmp/prefix`, which `build.rs` searches. The trajectory files are read from `/tmp/traj` and are not committed.

```
cd design/candidate-b-proto
cargo +stable build --release
./target/release/candidate-b
```

`results.json` next to the crate is the record of the run above. Codec crates: `pco` 1.0.4, `fpzip-rs` 0.1.0, `zstd` 0.13.3, readcon-core 0.17.0, heed 0.21.0.
