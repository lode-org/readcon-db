# Ballot: numerical-precision reviewer

## Vote

`Vote: A`

A stores the CON UTF-8 that `hash_frame_bytes` already hashes, and it stores numeric values as a raw little-endian f64 copy. B's measured pcodec path is also bit-exact and B does not drop that requirement, so this is not a hybrid. C never names the hashed bytes or the numeric tier.

## What must stay bit-exact

The xxh3-128 input is the ingested CON UTF-8. `hash_frame_bytes` runs `xxh3_128` on the slice it is given and stores the digest little-endian. That slice is the decompressed text, the same bytes a get returns, including every binary64 digit, tab, and JSON key order.

When a numeric tier exists, each f64 is the little-endian bits (`to_le_bytes` / `to_bits`). RCSO stays non-authoritative. `cooked_soa.rs` states it is not fully equivalent: positions and optional forces and velocities only, with no symbols, masses, cell, constraint masks, JSON metadata, section labels, or exact on-disk CON bytes, and it is not the input to xxh3, reindex, or export.

A residual, if one is ever stored, is XOR of `to_bits()` or wrapping `u64` subtraction of those bits. IEEE subtraction is not a residual.

## Design A

Hash input. xxh3-128 stays on the decompressed UTF-8, inside `hash_frame_bytes`, before the put. Legacy raw, level 1, and a later level of one text dedup together. `reindex` and `touch_trajectory_blobs` decompress before they hash or checksum. A checksum of the compressed bytes would stop tracking the CON. Hashing the compressed bytes makes dedup follow the level and the encoder, and identical geometries stop matching after a recompress.

Prefix. A 0.1.8 value that is valid UTF-8 is returned as-is. A new value starts with `0xFD 0x01` (the lead byte is illegal UTF-8, so it cannot be a legacy frame) and the rest is one zstd frame of that UTF-8. Anything else is a corrupt value.

Tiny-frame guard. The writer compares compressed length with the UTF-8 length and, when zstd does not shrink the frame, writes the legacy UTF-8 form. Those stored bytes are the original span. The four-atom probe shrank from 1,408 B to 425 B, so ordinary frames take the zstd path. The guard does not reprint the text.

Raw SoA. `frames_soa` stays uncompressed RCSO, a bit-exact copy of little-endian f64. The warm get is 1.11 µs. The raw payload is 11,156,784 B and the LMDB file is 12,476,416 B.

Where A is safe. Decompress must return the original UTF-8, and the harness checks that. Level 1's tagged payload is 15,225,636 B and level 3's is 15,364,807 B. Both occupy the same 17,383,424 B LMDB, 0.488× of 35,611,626 B raw CON. A warm zstd get is 49.7 µs at level 3 and 53.1 µs at level 1, against 2.51 µs for the plain text. The level changes size and latency. It does not change the bits the hash sees. A refuses to promote SoA to authority, because that would drop symbols, cell, JSON metadata, and the exact bytes xxh3 is defined on. Raw SoA on this corpus is 11.2 MB beside a 15.2 MB zstd CON; deleting the CON would be a smaller file and a different database.

Where A refuses a bit-exact win. Shuffle plus zstd, and XOR chunks, were lossless in the harness. Per-frame byte-shuffle plus zstd made the numeric LMDB larger (12,509,184 B against 12,476,416 B) and the get slower (20.4 µs against 1.11 µs). A 32-frame XOR chunk is the smallest numeric file in A's set, 9,424,896 B against 12,476,416 B, and a random get rises to 608 µs. A rejects both for size and latency, not because a bit moved. I accept the refusal. The stored numeric bytes remain the f64 bits. I will not spend those bits to buy the 9,424,896 B file.

## Design B

The 18% subtraction result. On the packed MD position and force series, `(a - b) + b` differed in bits from `a` for 454,676 of 2,532,546 consecutive pairs (18.0%). The reversible forms B names are XOR of `to_bits()` and wrapping `u64` subtraction of those bits. That split is the right one. An IEEE difference is not a stored residual.

Legacy mismatch. Writer-owned frames (`FloatFormat::RoundTrip`, `{value:e}`) rebuilt bit-exact: 2188 MD frames and 11 NEB images. Parse-then-rewrite, rebuild from the f64s that were written, and rebuild from the f64s the parser returned all matched. Position bits changed for 0 / 1,266,852 (MD) and 0 / 4,785 (NEB). Force bits changed for the same counts. Energies changed for 0 frames. The same writer matched none of 7 legacy files in `resources/test` (8 frames). The first differences are spelling (`20.000000` against `2e1`), tabs against spaces, and JSON key order (`sections` and `energy` move). Those file bytes are what xxh3 stores. A reprint from coordinates is a different blob, a different hash, and a different export.

pcodec versus XOR, zfp, and fpzip. Every codec row B reports decoded to the same f64 bits, or the same text bytes, for every frame. pcodec per frame is the bit-exact numeric outlier: 7,434,922 B = 0.367× of 20,269,632 raw f64, median decode 8.9 µs, one get. XOR of the previous frame and then pcodec expands to 17,638,938 B (17.64 MB); the XOR pattern is a worse pcodec input than the floats. zfp reversible per frame is 25,869,552 B, 1.276× the raw f64, larger than the uncompressed array. fpzip lossless per frame is 19,102,320 B, 0.942× raw. Byte shuffle, blosc2, and plain zstd of the raw f64s land in the same band and do not beat pcodec. None of those rows is a precision miss. The rows that grow are still the same bits, in more bytes. B keeps CON text as the authority and would replace RCSO with one pcodec blob per frame, still non-authoritative. On this vote the numeric tier stays A's raw copy. pcodec's zero bit-diff is real, and it is a size result on a derived table, not a new identity for the frame.

Text tier on B's corpus. zstd level 3 is 14,217,637 B = 0.450× of 31,584,938 B, median decode 17.4 µs. Hash the bytes after decompression. That is the same invariant as A, measured on a different file.

After `recook_all`, `data.mdb` is 64,303,104 B, 2.036× the text, because the sum of `frames_soa` values is 20,322,144 B (20,269,632 raw f64 plus a 24-byte header on each of 2188 frames). The file roughly doubles because the bits are stored twice. The second copy is not authority.

The temptation. Zero bit changes on 2188 MD frames and 11 NEB images will be read as permission to regenerate the stored text from the floats. The 7 legacy files are the counterexample, and the "not fully equivalent" comment in `cooked_soa.rs` is the same fact: the numeric blob cannot feed xxh3 or CON export. B already leaves regeneration as an export of writer-owned frames. That limit stays.

The NEB chord stores the XOR of position bits against `(1-t)*R0 + t*RN` and treats the result as lossless because both sides evaluate the same expression. That expression is IEEE arithmetic. It is not XOR of `to_bits()` and it is not wrapping `u64` subtraction. Matching bits require matching contraction on both sides. I do not count it as a residual.

## Design C

C keeps LMDB as the index only. Each MPI rank appends zstd level 3 frames to its own segment files. The index holds the segment id, offset, length, and xxh3. A final read-only index is the Lustre/GPFS claim. Measured size is 0.495× raw against 0.60× in LMDB, the gap attributed to 4 KiB page slack. Ingest scaled from 6.7k frames/s at 1 rank to 19.9k at 8, against single-writer LMDB falling from 14.6k to 3.5k. The rates are not a statement about bits.

xxh3 in the index, if the segment bytes are compressed. `hash_frame_bytes` hashes the slice it is handed. If that slice is the zstd frame, the identity follows the encoder and the level. A recompress of the same CON UTF-8 changes the hash, which is the silent dedup failure A already names. The indexed xxh3 has to be the digest of the decompressed UTF-8, and a read has to check those decompressed bytes. The indexed length is then the compressed length, which is not the length of the hashed span. C does not say which bytes were hashed. Until it does, the index hash is not the CON identity.

Numerics. C does not say whether the zstd payload is the CON text or a numeric array, and it does not mention `frames_soa`. Silence is text tier only, numeric tier unspecified. An unspecified numeric tier is where an IEEE residual or a lossy codec would sit. I will not fill the gap with raw RCSO, pcodec, or XOR chunks. Unspecified is not bit-exact.

How 0.495× interacts with a second numeric blob. 0.495× is the segment-file ratio of the text tier, set next to 0.60× for the same bytes inside LMDB pages. It does not include a numeric payload. On B's corpus the raw SoA is about 20.3 MB beside 31,584,938 B of text, and `recook_all` takes `data.mdb` to 2.036× the text. On A's corpus the raw SoA LMDB is 12,476,416 B beside a zstd frames table of 17,383,424 B. pcodec at 7,434,922 B would be another pile of derived bytes, still non-authoritative. Any of those added to C's segments changes 0.495×. A's level 1 and level 3 already land on one 17,383,424 B high-water mark: a payload cut can disappear into 4 KiB pages. That page fact does not make a second blob free, and it does not make an unnamed codec exact.

## What I refuse

- An xxh3-128 taken on compressed segment bytes, on a zstd frame, or on an RCSO or pcodec blob.
- Writing a reprint of CON text from f64s back as the stored authority, including for frames this writer just emitted.
- IEEE `(a - b) + b` as a residual, the chord `(1-t)*R0 + t*RN` as a stored predictor, scaled-integer packing, or any codec whose contract is a tolerance.
- Promoting pcodec, zfp reversible, or fpzip to authority. pcodec's 7,434,922 B round-tripped the f64 bits; zfp reversible was larger than the raw 20,269,632 B. Both facts leave the text in charge.
- Design C while the hashed bytes and the numeric tier are unspecified.
- Spending a bit of an f64, or a byte of the hashed UTF-8, to make a file smaller. A's choice to keep the 12,476,416 B raw SoA instead of the lossless 9,424,896 B XOR chunk is a latency choice. I will not reopen it by relaxing exactness.

## Conditions

- The frames value is `0xFD 0x01` plus one zstd frame of the ingested CON UTF-8, or that UTF-8 itself when zstd does not shrink. Decompress equals the ingested span before `hash_frame_bytes`.
- `frames_soa` stays raw little-endian f64 RCSO, non-authoritative, as specified in `cooked_soa.rs`.
- A later numeric codec is precision-neutral only if it shows zero f64 bit diffs the way B did (0 / 1,266,852 positions, 0 / 1,266,852 forces, 0 / 4,785 and 0 / 4,785, 0 energies) and stays off the xxh3 path. That bar is how pcodec's 7,434,922 B and 8.9 µs would be judged. It is not part of this vote. The chord does not meet the bar.
- The 7 legacy files (8 frames) in `resources/test` stay byte-identical. The writer-owned round trip (2188 MD frames, 11 NEB images, 0 bit changes) is an export property of `{value:e}` frames, not the storage identity.
- C would need three statements this text does not have: each segment frame is zstd of the CON UTF-8, the indexed xxh3-128 is the hash of those decompressed bytes, and any numeric tier is raw little-endian f64 or a codec with a measured zero bit-diff. The 0.495× figure would be restated with that second blob included. Those statements are absent, so C is not the vote.
