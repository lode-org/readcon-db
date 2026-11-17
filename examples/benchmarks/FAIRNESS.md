# ASE sqlite comparison

## Fair read protocol

One row is one fresh process. The clock covers one operation. Open,
directory fsync, `posix_fadvise`, `mincore`, the field check, and the
collector stay outside it. A read shorter than 50 ms repeats until the
timed spans add to 50 ms, and a commit shorter than 2 s repeats until
they add to 2 s. The sample is that mean. One warmup is discarded.
<!-- fair-docs:begin fairness-n-protocol -->
Twelve samples are kept for every row.
<!-- fair-docs:end fairness-n-protocol -->
The summary is the type-7
median and IQR. Peak RSS is `VmHWM` of that
process, not a high-water mark carried over from an earlier row.

Inside the clock both tools return the same materialized frame, as
NumPy arrays:

* positions, shape `(frames, atoms, 3)`, float64
* cell, shape `(frames, 3, 3)`, float64
* masses, shape `(frames, atoms)`, float64
* atomic numbers, shape `(frames, atoms)`, uint32
* constraints, shape `(frames, atoms, 3)`, uint8, 1 where that axis is fixed
* velocities, shape `(frames, atoms, 3)`, float64, zeros when the frame stored none
* forces, the same shape, or absent when the frame stored none

readcon-db copies those columns out of the cooked RCSO blob (and parses
the CON text only when that blob lacks the extended blocks). ASE sqlite
does `select`, then `toatoms()`, and reads masses, atomic numbers,
velocities, the cell, and forces from that object. The element-wise
check of those arrays is outside the clock. An insert's clock is the
commit only: the CON text or the ASE atoms are built before it, and the
check does not build a second per-atom frame. Both tools use one write
transaction and one commit fsync. ASE is SQLite `journal_mode=DELETE`,
`synchronous=FULL`, and its 5000-row commit is off. A readcon-db
trajectory longer than 64 frames puts one zstd group at a time and
drops that group's cooked blob and decompressed text after the put.
Those puts share the one transaction. LMDB's dirty pages hold the
bytes until that commit. A failed insert aborts the transaction, so
no frame from it is visible.

Both tools get the same cache treatment on every row.

`cold`: `posix_fadvise(DONTNEED)` until `mincore` is 0, then open,
then the query. The open stays outside the clock. The harness does
not read the file. readcon-db's open reads a corpus under 1 MiB with
one `pread` before it returns, so the query is a read after that
prefetch. ASE gets the same harness steps:
drop, then sqlite's own open, then the query.

`warm-after-open`: the same drop, then that tool's own open, then the
query. No harness pre-read. The open stays outside the clock.

`cold-open`: the drop is outside the clock. The clock is only the
library open plus the query. readcon-db's open includes the `pread`.
ASE's open is sqlite's open. The harness does not read the file.
Automatic collection is off for every read row, for both tools, and
restored when the row returns. A collection between iterations is
outside the clock, but it returns heap pages and the next open faults
them.

`warm`: one warmup read on the already open store, then the query.
Both tools.

No cpufreq governor is exposed here, and the TSC is constant. For a
corpus under 1 MiB the query also `posix_fadvise(WILLNEED)`s the
database file and `madvise(WILLNEED)`s a cooked value of at least
4 KiB before copying it.

<!-- fair-docs:begin fairness-db8 -->
The db8 frames-100 selected cold row, open outside the clock, is n=12, median 1.6688e-04 s, p90 of those means 1.7715e-04 s, IQR/median 0.10058. Its per-call distribution is `bench/db8/noise.json`.
<!-- fair-docs:end fairness-db8 -->

The db9 rows are `bench/db9/`.

Cooked frames are stored in frame-key order, one RCSO blob per frame.
A 100-frame `tiny_cuh2` trajectory is 27,900 cooked bytes: 9 pages,
inside a 15-page span. One contiguous value per 64-frame group would
be 7 pages. `frames-100` is about 180 KiB. `open` reads a corpus under
1 MiB with one `pread` before the environment is mapped, so selected
cold is the query after that prefetch and `cold-open` is the
end-to-end number. The per-frame blobs stay.

The CPC ingest is `db_v2.py` in readcon-core.

<!-- fair-docs:begin fairness-cpc -->
The published CPC samples in `fair-db32/campaign.json` (`published_db_v2`, 40 frames of 8 Cu) give readcon-db a type-7 median **4.6724e-03 s** (n=6). ASE's 6 samples are 1.0352e-01 s, 8.1242e-02 s, 1.0350e-01 s, 5.9195e-02 s, 6.3394e-02 s, 5.4239e-02 s. Type-7 median **7.2318e-02 s**, Q1 6.0244e-02 s, Q3 9.7937e-02 s, IQR **3.7693e-02 s**.
<!-- fair-docs:end fairness-cpc -->

## What the published timer actually did

Both sides stored a different row.

* readcon-db received the CON text already in memory and committed it
  in one LMDB transaction (`MDB_NOSYNC` is unset, so that commit
  fsyncs). The timer also ran `select` and checked the frame count.
* ASE parsed `db.con` from disk inside the timer (`ase.io.read`,
  format `eon`) and called `db.write` once per frame with no `with db`
  block. Each `write` opens SQLite, inserts one row, and commits.
  SQLite's default is `journal_mode=DELETE` and `synchronous=FULL`, so
  each commit fsyncs. Forty frames are forty fsyncs. The backend,
  journal mode, and synchronous level were not recorded.
* Nothing was read back. The check is the frame count.
* There is no discarded warmup. The repeats alternate which tool
  runs first. ASE's samples are slower when they follow readcon-db's
  commit, and the CON file stays in the page cache after the first
  parse while each sqlite file is a new inode only after the child
  exits. The IQR is that mixture: per-row fsync latency, writeback
  left by the other store, and a parse that gets cheaper as the page
  cache warms. The samples are kept.

## What this tree measures

`examples/benchmarks/fair_ase_protocol.py` keeps every sample and
changes the harness.

* The same records go into both stores: symbols, positions, cell,
  periodicity, masses, forces, energy, and `frame_index`. The
  40×8 rerun is the published text; ASE is given `to_ase()` of those
  frames, parsed once, outside the timer.
* ASE is SQLite (`connect(..., type="db")`), `journal_mode=DELETE`,
  `synchronous=FULL`. Those pragmas are read back into the log.
  LMDB still fsyncs on commit. Each insert is one transaction and one
  commit fsync. ASE's 5000-row intermediate commit is turned off for
  the timed connection. The commit counter must be 1 or the run stops.
* Each insert sample unlinks the directory and creates a new database
  file, then fsyncs that directory, before the timer. The previous
  sample's pages belong to an inode that is gone. Schema creation sits
  outside the timer on both sides, so the timed section is the data
  transaction.
* A cold read drops the page cache with `posix_fadvise(DONTNEED)` on
  the closed database files, then times a readonly open plus the read.
  `/proc/sys/vm/drop_caches` is not writable on this host, so it is not
  used. A fresh file would still be in the page cache after the write,
  and it would not be a colder read than the advice. A warm read is
  the next read on the open store. One warmup is discarded for every
  series.

<!-- fair-docs:begin fairness-n -->
Samples are serial, the two tools alternate, n = 12.
<!-- fair-docs:end fairness-n -->

* The summary is the type-7 median and IQR (NumPy `percentile` method
  `linear`). The timed close does not call `gc.collect()`.
* After every insert, and inside every read, each stored field is
  compared by its float64 bytes. A mismatch aborts the run. Raw lines
  are `results/fair-db29/campaign.log`.

The size ladder is one frame of 10³, 10⁴, 10⁵, and 10⁶ atoms, and 1,
100, and 10⁴ frames of 8 atoms. Each point is an insert, a full read,
and a selected-row query (`frame_index` of the middle frame), cold and
warm for the reads.

A CON text above 512 KiB is indexed from the stored span. Coordinate,
fixed-flag, and force rows are checked for field count and float syntax,
and the declared atom counts are checked, without building a per-atom
object. A long row block is scanned on several threads. Force magnitudes
still reduce to `fmax` with the same fast-float kernel as a frame parse,
and the bytes stored are still that span (zstd when it shrinks). The fair
insert of that frame is the 10^6-atom row in the generated table below.
Texts at or below 512 KiB
keep the full parse, and a malformed frame is rejected at both sizes.

Cold readcon-db times include a readonly open. The previous cold rows
were slower than ASE on the small corpora because the timed close
called `gc.collect()` and the open was writable, so it could commit.
The read itself is the open plus the column read once the collection
is outside the timer. The default map stays 8 MiB, and reopening an
existing corpus still maps at least twice `data.mdb`.

## Warm full read

The old full read copied the span out of LMDB, decompressed it into a
Python `str`, then `readcon.read_con_string` built a frame object and
the check walked one Python atom per site.

`ConCorpus.read_columns` parses the stored bytes in place when they are
uncompressed, decompresses a zstd group once, and fills column buffers
with the same fast-float kernel as readcon-core. NumPy arrays view those
buffers. The GIL is released for the scan. Field checks stay on the
float64 bytes.

<!-- fair-docs:begin fairness-tables -->
Warm full read, cold full read, and cold single-frame read from `fair-db32/campaign.json`. Each cell is the type-7 median. n is the sample count stored for that row.

| workload | op | cache | readcon-db | ASE | faster |
| --- | --- | --- | --- | --- | --- |
| 10^3 atoms, 1 frame | full_read | warm | 1.6831e-05 (n=12) | 4.3995e-04 (n=12) | readcon-db |
| 10^3 atoms, 1 frame | full_read | cold | 3.0909e-05 (n=12) | 1.4131e-03 (n=12) | readcon-db |
| 10^3 atoms, 1 frame | selected | cold | 2.9382e-05 (n=12) | 1.4830e-03 (n=12) | readcon-db |
| 10^4 atoms, 1 frame | full_read | warm | 2.5438e-04 (n=12) | 3.1741e-03 (n=12) | readcon-db |
| 10^4 atoms, 1 frame | full_read | cold | 2.8230e-04 (n=12) | 4.9253e-03 (n=12) | readcon-db |
| 10^4 atoms, 1 frame | selected | cold | 2.7642e-04 (n=12) | 4.8785e-03 (n=12) | readcon-db |
| 10^5 atoms, 1 frame | full_read | warm | 2.5267e-03 (n=12) | 2.8448e-02 (n=12) | readcon-db |
| 10^5 atoms, 1 frame | full_read | cold | 5.6241e-03 (n=12) | 3.4032e-02 (n=12) | readcon-db |
| 10^5 atoms, 1 frame | selected | cold | 5.2856e-03 (n=12) | 3.3449e-02 (n=12) | readcon-db |
| 10^6 atoms, 1 frame | full_read | warm | 1.7261e-02 (n=12) | 3.0055e-01 (n=12) | readcon-db |
| 10^6 atoms, 1 frame | full_read | cold | 5.7936e-02 (n=12) | 3.4425e-01 (n=12) | readcon-db |
| 10^6 atoms, 1 frame | selected | cold | 4.7590e-02 (n=12) | 3.2115e-01 (n=12) | readcon-db |
| 8 atoms, 1 frame | full_read | warm | 1.1489e-05 (n=12) | 1.5351e-04 (n=12) | readcon-db |
| 8 atoms, 1 frame | full_read | cold | 2.2806e-05 (n=12) | 5.8094e-04 (n=12) | readcon-db |
| 8 atoms, 1 frame | selected | cold | 2.1025e-05 (n=12) | 8.2697e-04 (n=12) | readcon-db |
| 8 atoms, 100 frames | full_read | warm | 4.3081e-05 (n=12) | 8.6057e-03 (n=12) | readcon-db |
| 8 atoms, 100 frames | full_read | cold | 6.7944e-05 (n=12) | 1.0095e-02 (n=12) | readcon-db |
| 8 atoms, 100 frames | selected | cold | 2.0075e-05 (n=12) | 1.1083e-03 (n=12) | readcon-db |
| 8 atoms, 10^4 frames | full_read | warm | 6.6220e-03 (n=12) | 9.0089e-01 (n=12) | readcon-db |
| 8 atoms, 10^4 frames | full_read | cold | 6.0512e-03 (n=12) | 9.2588e-01 (n=12) | readcon-db |
| 8 atoms, 10^4 frames | selected | cold | 4.3335e-05 (n=12) | 4.5176e-03 (n=12) | readcon-db |

The 10^6-atom warm one-thread full read in this table is the fair-db32 median 1.7261e-02 s. On this host the same sampling rule gave medians of 1.726e-02 s to 3.447e-02 s across campaigns. sample_operation repeats the call until the timed spans reach 50 ms and stores the mean. fair-db30 has eleven samples at inner_n=2 and one at inner_n=1. fair-db31 has inner_n=3 on all twelve. fair-db32 has eleven samples at inner_n=3 and one at inner_n=4. `bench/db31/ase` is the Cu2 stand-in. The equal comparison is fair-db32.

The large inserts do not share a winner (10^4 atoms, 1 frame: readcon-db 3.6756e-03 (n=12), ASE 3.7001e-03 (n=12), readcon-db is faster; 10^5 atoms, 1 frame: readcon-db 2.4768e-02 (n=12), ASE 2.4911e-02 (n=12), readcon-db is faster; 10^6 atoms, 1 frame: readcon-db 2.8619e-01 (n=12), ASE 2.3656e-01 (n=12), ASE is faster).
readcon-db is faster on the read rows in this table and on the other insert rows.

## This machine

Regenerated from `fair-db32/`. Commit `d8bf1cb567c4117a995fcc7605d1b8dc488324a8`. The host line in the log is the neutral label.

The published-harness rerun in that directory, one warmup discarded, is readcon-db 4.1602e-03 (IQR 8.1670e-04, n=12) and ASE 7.7911e-02 (IQR 6.4512e-03, n=12).

The fair 40×8 insert, one commit each, is readcon-db 2.2603e-03 and ASE 5.7336e-03.
<!-- fair-docs:end fairness-tables -->
