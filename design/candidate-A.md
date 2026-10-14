# Campaign storage for mixed saddle-search corpora

readcon-db 0.1.8 should keep sharded LMDB, one get per frame, and xxh3-128 on the exact CON bytes. The change that pays for itself is to store each `frames` value as a zstd frame of that same text. A trained dictionary does not. Compressing `frames_soa` does not either.

Measured on one mixed corpus (a 600-frame Lennard-Jones MD run and a 19-image vacancy-hop NEB, 619 distinct frames, full binary64 digits): the plain frames LMDB is 37,285,888 bytes, 1.047× the 35,611,626 bytes of CON text. The same table with a one-byte codec tag and a zstd frame is 17,383,424 bytes. A warm random get of the plain text costs 2.51 µs in the Python harness; the zstd value costs 49.7 µs at level 3 and 53.1 µs at level 1. Level 1 and level 3 land on the same LMDB high-water mark. Level 9 is larger and slower. The numeric SoA, left as raw little-endian f64, is a 12,476,416-byte LMDB and a 1.11 µs get. Byte-shuffle plus zstd on each frame makes that file larger (12,509,184) and the get slower (20.4 µs). A 32-frame XOR chunk shrinks the numeric file to 9,424,896 bytes and raises a random get to 608 µs.

## What 0.1.8 stores

`ConCorpus.frames` is a Heed `Database<Bytes, Str>` (`src/corpus.rs`). The key is `FrameKey::to_bytes`: 8 bytes of big-endian trajectory id and 4 bytes of big-endian frame index, 12 bytes total. One `get` returns the CON text. xxh3-128 (`hash_frame_bytes`) is taken on those UTF-8 bytes. `frame_by_hash` records the first key with that hash; the text is still stored under every key. `frames_soa` is an optional uncompressed RCSO blob (magic `RCSO`, little-endian f64 positions and optional forces and velocities) and is not authoritative. MPI ingest has two routes over a caller communicator: one writer for the whole env, or one writer per shard (`traj_id % n_shards`). Ranks parse and hash a block, allgather the 16-byte hashes, and `MPI_Alltoallv` the text of the first occurrence of each hash. On campaign files the full `data.mdb`, indexes included, is 1.24× the raw CON. This study did not rebuild those indexes.

A saddle-search campaign mixes systems. Trajectories differ in atom count, species, cell, and which sections exist. MD frames move by a fraction of an angstrom. NEB images on one band share a stoichiometry and differ by a hop. Selection returns keys, then analysis point-reads them. The stored bytes have to round-trip to the same UTF-8 the hash was computed on, including every binary64 digit.

## Survey

**H5MD.** HDF5 groups for particles, positions as a time-dependent dataset, compression delegated to HDF5 filters (shuffle, gzip, szip). A file is one simulation, or a handful of particle groups of one shape. A campaign of unrelated cells is many files or a ragged tree. A random frame reads a chunk, not a 12-byte key. Parallel HDF5 is a different ingest protocol from the sharded single-writer envs already in `src/shard.rs` and `src/mpi_ingest.rs`. This crate already writes H5MD as interchange (`export_h5md`) and leaves CON as the authority. That split should stay.

**TNG.** GROMACS' container for one molecular system: general blocks plus particle blocks, with a lossless double path and a lossy XTC-like path. The lossy path drops binary64. The lossless path is a sequential file with a frame index, with no composition, fmax, or NEB-bead indexes, and with one topology for the file. Mixed campaigns do not fit one topology.

**XTC.** Lossy. Coordinates are scaled to integers at a caller precision (GROMACS default 1000, meaning 0.001 nm), then delta-coded and bit-packed. Smooth MD shrinks a lot. The discarded bits change xxh3, forces, and saddle comparisons. Fixed atom count. Random access needs an external index and a scan of block headers. Usable as a visualization export. Unusable as the authority value.

**ASE db.** One SQLite row per structure, positions as raw float64 blobs, metadata in side tables. Mixed systems fit, which is why heterogeneous sets end up there. There is no trajectory codec, the point read is a SQL row decode, and this repo's own timings already put that path far behind an LMDB get. Nothing in the ASE schema solves the 1.24× size.

**fairchem / OC20 LMDB.** Training sets store one structure per LMDB key, in shards, because the reader is a random sample. The public writers pickle a numeric dict into the value and do not apply a trajectory codec. That confirms the engine (sharded LMDB, one record per frame, random get) and the reason they skip compression: their values are already packed float32, and a random training read is sensitive to decode cost. CON text with full digits is the opposite payload. Their choice of engine stands. Their choice to leave values uncompressed does not transfer.

**zarr / blosc.** A chunked numeric array with a shuffle or bitshuffle filter and a fast codec (lz4 or zstd) is the right compressor for a rectangular `(T, N, 3)` block of one system. A directory of chunk files is a bad campaign container: mixed `N` means one array per trajectory, and one file per chunk is the Lustre metadata pattern the sharded `data.mdb` layout was built to avoid. The codec itself is what prototype B measures, inside LMDB, so the container does not confound the size.

**zstd dictionaries.** A dictionary is how zstd compresses many small records that share a header. It preserves one-value random access. It also becomes a corpus-global object: every shard must carry the same bytes, or `join-drained` has to recompress. Prototype A trains one and measures it.

**BGZF.** Blocked gzip, 64 KiB uncompressed members, virtual offsets, and legal concatenation. Concatenation is the attractive part for a shard merge. The merge this crate actually does (`join-drained`, `compact-join`) copies keyed records and rebuilds indexes; a concatenated byte stream has neither keys nor indexes. Deflate is a weaker codec than zstd, and a random frame inflates a whole member. BGZF was not prototyped.

## Trajectory

Both systems are Lennard-Jones, integrated in double precision, and written with 17 significant digits (Python `format(x, ".16e")`, checked to parse back to the same f64). CON headers follow the files under `resources/test/` (title, JSON metadata, box, the `0 0` / `218 0 1` lines, then per-component coordinate and force blocks). The two trajectories share no species and no atom count.

**MD.** 4×4×4 FCC, lattice constant 3.615 Å, 256 atoms (205 Cu, 51 Ni). Pair σ is set so the LJ minimum lies on the FCC nearest neighbour (Ni σ from a 3.52 Å lattice). ε is 0.17 eV (Cu) and 0.22 eV (Ni), cutoff 6.0 Å, which is under half the box. Velocity Verlet, Δt = 5 fs, 600 stored steps, NVE. Initial velocities are Maxwellian at 400 K with the centre-of-mass velocity removed. The unit conversion used is CODATA 2018: acceleration in Å/fs² is `(F in eV/Å) / (m in amu) × 9.648533e-3`. Total energy goes from −327.893 eV to −327.851 eV. The relative span over the run is 2.51×10⁻⁴. Root-mean-square displacement per step stays between 0.0091 Å and 0.0198 Å.

**NEB.** 3×3×3 FCC aluminium, lattice constant 4.05 Å, one vacancy, 107 atoms. The hopping atom moves from a nearest-neighbour site into the vacancy. Endpoints are relaxed to a max force of 8.0×10⁻⁷ eV/Å. Nineteen images, 60 nudged-elastic-band iterations, spring constant 0.8 eV/Å², improved tangent, minimum-image displacements. The barrier is 0.860 eV. The saddle is image 9. The two endpoints have the same energy (−143.0375323883842 eV) and the profile is symmetric, which is the expected vacancy hop.

All 619 CON texts are distinct (SHA-256 prefix `165c617a7ce72f4b`). Dedup therefore removes nothing on this file; the hash rule still matters for campaigns that repeat an endpoint across bands.

Host: CPython 3.12.3, libzstd 1.5.7, LMDB page size 4096, CPU string `Intel(R) Xeon(R) Processor`. The harness is `scratch/storage-a/measure.py` (LMDB table) and `scratch/storage-a/sweep.py` (levels, held-out dictionary, raw SoA, bitshuffle, tiny frame). Outputs are `results.json` and `sweep.json` beside those scripts. `posix_fadvise(DONTNEED)` did not change latency on these 10–40 MB files, so the table below is warm-cache only. A cold Lustre read of a multi-gigabyte env was not measured.

## Prototype A — per-frame zstd of the CON bytes

Each frame is one LMDB value under the real 12-byte key. The value is a `0x01` tag plus a zstd frame. Decompress must return the original UTF-8; the harness checks that. A 32 KiB dictionary was trained on 32 evenly spaced MD frames and 7 NEB images (`train_dictionary`, k=128, d=8) and applied to the other frames as well as the training frames. The dictionary bytes are stored once in the env.

| store | value bytes | LMDB used bytes | used / raw CON | warm µs / frame |
|---|---:|---:|---:|---:|
| plain CON | 35,611,626 | 37,285,888 | 1.047 | 2.51 |
| zstd level 3 + tag | 15,364,807 | 17,383,424 | 0.488 | 49.7 |
| zstd level 1 + tag | 15,225,636 | 17,383,424 | 0.488 | 53.1 |
| zstd level 3 + 32 KiB dict | 15,112,412 | 17,391,616 | 0.488 | 65.9 |

Level 1, 3, and 9 without the tag, in memory, on the same bytes:

| level | payload bytes | raw / payload | compress | decompress |
|---|---:|---:|---:|---:|
| 1 | 15,225,017 | 2.339 | 367 MB/s | 56.7 µs |
| 3 | 15,364,188 | 2.318 | 209 MB/s | 54.1 µs |
| 9 | 15,694,514 | 2.269 | 46 MB/s | 61.9 µs |

Level 9 loses on size, on compress rate, and on decompress rate. The in-memory decompress times include the Python call; the LMDB column is the number to compare with the plain get, because both run in the same loop. Level 1's payload is 0.9% smaller than level 3's, and on this frame size (~25 KB compressed) the difference sits inside LMDB overflow slack, so the files match at 17,383,424 bytes. Level 1 compresses about 1.8× faster. Level 3 decompresses about 3 µs faster inside LMDB.

Per trajectory at level 1, no dictionary: the MD text shrinks by 2.333× (35,297,084 → 15,129,210) and the NEB text by 3.283× (314,542 → 95,807). The NEB frame is shorter (16.6 KB against 58.8 KB) and a larger fraction of it is repeated JSON and component labels, so it compresses further. Both systems shrink. One codec covers the mix.

Held-out frames at level 3, dictionary against plain zstd, no tag:

| subset | frames | plain zstd | with dictionary | saved |
|---|---:|---:|---:|---:|
| MD, not in the training sample | 568 | 14,458,979 | 14,201,236 | 1.78% |
| NEB, not in the training sample | 12 | 61,682 | 60,616 | 1.73% |

The dictionary generalises across both systems and across frames it did not train on. The gain is under 2%. Inside LMDB that gain is smaller than a page: the dictionary env is 8,192 bytes larger than the plain zstd env, and each random read slows from 49.7 µs to 65.9 µs. The 32 KiB dictionary is a join hazard for a 1.8% payload cut that the page size then erases. It should not ship.

A four-atom CON with forces, velocities, and full digits is 1,408 bytes and compresses to 425. Typical frames do not expand. A writer that compares the two lengths and stores raw UTF-8 when zstd grows is still the right guard for degenerate one-line records.

## Prototype B — shuffled f64 chunks

`frames_soa` today is uncompressed f64. The alternative is the blosc/zarr numeric filter: little-endian f64, HDF5-style byte shuffle (the eight byte planes separated), then zstd level 3. An optional XOR of each frame's bits with the previous frame inside the chunk is a lossless predictor. Chunks never cross a trajectory, because `N` and the section set differ. Random access stores a 16-byte locator under the frame key (chunk key plus local index) and a second get loads the chunk. Chunk length 1 is a single get of that frame plus the locator the harness still writes.

Raw SoA is 11,156,784 bytes (MD 256×9 f64 × 600 = 11,059,200; NEB 107×6 f64 × 19 = 97,584).

| store | chunk bytes | LMDB used | warm µs / frame |
|---|---:|---:|---:|
| raw f64, one value per frame | 11,156,784 | 12,476,416 | 1.11 |
| shuffle + zstd, chunk 1 | 10,179,186 | 12,509,184 | 20.4 |
| shuffle + zstd, chunk 8 | 10,047,701 | 10,280,960 | 123 |
| XOR + shuffle + zstd, chunk 8 | 9,607,818 | 9,863,168 | 143 |
| shuffle + zstd, chunk 32 | 9,776,728 | 9,875,456 | 564 |
| XOR + shuffle + zstd, chunk 32 | 9,304,176 | 9,424,896 | 608 |

Per-frame shuffle+zstd compresses the binary by 1.096× (11,156,784 / 10,179,186) and the LMDB file gets larger than the raw f64 file, because of the 9-byte chunk headers and the locators. The get is 18× slower than copying the raw f64 (20.4 µs against 1.11 µs). Thermal mantissas are close to entropy. The repeated exponent bytes are the whole of the 8.8% payload cut.

A 32-frame XOR chunk is the best size in the set, 9,424,896 bytes against 12,476,416 for raw f64, a 1.32× smaller file. A random frame then decompresses the whole chunk: 608 µs against 1.11 µs. That is the zarr tradeoff, and it is the wrong one for a select that returns scattered keys. An NEB band of 19 images would happily be one chunk for a sequential "load this band" call. The stored form is shared with MD point reads, and those dominate the latency average here (600 of 619 frames).

Full-array bitshuffle plus zstd, in memory, does not change the conclusion. At chunk 1 it produces 9,960,517 bytes against 10,173,615 for byte shuffle (2.1% smaller). At chunk 32 it produces 9,596,413 against 9,776,548 (1.8% smaller). Round-trip of the bit transpose was checked. Bitshuffle is not a different design.

## Design

**Authority value.** `frames` becomes `Database<Bytes, Bytes>`. A value that is valid UTF-8 is a 0.1.8 CON text and is returned as-is. A new value starts with `0xFD 0x01` (the lead byte is illegal UTF-8, so it cannot be a legacy frame) and the rest is one zstd frame of the UTF-8. Anything else is a corrupt value and is an error, not a parse. The zstd frame carries its own level. Ingest writes level 1: on this corpus it has the smallest payload, the fastest compress, and the same LMDB occupancy as level 3. Readers do not branch on the level. If a later campaign is dominated by nearly identical frames, the writer can raise the level without a migration.

**Hash and dedup.** xxh3-128 stays on the decompressed UTF-8, in `hash_frame_bytes`, before the put. `frame_by_hash` / `hash_by_frame` do not change. Two encodings of one text (legacy raw, level 1, a future level) dedup together. `reindex` and `touch_trajectory_blobs` decompress before they hash or checksum. A checksum of the compressed bytes would stop tracking the CON.

**Where the CPU goes.** Compression runs in the existing prepare step, outside the write transaction, next to index-key construction. The transaction only puts bytes. Single-writer MPI and sharded MPI keep the hash allgather on the 16 plaintext bytes. The `Alltoallv` payload for a first occurrence becomes the tagged zstd bytes (about 2.3× less on this corpus). The owner stores those bytes. A duplicate key still receives its own copy of the compressed value, which is what serial ingest does with the `String` today. `kept` holds the compressed buffer.

**Shards, drain, join.** No dictionary, so a shard is readable alone and `join-drained` / `compact-join` keep copying values. `INPUT_MAP_FACTOR` (3× the raw CON file) still covers the write: the new values are smaller, and the indexes are unchanged. Reserving the map from the compressed size alone would under-count the index pages.

**`frames_soa`.** Leave it uncompressed RCSO. The numeric hot path is a 1.11 µs copy of the f64 block, against 20.4 µs for a per-frame compressed block that does not even shrink the file. Chunked compression is a sequential-export choice, and H5MD already covers "one trajectory, rectangular arrays." Promoting SoA to authority would drop symbols, cell, JSON metadata, and the exact bytes xxh3 is defined on. Raw SoA is 11.2 MB here and the zstd CON is 15.2 MB; deleting the CON would be a smaller file and a different database.

**Ingest default and the tiny-frame guard.** Compare compressed length with the UTF-8 length. If zstd does not shrink the frame, write the legacy UTF-8 form. The four-atom probe shrank (1,408 → 425), so ordinary frames take the zstd path.

## Risks

The C library (or a pure-Rust encoder/decoder of the same frame format) becomes a link dependency of the Rust crate and of every binding, including the wrapdb and fpm packages, which today do not ship a compressor. The frame format is stock zstd, so the writer and the reader can be different implementations. They have to agree on the `0xFD 0x01` prefix.

Opening `frames` as `Bytes` instead of `Str` will reject nothing that 0.1.8 wrote, and it will reject a new value in any reader that still asks Heed to decode `Str`. Old envs and new envs will sit next to each other during a campaign; `get_frame_text` has to accept both. A tool that snapshots `data.mdb` and parses values as UTF-8 outside this crate will see illegal UTF-8 on new frames.

Hashing the compressed bytes would make dedup depend on the level and the encoder. The bug is silent: identical geometries stop matching after a recompress. The hash input has to be the UTF-8, and tests that round-trip a frame through zstd need to assert the hash is unchanged.

A truncated or bit-flipped value must fail at decompress. Passing the compressed bytes to the CON parser produces a confusing parse error and, worse, could hash them if a caller forgets the order.

Level 9 is the setting that looks like the archival choice and is the worst of the three on this file. Document the measured ingest level next to the writer so a later change does not "upgrade" to it for size.

`touch_trajectory_blobs` exists to force the campaign to pay for payload bytes. If it checksums the compressed form, regressions in decompress will not show up there.

The dictionary result is easy to overturn with a different corpus of tiny, header-dominated frames. On the frames this campaign actually writes (16 KB and 59 KB of full-digit text) the held-out saving is 1.8% and the file got bigger. Revisit the dictionary only if a measured campaign of small frames shows a file-size win after LMDB pages, and ship the dictionary bytes inside each shard so a shard snapshot still opens.

Chunked SoA will keep looking attractive in a size-only table (9.4 MB against 12.5 MB). The random-get column is the one that matches `get_frame_text` / `get_positions` after a select: 608 µs against 1.11 µs for the raw f64. A bulk loader that knows it will read every frame of one trajectory can decompress a chunk once; that path is an export, not the on-disk key layout.

These absolute microseconds are CPython calling libzstd and LMDB. The plain get and the zstd get share the interpreter, so the gap is the codec, and a Rust port should be judged by a new measurement of the same corpus rather than by subtracting an imagined constant. The size ratios are properties of the bytes and of the 4 KiB page, and those will carry over to Heed.

The warm-cache numbers hide a cold-read effect that a large Lustre env might show in the other direction: fewer overflow pages per frame means less I/O per random key. This 36 MB file stayed cached after an fadvise drop. Do not claim a cold-read win from this run. The on-disk reduction from 37.3 MB to 17.4 MB for the frames table is already measured.
