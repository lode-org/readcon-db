# Ballot: H5MD format steward

## Vote

`Vote: A`

## Authority boundary

Two public formats, and only those.

The corpus authority is the CON text. xxh3-128 names those UTF-8 bytes. Dedup, reindex, join, and export read that span. A value encoding is allowed only as a private way to store that same span: decompress, and the bytes match the hash input.

H5MD is the interchange export of one trajectory. `export_h5md` / `collect_h5md` collect cooked arrays from CON (RCSO is only a cache the collector may read) and the Python binding writes the HDF5 file. The shape is one particles group, a time-dependent position dataset, rectangular `[T][N][3]`, fixed atom count, fixed box and boundary, fixed species Z. Engine units on the way out are Å, ps, and kJ mol^{-1} Angstrom^{-1}; velocity is Angstrom ps-1, with the D10 and `units.time` rules already in the README. Compression of that dataset belongs to HDF5 filters at export time.

A mixed saddle-search campaign (different N, species, cells) is many H5MD files or a ragged tree. It is not one dataset, and it is not a new container beside CON and H5MD.

An internal codec may wrap the CON bytes inside the existing LMDB record. It has no file magic that another tool implements, no versioned layout, and no reader except the path that returns the CON text. Numeric caches (`frames_soa`, RCSO, anything that replaces RCSO) stay non-authoritative: no symbols, no cell, no JSON, no exact text, and not the xxh3 input.

## Design A

A draws the boundary this crate already ships. H5MD is the wrong campaign container and the right interchange: one simulation, or a handful of particle groups of one shape; a campaign of unrelated cells is many files or a ragged tree; a random frame is a 12-byte key, not an HDF5 chunk; parallel HDF5 is a different ingest protocol from the sharded writers. Chunked numeric SoA is an export problem. H5MD already covers one trajectory and rectangular arrays.

The stored authority stays the CON text. Each `frames` value is a zstd frame of that UTF-8 (tag `0xFD 0x01`, lead byte illegal as UTF-8, so a legacy raw value is still the text). On this corpus the frames table is 17,383,424 B, 0.488× the 35,611,626 B of CON text. Plain text in LMDB was 37,285,888 B, 1.047×. The level-3 payload plus tag is 15,364,807 B. Level 1 and level 3 occupy the same 17,383,424 B. A warm get is 49.7 µs at level 3 and 53.1 µs at level 1, against 2.51 µs for the plain text. The returned bytes are the CON either way. The hash is taken on the decompressed UTF-8, so two levels dedup together. A checksum of the compressed bytes would stop tracking the CON.

The dictionary stays out. Held-out frames saved 1.78% (MD, 568 frames, 14,458,979 → 14,201,236) and 1.73% (NEB, 12 frames, 61,682 → 60,616). The LMDB file was 8,192 B larger (17,391,616 against 17,383,424) and the get moved from 49.7 µs to 65.9 µs. A dictionary is a corpus-global object every shard would have to carry. It does not belong in the format.

`frames_soa` stays raw RCSO. The numeric file is 12,476,416 B and a get is 1.11 µs. Byte-shuffle plus zstd on each frame makes that file larger (12,509,184 B) and the get 20.4 µs. A 32-frame XOR chunk is 9,424,896 B and 608 µs per random frame. That size is the sequential-export tradeoff, and it is the H5MD case, not the key layout. Promoting SoA to authority would drop symbols, cell, JSON metadata, and the exact bytes xxh3 is defined on. Raw SoA here is 11,156,784 B against a 15.2 MB zstd CON payload; deleting the CON would be a smaller file and a different database.

This encoding is not a second container. It lives under the existing 12-byte frame key. Readers return text. The zstd level rides inside the frame; readers do not branch on it.

## Design B

B keeps the authority on the ingested span, and the legacy measurement is the one that settles it. Regenerating text from f64s matched 0 of 7 legacy files (8 frames): spelling, whitespace, and JSON key order. Those files are the bytes xxh3 would have stored. Writer-owned frames were bit-exact (2188 MD frames and 11 NEB images; 0 position bits, 0 force bits, 0 energy bits changed). A binary record plus the builder can reprint a frame this writer just emitted. It cannot reprint a span already in the database. Dedup, reindex, and export stay on the stored text.

The text tier matches A on a different corpus. zstd level 3 of the CON is 14,217,637 B, 0.450× of 31,584,938 B, 17.4 µs median decode in memory. Value-only zstd-3 LMDB is 18,010,112 B, 0.570× raw CON. `data.mdb` after ingest is 37,314,560 B, 1.181× text, and 64,303,104 B, 2.036× text, after raw `recook_all`. The SoA table is what doubles the file (sum of values 20,322,144 B).

pcodec as a private replacement for RCSO is a numeric cache with a measured size: 7,434,922 / 20,269,632 = 0.367× raw f64, bit-exact, one get, 8.9 µs median. It is still not the trajectory. The blob has no species, cell, JSON, or units, and it is not the xxh3 input. pcodec's chunk format is specified outside this crate. A value another tool already knows how to open is how a cache becomes a container. The time-major whole-series blob is that container in one object: 6,149,738 B, 0.303× raw f64, and a random frame decodes the whole series (about 20 ms; 9.2 µs only if the cost is amortized across 2188 frames). That is a sequential encoding of one series. H5MD's time-dependent dataset is the public form of that idea, with the rectangular contract and the units. The NEB band does not rescue the series layout: per-image pcodec is 38,843 B (0.507× of 76,560 raw f64, 8.1 µs) and time-major pcodec of the whole band is larger, 40,180 B (0.525×).

B's dictionary (12,742,318 B, 0.403×, 32,768 B dictionary included) was trained on every 20th frame of the same trajectory. B marks that comparison optimistic. It is not a license to ship a shared dictionary. Reprinting writer-owned frames is a CON round-trip for frames this writer emitted. It is not an interchange path, and it is not H5MD.

## Design C

A segment file becomes a shadow container. Each MPI rank appends zstd-3 frames to its own segment files, and LMDB keeps only segment, offset, length, and xxh3. The durable bytes are then a framed file: zstd members, located by offset and length, named by a hash. The read-only index is what makes that safe on Lustre/GPFS, and it is also what makes the segments the object operators copy. A reader that understands the locator and zstd never opens a CON parser and never writes H5MD. Rank segments are not one-simulation files. A mixed campaign in those files is a sequence of unrelated N, species, and cells — the pile H5MD will not store as one dataset. The size is real: 0.495× raw against 0.60× in LMDB, the difference attributed to 4 KiB page slack. Ingest is 6.7k frames/s at 1 rank and 19.9k at 8, against single-writer LMDB at 14.6k falling to 3.5k. Those numbers justify an internal extent. They do not justify a file format with a name, a version, or a second reader.

xxh3 in that index names CON bytes only if the hash input is the decompressed UTF-8. The C summary lists xxh3 beside segment, offset, and length and does not say which bytes were hashed. The hash that is allowed in the index is xxh3-128 of the exact CON text, the same input `hash_frame_bytes` uses, checked again when the extent is read back. A hash of the zstd frame names the encoder and the level. The segment bytes are the storage. The CON bytes are the identity.

Export of one trajectory from segments is a decompress, then the existing collector. Resolve that trajectory's keys, read each extent, inflate to CON text, and call `collect_h5md` / `export_h5md`. One `traj_id`. Fixed natoms, fixed box and boundary, fixed species Z. Positions `[T][N][3]`, edges `[T][3][3]`, times in ps, length in Angstrom, force in kJ mol^{-1} Angstrom^{-1}, velocity in Angstrom ps-1. Frames that fail the fixed-shape checks stay out of that file. A campaign is many H5MD files or a ragged tree. Nothing concatenates a rank segment into one HDF5 dataset. The zstd-3 on the segment is the text codec. Shuffle, gzip, and szip, if used, are HDF5 filters on the position dataset at export, applied to the rectangular arrays after the CON has been parsed.

## What I refuse

- A named segment format: magic, version, documented record layout, or any reader outside this crate's get path. The 0.495× figure and the 6.7k-to-19.9k frames/s scaling do not make those files public.
- xxh3 of compressed bytes, in LMDB or in a segment index.
- pcodec, a time-major pcodec series (0.303× raw f64, about 20 ms for one frame), a keyframe XOR chain, or an NEB chord as a trajectory container. Per-frame numeric blobs stay a non-authoritative cache or they do not ship.
- Promoting `frames_soa` or any f64 reprint to authority. The 0-of-7 legacy result is the reason.
- A zstd dictionary as a required corpus object.
- Chunked SoA as the on-disk key layout (9,424,896 B and 608 µs against raw f64 at 12,476,416 B and 1.11 µs). That shape is an H5MD export.
- H5MD as the campaign store, one dataset for mixed N, or a second HDF5 writer that skips the fixed-shape and unit checks in `export_h5md`.
- Regeneration from f64s as interchange for spans already stored.

## Conditions

A ships with the codec sealed inside the `frames` value. `0xFD 0x01` plus one zstd frame is an implementation detail of that value. No spec, no separate file, no tool that parses `data.mdb` values except by decompressing to CON. A snapshot parser that expects UTF-8 will see the illegal lead byte; the supported read is `get_frame_text`.

xxh3-128, `frame_by_hash`, reindex, dedup, export, and `touch_trajectory_blobs` all run on the decompressed UTF-8. Tests that round-trip zstd assert the hash is unchanged. A truncated frame fails at decompress.

`frames_soa` stays optional, uncompressed RCSO, and non-authoritative. The 32-frame XOR chunk does not become the store. H5MD export stays one trajectory, rectangular `[T][N][3]`, units as documented. HDF5 filters, if any, are chosen at export and are independent of the zstd tag.

The dictionary does not ship. Held-out savings of 1.78% and 1.73% with a file 8,192 B larger are the measurement that closes it.

Ingest level is a writer knob inside the zstd frame (A measured level 1 and level 3 at the same 17,383,424 B). Readers accept any level. Level 9 is not an archival mode: on this file it was larger and slower.

The 0-of-7 legacy mismatch is a regression test on this design. No path treats parsed f64s as able to recreate stored CON bytes.

A later Lustre layout may use untyped extents for the same zstd-CON values. Those extents stay unnamed and unversioned, the index hash stays the CON bytes, and H5MD export stays decompress-then-`collect_h5md`. That layout is a different ballot. It is not part of this vote.
