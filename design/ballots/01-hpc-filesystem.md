# Ballot: HPC filesystem engineer

## Vote

Vote: hybrid sharded segments

Keep C's split: zstd level 3 frames in append-only segment files, LMDB reduced to segment id, offset, length, and xxh3, published read-only. Keep the existing shard tree (`traj_id % n_shards`, default 64), node-local private trees, and drain/join so the campaign directory stays one index plus a bounded segment set per shard. Drop dictionaries, drop chunked `frames_soa`, and drop B's second numeric object from the file readers open.

Pure C loses on a filesystem fact: one segment inode per MPI rank, for the life of the campaign, is the metadata pattern the 64-way shard tree exists to cap. Pure A and pure B leave the bulk bytes in an mmap'd `data.mdb`.

## Ranking

1. Hybrid sharded segments. Published layout is a small read-only index plus dense, immutable, stripe-count-1 segment files under a fixed shard count. Ingest appends are single-writer. Readers open the index only after the writer has closed it. Cost on Lustre/GPFS: a publish barrier, a drain copy, and one extra inode per shard for the segment file.

2. C. Same read and write shapes, file count equal to the rank count. Eight ranks hid this. A few thousand ranks in one directory is an MDS/GPFS-inode problem on every open, stat, and unlink. The reported ingest curve is 6.7k frames/s at 1 rank and 19.9k at 8 (2.97×, 37% of linear). The index LMDB is still on that path.

3. A. Payload stays in LMDB overflow pages. The frames table shrinks from 37,285,888 B (1.047× the 35,611,626 B of CON) to 17,383,424 B (0.488×) for both zstd-1+tag and zstd-3+tag. That is a real cut in bytes drained and in bytes a cold client would move. It is still 4 KiB pages, a lock file, and a shared mmap. Warm gets (2.51 µs plain, 53.1 µs level 1, 49.7 µs level 3) were measured on a 10–40 MB file that stayed cached after `posix_fadvise(DONTNEED)`. A states that a cold Lustre read was not measured.

4. B. Same LMDB container as A, then a second copy. `data.mdb` is 37,314,560 B (1.181× the 31,584,938 B CON) after ingest and 64,303,104 B (2.036×) after `recook_all`, because `frames_soa` adds 20,322,144 B. pcodec's 7,434,922 B (0.367× the 20,269,632 B of raw f64) and 8.9 µs decode are in-memory, after the bytes are already resident. They do not change the read syscall. The file readers mmap is the 2.036× file.

## Design A

A never leaves LMDB. Every compressed CON value is an overflow span inside `data.mdb`, and every reader of a live env maps that file.

Page slack is measured, and it eats codec differences whole. Tagged level 1 is 15,225,636 B and tagged level 3 is 15,364,807 B. Both envs occupy 17,383,424 B. The 139,171 B payload gap is invisible on disk. The level-3 file sits 2,018,617 B above its tagged payload; the level-1 file sits 2,157,788 B above its tagged payload. Those are 4 KiB overflow pages. A ~25 KB compressed frame occupies a contiguous run of them, so a point read is one span, and that span is still rounded up to the next page. The 32 KiB dictionary cuts the tagged payload to 15,112,412 B and the env grows to 17,391,616 B, exactly 8,192 B (two pages) larger, with the warm get moving from 49.7 µs to 65.9 µs. On this page size the dictionary is a join hazard with a larger file.

`frames_soa` shows the same rounding in the other direction. Raw f64 occupies 12,476,416 B and a warm get is 1.11 µs. Per-frame shuffle+zstd is a smaller payload and a larger file (12,509,184 B) at 20.4 µs. The 32-frame XOR chunk is the one numeric layout that shrinks the file (9,424,896 B). A random get then inflates the whole chunk (608 µs). On a cold filesystem that get reads the chunk, not the frame. MD is 600 of the 619 frames, so the latency average is the point-read case.

Campaign layout around that table is already many files: default 64 shard directories, each with `data.mdb` plus `lock.mdb`. `snapshot_to` copies `data.mdb` only and writes no lock file. That is the right published object. A live env is the wrong one. The crate comment in `corpus.rs` describes one writer and many readers sharing the mmap across OS processes, and it says this is not a cluster protocol. Lustre close-to-open will not show one client's dirty mmap pages to another client until a close or a lock drop. GPFS will hand out a write token and revoke it on every remote reader. `MAX_READERS` is 512 per env, stored in the lock file. An analysis job larger than that, pointed at one shard, stops on the reader table even when the bytes would have fit.

A's full-file production ratio, indexes included, is 1.24× raw CON on campaign files. This study did not rebuild those indexes, so 0.488× is the frames table, not the object drain copies. `INPUT_MAP_FACTOR` stays 3× the raw CON because the index pages are still there. Compression shortens the overflow region. It leaves the B-tree, the meta pages, the lock file, and the random 4 KiB write pattern in place. Level 1 and level 3 matching at 17,383,424 B is the clue: once values live in overflow pages, small payload wins do not move the file, and a cold read of that file was never taken.

## Design B

B's text result is the same LMDB decision on a different corpus. zstd-3 payload is 14,217,637 B, 0.450× the 31,584,938 B CON, with an in-memory median decode of 17.4 µs. Put into a value-only env the file is 18,010,112 B: 1.267× the compressed values and 0.570× the raw CON. Plain text in that same value-only env is 35,934,208 B, 1.138× the CON. Page overhead remains after compression and is a larger fraction of the smaller values (3,792,475 B between the zstd-3 payload and the value-only file). `get_frame_text` at 4.0 µs median is a warm local transaction, in the same class as A's 2.51 µs plain get. Neither number is a Lustre read.

`recook_all` is the filesystem failure. The env grows from 37,314,560 B to 64,303,104 B. The added `frames_soa` values are 20,322,144 B (20,269,632 B of f64 plus a 24-byte header on each of 2188 frames); the file grows by 26,988,544 B. Text readers and numeric readers share one striped object. A rank that only wants the CON still opens a file twice the size, and the client cache and the OST lock cover both copies. Drain then copies the doubled `data.mdb`.

pcodec does not change that I/O. The prototype times decode after the bytes are in memory: 8.9 µs median, p99 12.2 µs, one value, 7,434,922 B. There is no `data.mdb` occupancy for a pcodec value in the document. The numeric layouts that were put in LMDB are shuffle+zstd, at 26,972,160 B (0.854× raw CON, 1.438× the values). A time-major pcodec of the whole series is 6,149,738 B and decompresses the series to read one frame. That is a sequential blob, which is a legal segment, and it is a bad point get. Keyframe XOR chains (30.5 µs at a keyframe every 8, 121 µs every 32) turn one logical read into a walk of several values. On Lustre each step is another pread unless the whole chain is one contiguous extent.

The dictionary line (14,217,637 B down to 12,742,318 B, dictionary included) was trained on every 20th frame of this same trajectory. B calls that number optimistic. It was not measured as an LMDB file. A's held-out dictionary, which was measured as an env, got larger. A per-topology dictionary is also a second object every shard must carry or `join-drained` must recompress.

B's warm text get stays one get, which is the right shape. The bytes of that get still live in the mmap, next to a non-authoritative numeric copy the header of `cooked_soa.rs` already says cannot replace the CON.

## Design C

The 0.495× versus 0.60× claim matches the slack both prototypes already measured, and it is the right reason to take bulk bytes out of LMDB. C's own pair: compressed payload at 0.495× raw, the same payload inside LMDB at 0.60×, gap 0.105× raw, attributed to 4 KiB pages. A's gap on larger frames is in the same direction: tagged level-3 payload 15,364,807 / 35,611,626 = 0.431, file 0.488, difference 0.057 of raw, and the file is 2,018,617 B above the payload. B's value-only gap is 0.570 − 0.450 = 0.120 of raw, 3,792,475 B. C's 0.105 sits between those two corpora. I believe the slack. I do not treat 0.495× as a comparison against A's 0.488×; those are different files. What C adds is a place to put the compressed bytes where the next page boundary does not round each frame up.

The scaling pair does not say what the summary leans on it to say. Segment ingest goes from 6.7k frames/s at 1 rank to 19.9k at 8 ranks, 2.97×. Single-writer LMDB goes from 14.6k to 3.5k over that sweep, a drop to 0.24×, and at 8 ranks the segments are 19.9 / 3.5 = 5.7× the LMDB rate. At 1 rank the LMDB rate is 14.6 / 6.7 = 2.2× the segment rate. That LMDB curve is one env and one write transaction. `shard.rs` already refuses that layout: one env per shard, default 64, route `traj_id % n_shards`, and if several ranks share a shard id each node keeps a private tree, drains to a unique dest, and joins. Eight ranks can own eight shard ids with no shared write txn. C did not measure that configuration. The collapse to 3.5k is the failure mode the current tree was built to avoid. The segment speedup that stops at 2.97× on 8 ranks is the number I will hold C to: something in that path, the index puts or the per-frame size updates, is already not scaling on a job this small.

Segment-per-rank is the correct ingest file and the wrong campaign file. One writer appending to its own file, stripe count 1, no other rank holding the file open, is the Lustre and GPFS pattern that avoids lock revocation. Default wide striping on that file would only add OST locks to a single writer. The published tree is a different budget. A names it: one file per chunk is the metadata pattern the sharded `data.mdb` layout was built to avoid. A segment per rank is that pattern with the count set by the job size. The 8-rank measurement creates eight inodes. The jobs this layout has to survive create thousands, and every later select that opens the set pays MDS or GPFS inode lookups for all of them. Drain into `shard_XXXX/` is how the current code turns a per-node private tree into a bounded campaign tree. Segments need the same fold: append on the node, concatenate or pack under the shard at drain, publish O(n_shards) files.

Crash and partial append are the cost of leaving LMDB's commit. An LMDB write txn publishes a meta page or it does not; a torn overflow page is not a committed value. An append-only segment has a torn tail whenever the writer dies mid-write. Recovery is truncate-to-last-indexed-offset, and the xxh3 stored in the index is what detects a short or flipped span. The order on a shared filesystem is stricter than the order on a local disk. Lustre close-to-open: another client sees the writer's data after the writer closes or drops the lock, not after `write()` returns. GPFS: a reader that opens a file still being appended takes the write token away from the writer on each read. A "final" read-only index is safe only when every segment it names has been flushed and closed first, and no rank appends to those files again. An index updated live, while the segment is still open on the writer, publishes offsets other ranks cannot read coherently.

The index LMDB still touches the filesystem, and it is the part LMDB is fit for. Records of segment id, offset, length, and xxh3 fit in leaf pages. They do not allocate overflow. The remaining object is still a `data.mdb` with 4 KiB pages, a single writer per env, and, if opened normally, a `lock.mdb` with 512 reader slots. Many ranks may open that file after it is immutable: read-only, no lock file, the same shape as `snapshot_to`. They may not open it while a writer on another node is committing. The sublinear 6.7k → 19.9k curve is what an index updated on the ingest path still costs. Build it at publish, per shard, after the segment data is durable.

## What I refuse

- A live mmap of `data.mdb` on Lustre or GPFS while another rank holds the write transaction. Client cache and token revocation will serialize the job, and the bytes a reader faults can be stale under close-to-open.
- Quoting 14.6k → 3.5k frames/s as the cost of this repository's LMDB. That curve is the unsharded env. The code shards specifically so writers do not share one write transaction.
- A campaign directory whose inode count equals the largest job that ever ingested it.
- An index record that becomes visible to another node before the corresponding segment bytes have been flushed and the writer has closed the file.
- A torn tail served as a frame. The readable end of a segment is the last offset the index committed, and a checksum mismatch is an error.
- `recook_all` inside the authoritative env. 37,314,560 B to 64,303,104 B doubles the object every text reader opens and every drain copies.
- A point read that inflates a 32-frame chunk. The file is 9,424,896 B and the warm get is 608 µs; the cold read moves the chunk.
- A zstd dictionary as a corpus-global side object. On A the env grew by 8,192 B. On B the training set was the trajectory being compressed.
- `lock.mdb` on the published tree, and any open path that needs more than 512 reader slots in one env.
- Stripe count greater than 1 on a file with a single writer, whether that file is a per-rank segment or an LMDB.

## Conditions

The hybrid stands only if all of these hold.

- Each ingest segment is created with stripe count 1. Appends on the shared filesystem are buffered to a full stripe (1 MiB) before a write that extends the file. Per-frame appends of a few kilobytes stay on node-local disk.
- Publish order is fixed: flush and close the segment, then commit the index records for those offsets, then close the index, then let any other rank open it. Crash recovery truncates each segment to the last indexed offset and checks xxh3.
- The published index is per shard, `data.mdb` only, no lock file, opened read-only after the writer has exited. Reader count is not capped by the 512-slot lock table.
- Drain folds node-local segments into the shard directory so the durable tree is O(n_shards), not O(ranks). `snapshot_to` semantics stay: refuse to overwrite an existing dest.
- The segment payload is one zstd frame of the exact CON bytes. xxh3 is taken on the decompressed UTF-8. `frames_soa` stays optional, non-authoritative, and out of this file. A pcodec numeric cache, if it ever exists, is a separate segment with its own locator, never a second copy inside the index env.

A measurement that would move me to pure A: sharded LMDB, 64 envs, one writer per owned shard, node-local then drain, on Lustre, beating segment append on the 8-rank ingest curve and on a cold point read of one frame, with drain bytes inside the slack C reports (0.60× against 0.495×). A measurement that would move me to pure C: a rank count in the thousands, stripe count 1, no per-frame fsync on the MDS, segment ingest still climbing, and a published directory whose open cost stays flat without a drain fold.
