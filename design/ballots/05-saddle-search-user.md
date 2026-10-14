# Ballot: saddle-search campaign user

## Vote

`Vote: C`

## How I actually read

A run is many short bands, endpoints copied from one band onto the next, cells and atom counts mixed, plus some thermal MD kept in the same directory. I select on fmax, energy, and formula. The select gives me keys. I then point-read those images. The keys are scattered.

The timed get that matches that follow-up read is one frame. On the mixed LJ file a warm zstd get is 53.1 µs at level 1 and 49.7 µs at level 3, against 2.51 µs for the plain text. Fifty microseconds disappears beside a force call. The timed get that misses the select is the 32-frame XOR chunk at 608 µs, because one scattered key inflates the whole chunk. On the argon band, XOR against the previous image reaches 36,004 bytes and the median walk from image 0 is 35.9 µs. The select does not hand me the neighbour.

The vacancy-hop file has 619 distinct frames, so the hash removed nothing there. The same writeup says a real campaign repeats endpoints across bands. I need the later comparison to see the same CON, forces included. I have no measured byte count for what those repeated endpoints would save. A still stores the value under every key and only records the first key in `frame_by_hash`.

## Design A

This is the file that looks like the job: 600-frame LJ MD and a 19-image vacancy hop, endpoints relaxed to 8.0×10⁻⁷ eV/Å, barrier 0.860 eV, saddle at image 9. Level 1 takes the MD text from 35,297,084 bytes to 15,129,210 (2.333×) and the NEB text from 314,542 to 95,807 (3.283×). The frames table goes from 37,285,888 to 17,383,424, 0.488× the raw CON. Level 1 and level 3 occupy the same LMDB high-water mark. The 32 KiB dictionary makes the env 8,192 bytes larger (17,391,616) and the get goes from 49.7 µs to 65.9 µs, after a held-out saving of 1.78% on MD and 1.73% on the NEB. One codec covers the short band and the MD. Mixed atom counts stay one value per key. Hashing the decompressed UTF-8 is the right identity for a shared endpoint.

The frames-table ratio is the frames table. On campaign files the full `data.mdb` today is 1.24× the raw CON, and this study did not rebuild the indexes, so I do not have a new full-env size. Raw `frames_soa` stays in the design because a raw f64 get is 1.11 µs and per-frame shuffle makes the LMDB file larger (12,509,184 against 12,476,416). That microsecond is not the quota. There is no measured ingest rate as ranks are added. The shard writer is described. The campaign number is absent.

## Design B

The quota number I recognize is the recook. After ingest, `data.mdb` is 37,314,560 bytes, 1.181× the 31,584,938-byte CON text. After `recook_all` it is 64,303,104, 2.036× text. The SoA values are 20,322,144 bytes. pcodec per frame cuts the numeric payload to 7,434,922 bytes, 0.367× the 20,269,632 raw f64, same bits, 8.9 µs, one get. On the 11-image band, pcodec is 38,843 bytes against 76,560 raw (0.507×), 8.1 µs.

I select on fmax. That band ran the full 250 FIRE steps and stopped at nudged fmax 0.524 eV/Å. The MD beside it is 2188 frames of proton water, consecutive RMSD median 0.387 Å. zstd-3 on that text is 14,217,637 bytes, 0.450×, and decodes in 17.4 µs, next to a 4.0 µs plain `get_frame_text`. Inside a value-only LMDB the same zstd-3 text is 18,010,112 bytes, 0.570× the raw CON, so the pages stay. Legacy spans rebuild from floats on 0 of 7 files. The ingested CON is the geometry a later comparison has to see. There is no rank sweep. The dictionary was trained on every 20th frame of this same water run; my bands change cell and atom count.

## Design C

This is the only design that was timed the way the script is launched. At 1 rank, single-writer LMDB does 14.6k frames/s and the segment writer does 6.7k. At 8 ranks the segment writer does 19.9k and single-writer LMDB does 3.5k. Eight ranks are about 3× the 1-rank segment rate. The segment rate goes up. Segments are 0.495× raw; the same payload in LMDB at 4 KiB pages is 0.60×. The index holds segment, offset, length, and xxh3. A read-only index is safe on Lustre and GPFS, which is where the campaign directory sits. A point read is one locator and one range. That is the shape of a select that returns scattered images.

## What I refuse

I refuse an ingest that falls from 14.6k frames/s at 1 rank to 3.5k at 8. Adding ranks has to shorten the wall, and that curve lengthens it.

I refuse a raw cooked-force table that takes the file from 1.181× text to 2.036×. The forces I compare are already in the CON.

I refuse a 32-frame chunk at 608 µs, and a neighbour walk at 35.9 µs, as the fetch behind a scattered select.

I refuse a dictionary. On the mixed LJ file the env grew. On the water file the training set was the same trajectory.

I refuse regenerating the CON from floats. Zero of seven legacy files matched, and a respelling is a different geometry.

I refuse treating 619 distinct frames as proof that repeated endpoints are free on disk. Dedup removed nothing on that file, and the value is still stored under every key.

## Conditions

xxh3 is taken on the decompressed CON bytes, forces included, the same UTF-8 the next band will hash. A hash of the zstd payload would make one endpoint look like two geometries after a recompress.

The campaign store is the zstd-3 segment files plus that index. Raw RCSO is not written beside them.

A point read stays one index lookup and one range. A's 49.7 µs and B's 17.4 µs are both fine next to a force call. I have no decode time for C.

A 1-rank smoke run can keep the single-writer path, where 14.6k beats 6.7k. A multi-rank campaign uses the segment writer. The measured gain at 8 ranks is about 3×, and that is the gain I am voting for.

If a numeric side table shows up later, it is one blob per frame, bit-exact, and still not the authority. pcodec at 0.367× the raw f64 and 8.9 µs is the size class I would allow. Comparisons keep reading the CON.
