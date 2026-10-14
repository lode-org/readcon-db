# Storage decision

Ship design A. The `frames` value stays the ingested CON text, stored as one zstd frame inside the existing sharded LMDB. `frames_soa` stays optional raw RCSO. H5MD stays the interchange export. pcodec does not enter the library. Per-rank segment files do not replace the container.

Design C had no branch. This branch is `design/storage-a` merged with `design/storage-b`. C was reviewed from the panel charge only.

## Vote tally

| Ballot | Vote |
|---|---|
| HPC filesystem (`design/ballots/01-hpc-filesystem.md`) | hybrid sharded segments |
| numerical-precision (`design/ballots/02-numerical-precision.md`) | A |
| H5MD format steward (`design/ballots/03-h5md-steward.md`) | A |
| maintainer, complexity (`design/ballots/04-maintainer-complexity.md`) | A |
| saddle-search campaign user (`design/ballots/05-saddle-search-user.md`) | C |

A 3, C 1, hybrid sharded segments 1, B 0.

The three A ballots agree on the value: exact CON UTF-8, xxh3-128 after decompression, no dictionary, raw non-authoritative SoA, one new link dependency (zstd) inside `readcon_db`. The two dissents are about the container on a shared filesystem. The HPC ballot wants C's append-only segments folded back into the existing shard tree (`traj_id % n_shards`, drain to O(n_shards) files). The campaign ballot wants C as voted, because that is the only rank sweep in the charge. Neither dissent votes for pcodec. The campaign ballot allows a later per-frame bit-exact numeric cache and still names the CON as the geometry that comparisons read.

A hybrid of C's segments and B's pcodec was on the table. It collected no vote. pcodec is a measured size win on a derived table (7,434,922 B, 0.367× of 20,269,632 raw f64, 8.9 µs, bit-exact) and it lost as a storage change: the precision ballot keeps the raw f64 copy, the format ballot treats a pcodec blob as a second container, the maintainer ballot refuses a second codec in every binding, and the filesystem ballot leaves numeric bytes out of the file readers open.

## What ships

`frames` becomes `Database<Bytes, Bytes>`. A value that is valid UTF-8 is a legacy CON span and is returned unchanged. A new value is the two bytes `0xFD 0x01` followed by one stock zstd frame of that same UTF-8. Any other shape is corrupt at decompress, before the CON parser runs. The writer compares lengths and stores the legacy UTF-8 when zstd does not shrink the frame. The four-atom probe in A shrank from 1,408 B to 425 B, so ordinary frames take the zstd path.

The writer emits level 1. On A's mixed corpus (35,611,626 B of CON text, 619 frames) the tagged level-1 payload is 15,225,636 B and the tagged level-3 payload is 15,364,807 B, and both occupy the same 17,383,424 B LMDB high-water mark, 0.488× the raw CON. Plain text occupies 37,285,888 B, 1.047×. Level 1 compresses at 367 MB/s against 209 MB/s for level 3 and 46 MB/s for level 9. Level 9's in-memory payload is larger (15,694,514 B). The zstd frame header carries the level. Readers accept any level. MD text shrinks 2.333× (35,297,084 → 15,129,210) and the 19-image NEB shrinks 3.283× (314,542 → 95,807). One codec covers both shapes.

xxh3-128 stays on the decompressed UTF-8, in `hash_frame_bytes`, before the put. `frame_by_hash`, `reindex`, dedup, export, and `touch_trajectory_blobs` see those bytes. A legacy span, a level-1 frame, and a later level of the same text dedup together. Compression runs in the existing prepare step, outside the write transaction. The MPI hash allgather stays 16 plaintext bytes. `Alltoallv` of a first occurrence carries the tagged frame. `INPUT_MAP_FACTOR` stays 3× the raw CON, because the secondary indexes are unchanged. No dictionary is stored. `join-drained` and `compact-join` copy values. A shard snapshot still opens alone.

`frames_soa` stays the RCSO layout in `src/cooked_soa.rs`: little-endian f64, non-authoritative, absent from xxh3, reindex, join, and export. On A's corpus the raw numeric LMDB is 12,476,416 B with a 1.11 µs warm get. Per-frame shuffle plus zstd made that file larger (12,509,184 B) and the get 20.4 µs. A 32-frame XOR chunk reached 9,424,896 B and 608 µs, which is the wrong fetch for a select that returns scattered keys. On B's corpus `recook_all` grew `data.mdb` from 37,314,560 B (1.181× the 31,584,938 B text) to 64,303,104 B (2.036×). The response to that growth is to skip the cook. The forces a saddle comparison needs are already in the CON.

H5MD is unchanged. `export_h5md` / `collect_h5md` still collect one trajectory, rectangular `[T][N][3]`, with the units already documented. HDF5 filters, when used, are chosen at export. A mixed campaign remains many H5MD files. The zstd tag is an implementation detail of the LMDB value. It has no separate magic, version, or reader.

## Work items, in order

1. Link zstd inside the existing `readcon_db` staticlib and cdylib, stock frames, encoder and decoder allowed to differ. Meson, CMake, the wrapdb package, and `fortran/ReadConDb` keep linking `readcon_db` and do not grow their own compressor discovery.
2. Open `frames` as bytes. Write `0xFD 0x01` plus a level-1 zstd frame from the prepare step. Keep the legacy UTF-8 value when the tagged frame is longer than the text. Document level 1 beside the writer so a later change does not treat level 9 as an archival upgrade.
3. Decompress before hash, reindex, checksum, dedup, and export. A truncated or bit-flipped value fails at decompress.
4. Tests on the bytes this crate already stores. The seven legacy files under `resources/test` (eight frames) stay byte-identical: B measured a writer-owned reprint matching 0 of those 7, so a rebuild from f64s is not the identity. A round trip through zstd asserts the xxh3-128 of the decompressed text equals the hash of the original span. A value that is still valid UTF-8 is returned unchanged. A synthetic frame that zstd does not shrink takes the legacy path. A short value is an error, not a CON parse.
5. Leave `frames_soa` uncompressed. State in the cook path that skipping `recook_all` is how a campaign avoids the 1.181× → 2.036× growth. Do not add `pco`, zfp, blosc2, or fpzip.
6. Re-measure A's corpus from Rust on the same 619-frame file before calling the port done. The Python gap (plain get 2.51 µs, level 1 53.1 µs, level 3 49.7 µs) shares the interpreter; the size ratios are the ones that have to carry.
7. Run the container gate below before any segment or locator code. The codec change and a new file format are separate changes.

## Risks

The new dependency is one compressor, and it lands in every binding because they all link `readcon_db`. A system `libzstd` that Meson, CMake, wrapdb, or fpm has to find recreates that cost outside the crate.

Old and new environments sit side by side during a campaign. A reader that still asks Heed for `Str` rejects a tagged value. A tool that snapshots `data.mdb` and parses values as UTF-8 sees the illegal lead byte. The supported read is `get_frame_text`.

Hashing the compressed bytes is a silent dedup failure. The same geometry stops matching after a recompress, because the identity followed the level and the encoder.

`touch_trajectory_blobs` exists to make a campaign pay for payload bytes. A checksum of the compressed form hides a decompress regression.

Level 9 looks like the archival setting and is the worst of A's three levels on size, compress rate, and decompress rate. B's level 19 per frame (12,506,021 B, 0.396× of 31,584,938) and whole-file level 19 (9,589,201 B, 0.304×) give up the point get or buy a payload cut the 4 KiB page may erase. A's dictionary saved 1.78% and 1.73% on held-out frames and the env grew by 8,192 B (17,391,616 against 17,383,424), with the get moving from 49.7 µs to 65.9 µs. B's dictionary (14,217,637 → 12,742,318 including 32,768 B) was trained on every 20th frame of the same trajectory, and on the 11-image band it made the payload larger (64,258 → 64,281).

Page slack is real and it is not, by itself, a reason to leave LMDB. A's tagged level-3 payload is 0.431× raw and the file is 0.488×. B's value-only zstd-3 text is 18,010,112 B, 0.570× of 31,584,938, against an in-memory payload of 0.450×. C reports 0.495× in segment files against 0.60× in LMDB. Those three gaps are the same 4 KiB rounding on three corpora. A's 0.488× already includes the slack and is the occupancy this decision accepts.

C's rank curve is the other real risk, and it is the unsharded env. Segment ingest went from 6.7k frames/s at 1 rank to 19.9k at 8 (2.97×). Single-writer LMDB went from 14.6k to 3.5k. At 1 rank the LMDB rate is higher (14.6 / 6.7). At 8 ranks the segments are 5.7× the single-writer rate. `src/shard.rs` already refuses one shared write transaction: one env per shard, default 64, private trees, drain, join. C did not measure that layout. Quoting 14.6k → 3.5k as the cost of this repository's LMDB overstates the failure the shard tree was built to avoid. Shipping segments before that measurement replaces a working container with an unspecified one: C does not say what happens to secondary indexes, `frames_soa`, or join, and a segment per rank is an inode per rank for the life of the campaign.

A live mmap of `data.mdb` on Lustre or GPFS, while another rank holds the write transaction, is unsafe for a different reason than size. Close-to-open and token revocation apply to the sharded env too. The published object is a `data.mdb` copy with no lock file, opened after the writer has exited, which is the shape `snapshot_to` already writes. `MAX_READERS` is 512 per env. An analysis job pointed at one live shard still stops on the reader table.

IEEE subtraction is not a residual. On B's packed MD series `(a - b) + b` differed for 454,676 of 2,532,546 consecutive pairs (18.0%). A numeric codec that returns later has to store the f64 bits, or an XOR / wrapping subtraction of `to_bits()`. The NEB chord `(1-t)*R0 + t*RN` is IEEE arithmetic on both sides and is not that residual. Writer-owned `{value:e}` frames rebuilt bit-exact (2188 MD frames, 11 NEB images, 0 position bits, 0 force bits, 0 energies). That is an export property of frames this writer emitted. It is not permission to regenerate a stored span.

A's warm-cache times are not a cold Lustre read. The study says `posix_fadvise(DONTNEED)` did not drop the 10–40 MB files. The on-disk drop from 37.3 MB to 17.4 MB for the frames table is measured. A cold-read win is not.

## Benchmark gates

**Codec gate, before the port is called done.** Same 619-frame corpus as A, Rust, Heed.

- Frames-table occupancy at or under 0.50× the raw CON. A measured 0.488× (17,383,424 / 35,611,626) for both level 1 and level 3.
- Decompressed bytes equal the ingested UTF-8 on every frame, including the legacy files.
- xxh3-128 of the decompressed text equals the hash of the original span at level 1 and at level 3.
- A truncated frame fails at decompress.
- Report level 1 and level 3 file size, compress rate, and warm-get median. Ship level 1 unless level 3 shrinks the LMDB file by more than one 4 KiB page. A's two levels tied at 17,383,424 B.
- The warm get may cost tens of microseconds. A measured 53.1 µs at level 1 against 2.51 µs plain in CPython. Fail the Rust port if the median zstd get on this corpus exceeds 200 µs, or if the compressed frames table is larger than the plain one.

**Container gate, before any segment code.** Same rank sweep C published: 1 rank and 8 ranks. Three writers, same frames.

- Control: one unsharded LMDB writer. The published shape is 14.6k frames/s at 1 rank and 3.5k at 8. If this control does not fall, the sweep is not the one C measured.
- The layout this crate already ships: sharded LMDB, one writer per owned shard, node-local tree, drain, join.
- C's segment writer. The published shape is 6.7k at 1 rank and 19.9k at 8.

Stay on sharded LMDB when the sharded 8-rank rate is at least the segment rate (19.9k frames/s), or when the shortfall is inside run-to-run noise. Also compare drain bytes and a cold point read of one frame. C's slack is 0.60× in LMDB against 0.495× in segments. A's frames table is already 0.488×, so a cold-read loss has to show up as bytes moved or as latency, not as a restatement of the 4 KiB gap.

Open a segment follow-on only when the sharded writer is still falling with rank the way the unsharded writer fell, and the cold read or the drain copy loses to segments by at least that slack. The follow-on, if the gate opens, has a fixed shape:

- Each ingest segment is stripe count 1. Extending writes are full-stripe (1 MiB). Per-frame bytes stay on node-local disk until publish.
- Publish order is flush and close the segment, commit the index records, close the index, then let any other rank open it.
- Crash recovery truncates each segment to the last indexed offset and checks xxh3 of the decompressed UTF-8.
- The published index is per shard, `data.mdb` only, no `lock.mdb`, opened read-only.
- Drain folds node-local segments so the durable tree is O(n_shards), not O(ranks).
- The payload is the same tagged zstd CON value. The index hash is the decompressed text. The extents are unnamed and unversioned. H5MD export is decompress, then `collect_h5md`.
- `frames_soa` stays out of that index.

**Numeric-codec gate, closed.** Reopen only as its own proposal, after the codec gate. Required evidence: zero f64 bit differences on a campaign corpus, on the bar B already cleared (0 of 1,266,852 MD positions, 0 of 1,266,852 MD forces, 0 of 4,785 NEB positions, 0 of 4,785 NEB forces, 0 energies), one get, and an on-disk occupancy for the numeric blob. The proposal has to say why skipping `recook_all` is not enough for the 2.036× file. XOR-then-pcodec expanded to 17,638,938 B and is not that proposal. zfp reversible (25,869,552 B, 1.276× raw f64), fpzip, blosc2, a dictionary, and chunked SoA (608 µs) stay out on the numbers already in hand.

## What each ballot changed in this text

The precision ballot fixed the hash input, the corrupt-value rule, the legacy-file test, and the refusal to treat writer-owned round trips as storage identity. The format ballot fixed the boundary: two public formats, CON and H5MD, and no named segment layout even if a later extent store exists. The maintainer ballot fixed the dependency shape (one compressor, inside `readcon_db`, level 1, no `pco`) and the order of work (codec first, container measured against the sharded env we have). The filesystem ballot fixed the container gate: the 14.6k → 3.5k curve is the unsharded env, publish is read-only and lock-free, and a segment per rank does not survive as the campaign directory. The campaign ballot fixed the user-visible bar: a select point-reads scattered images, 50 µs is acceptable next to a force call, 608 µs and a neighbour walk are not, repeated endpoints still store a value per key, and an ingest that slows as ranks are added is the failure a later measurement has to answer.
