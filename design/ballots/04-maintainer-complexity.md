# Ballot: maintainer (complexity)

## Vote

`Vote: A`

A is the smallest design that captures the size number I believe. On the mixed corpus the plain `frames` table is 37,285,888 bytes, 1.047× the 35,611,626 bytes of CON text. A one-byte tag plus a per-frame zstd frame brings that table to 17,383,424 bytes, 0.488× raw. Level 1 (payload 15,225,636) and level 3 (payload 15,364,807) land on that same LMDB high-water mark. A warm get moves from 2.51 µs to 49.7 µs at level 3 and 53.1 µs at level 1. I will pay that latency and one link dependency, because nothing simpler than a codec produces 0.488×, and the codec fits the value we already get by a 12-byte key.

I do not vote a hybrid. The measured leftovers have a smaller fix than another design. Page slack is why level 1 and level 3 share one file size, and the file is already 0.488×. The single-writer ingest collapse is the env we already shard. The recook doubling is the optional RCSO table, which can stay raw.

## Dependency and migration cost

`Cargo.toml` today depends on heed 0.21, readcon-core 0.17.0, thiserror, serde, serde_json, and xxhash-rust. `pyo3` and `mpi` are optional. There is no zstd and no pcodec. The library is built as `lib`, `cdylib`, and `staticlib`. Meson and CMake run cargo and install that library plus the hand-written headers. `fortran/ReadConDb/fpm.toml` links `readcon_db` and nothing else. The wrapdb package is the same library. A compressor that lands in `libreadcon_db` is therefore a dependency of Rust, C, C++, Python, and Fortran at once. That cost is acceptable once. It is not acceptable twice, and it is not acceptable as a new `-l` that each of those packages has to discover on its own.

The migration that belongs with that one dependency is the one A names. `frames` is `Database<Bytes, Str>` in `src/corpus.rs`. A legacy value that is valid UTF-8 stays a 0.1.8 frame. A new value starts with `0xFD 0x01` and one zstd frame. xxh3-128 is taken on the decompressed UTF-8, so `frame_by_hash`, `reindex`, and `touch_trajectory_blobs` keep identifying the CON text. Shards stay directories of full LMDB environments (`src/shard.rs`). With no dictionary, `join-drained` and `compact-join` keep copying values, and a shard snapshot still opens.

## Design A

This is the change I will ship. Ingest writes level 1: smallest payload of the three levels measured, compress at 367 MB/s against 209 MB/s for level 3 and 46 MB/s for level 9, and the same 17,383,424-byte LMDB file as level 3. Level 9's in-memory payload is 15,694,514 bytes, larger and slower. MD text shrinks 2.333× (35,297,084 → 15,129,210) and NEB text 3.283× (314,542 → 95,807). One codec covers both shapes.

The dictionary does not. Held-out frames save 1.78% on MD (14,458,979 → 14,201,236) and 1.73% on NEB (61,682 → 60,616). Inside LMDB the dictionary env is 17,391,616 bytes, 8,192 bytes larger than plain zstd, and the get moves from 49.7 µs to 65.9 µs. I accept A's rejection of it.

`frames_soa` stays raw RCSO. Raw f64 is a 12,476,416-byte LMDB and a 1.11 µs get. Byte-shuffle plus zstd per frame makes the file 12,509,184 bytes and the get 20.4 µs. A 32-frame XOR chunk reaches 9,424,896 bytes and 608 µs. That random-get column is the one that matches a select. H5MD remains the rectangular export. Promoting SoA to authority would drop the symbols, the cell, the JSON, and the exact bytes xxh3 is defined on.

Compression runs in the existing prepare step, outside the write transaction. The hash allgather stays 16 plaintext bytes. `Alltoallv` of a first occurrence carries the tagged frame, about 2.3× less on this corpus. `INPUT_MAP_FACTOR` stays 3× the raw CON so the unchanged indexes still fit. The four-atom probe shrank from 1,408 bytes to 425; if a frame does not shrink, the writer stores the legacy UTF-8. Readers do not branch on the zstd level, because the frame header carries it.

The 2.51 → ~50 µs gap is CPython calling libzstd and LMDB in one loop. The size ratios are properties of the bytes and of the 4 KiB page, and those carry to Heed. A Rust port gets judged by a new measurement of the same corpus. The warm-cache run does not support a cold Lustre claim. The on-disk drop from 37.3 MB to 17.4 MB for the frames table is already measured, and it is the number this design exists to take.

## Design B

B compresses the same authority text and then replaces RCSO with pcodec. The text half, on a different corpus, is zstd level 3 at 14,217,637 bytes, 0.450× of 31,584,938, 17.4 µs median in memory once the bytes are already resident. A value-only LMDB of that text is 18,010,112 bytes, 0.570× the raw CON, so 4 KiB page slack survives compression. Full `data.mdb` after ingest is 37,314,560 bytes, 1.181× text. `recook_all` makes it 64,303,104 bytes, 2.036× text. The added mass is the SoA table: 20,322,144 bytes of values on top of 20,269,632 raw f64.

pcodec (`pco` 1.0.4, `ChunkConfig::default`) is a real measurement on that optional table: 7,434,922 bytes, 0.367× raw f64, 8.9 µs median, bit-exact, one get. Shuffle, blosc2, fpzip, and zfp reversible miss that slot. XOR of the previous frame then fed to pcodec expands to 17,638,938 bytes. A time-major pcodec of the whole series reaches 6,149,738 bytes and costs a full-series decode for one frame. The prototype also linked zfp 1.0.1 and c-blosc2 2.15.2. The recommendation drops those two and still asks the library to link zstd and `pco`.

I will not take `pco`. `frames_soa` is not authoritative. Writer-owned frames reprinted bit-exact; the seven legacy files in `resources/test` matched 0/7, so a binary record cannot be the xxh3 identity of spans already stored. The cook is already optional. On A's get column the raw f64 copy is 1.11 µs. A campaign that cannot afford 1.181× → 2.036× skips `recook_all`. A second numeric codec in every binding, plus an RCSO migration, is a larger change than declining an optional table.

B's dictionary cut level-3 text from 14,217,637 to 12,742,318 including a 32,768-byte dictionary trained on every 20th frame of the same trajectory. B calls that comparison optimistic. On the 11-image band the dictionary made the payload larger (64,258 → 64,281). Level 19 per frame reaches 12,506,021 without a dictionary, and the whole-file level-19 size of 9,589,201 gives up the point get. Neither is a reason to complicate the value A already specifies.

## Design C

C takes LMDB down to an index. Each MPI rank appends zstd-3 frames to its own segment files. The env stores segment, offset, length, and xxh3. The quoted size is 0.495× raw against 0.60× with compressed values inside LMDB. The quoted ingest is 6.7k frames/s at one rank and 19.9k at eight, against a single-writer LMDB that goes from 14.6k to 3.5k.

The size increment over an in-LMDB zstd value is the page slack. A already measured that slack: level 1 and level 3 differ in payload and share 17,383,424 bytes, and the table is 0.488× raw anyway. B's value-only env is 0.570×, the same kind of remainder. Closing 0.60× toward 0.495× means a second file format, a locator schema, crash recovery for appends, and a read path that is no longer one LMDB get. C does not say what happens to `frames_soa`, the secondary indexes, or join. Those are paths this crate already ships: drain, `join-drained`, `compact-join`, reindex, and the optional cook. I will not adopt a container whose interaction with them is unspecified.

The scaling pair compares segment files with a single-writer LMDB. At one rank the single-writer figure is the higher of the two (14.6k against 6.7k). The drop to 3.5k at eight ranks is that one writer. Shards are already directories of full LMDB environments, one writer per shard (`traj_id % n_shards`). A final read-only index that is safe on Lustre or GPFS does not replace that layout. C would have to show a measured failure of the sharded envs we have. It does not. I will not add segment recovery to outrun a writer the shard path exists to avoid.

## What I refuse

- A second codec. `pco` 1.0.4 stays out, and so do zfp, blosc2, and fpzip. The prototype's extra native libraries are not a menu.
- Chunked SoA. 9,424,896 bytes at 608 µs loses to raw f64 at 12,476,416 bytes and 1.11 µs for the point read a select actually does.
- A dictionary, required or optional. A's held-out saving is under 2% and the file grew by 8,192 bytes. B's larger saving was trained in-sample. `join-drained` must not learn how to recompress.
- Segment files and locators. 0.495× against 0.60×, and 14.6k → 3.5k on a single writer, do not justify leaving the one-get env.
- Numeric authority. Legacy text rebuilt 0/7 files. Hash, dedup, reindex, and export stay on the ingested UTF-8.
- Level 9 or level 19 as the writer default. On A's file level 9 is the worst of the three on size, compress rate, and decompress rate.
- Hashing compressed bytes. Identical geometries would stop matching after a recompress, and the mismatch would be silent.
- A system `libzstd` or `libpco` that Meson, CMake, wrapdb, or fpm has to find. Those packages link `readcon_db` today and ship no compressor.

## Conditions

- The writer emits zstd level 1. On this corpus that is the smallest payload, the fastest compress, and the same LMDB occupancy as level 3 (17,383,424 bytes, 0.488×). The frame header carries the level, so a later campaign can raise it without a migration. Document level 1 beside the writer.
- The new value is `0xFD 0x01` plus one stock zstd frame. `get_frame_text` returns a still-UTF-8 value unchanged and decompresses the tagged value. Any other shape is a corrupt value, not a CON parse.
- xxh3-128 is computed on the decompressed UTF-8 before the put. Round-trip tests assert the hash matches the legacy hash of that text. `reindex` and `touch_trajectory_blobs` decompress before they hash or checksum.
- The encoder and decoder live inside the existing `readcon_db` staticlib and cdylib, stock zstd frames, writer and reader allowed to be different implementations of that frame. Fortran keeps `link = ["readcon_db"]`. Meson and CMake grow no compressor dependency of their own.
- No dictionary bytes in the env. `join-drained` and `compact-join` copy the stored values. `INPUT_MAP_FACTOR` remains 3× the raw CON.
- `frames_soa` stays uncompressed RCSO. Skipping `recook_all` is the response to a file that would otherwise go from 1.181× to 2.036× text. A pcodec cook needs its own proposal and a measurement on a campaign `data.mdb`.
- If the tagged frame is longer than the UTF-8, store the legacy text.
- A Rust port is accepted on a fresh measurement of this corpus. This ballot does not claim a cold-read win on Lustre or GPFS.
