# ASE sqlite comparison

## Fair read protocol

One row is one fresh process. The clock covers one operation. Open,
directory fsync, `posix_fadvise`, `mincore`, the field check, and the
collector stay outside it. A read shorter than 50 ms repeats until the
timed spans add to 50 ms, and a commit shorter than 2 s repeats until
they add to 2 s. The sample is that mean. One warmup is discarded.
Twelve samples are kept for every row. The summary is the type-7
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
prefetch. `readahead(2)` instead of the `pread` left the full read
waiting on the rest of the fault on some samples and not on others
(about 0.50 ms against 0.76 ms). ASE gets the same harness steps:
drop, then sqlite's own open, then the query.

`warm-after-open`: the same drop, then that tool's own open, then the
query. No harness pre-read. The open stays outside the clock.

`cold-open`: the drop is outside the clock. The clock is only the
library open plus the query. readcon-db's open includes the `pread`.
ASE's open is sqlite's open. The harness does not read the file.
Automatic collection is off for every read row, for both tools, and
restored when the row returns. A collection between iterations is
outside the clock, but it returns heap pages and the next open faults
them. On the full-read cold-open row that fault stepped the means
from about 0.33 ms to 0.43 ms halfway through the twelve.

`warm`: one warmup read on the already open store, then the query.
Both tools.

No cpufreq governor is exposed here, and the TSC is constant. For a
corpus under 1 MiB the query also `posix_fadvise(WILLNEED)`s the
database file and `madvise(WILLNEED)`s a cooked value of at least
4 KiB before copying it. The db8 frames-100 selected
cold row, open outside the clock, is n=12, median 1.56e-4 s, p90 of
those means 1.95e-4 s, IQR/median 0.251. Its per-call distribution is
`bench/db8/noise.json`. The db9 rows are `bench/db9/`.

Cooked frames are stored in frame-key order, one RCSO blob per frame.
A 100-frame `tiny_cuh2` trajectory is 27,900 cooked bytes: 9 pages,
inside a 15-page span. One contiguous value per 64-frame group would
be 7 pages. `frames-100` is about 180 KiB. `open` reads a corpus under
1 MiB with one `pread` before the environment is mapped, so selected
cold is the query after that prefetch and `cold-open` is the
end-to-end number. The per-frame blobs stay.

The CPC ingest is `db_v2.py` in readcon-core, recorded in
`paper/cpc/logs/db.log` (2026-11-05, 40 frames of 8 Cu). readcon-db's
six samples have type-7 median **0.00467 s**. ASE's six samples are
0.0542, 0.0592, 0.0634, 0.0812, 0.1035, 0.1035 s. Type-7 median
**0.0723 s**, Q1 0.0602 s, Q3 0.0979 s, IQR **0.0377 s**. That is the
wide band around 0.07–0.10 s. No sample was dropped to shrink it.

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
* There is no discarded warmup. The six repeats alternate which tool
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
  series. Samples are serial, the two tools alternate, n = 6. The
  summary is the type-7 median and IQR (NumPy `percentile` method
  `linear`). The timed close does not call `gc.collect()`: that walk
  was an 8 ms floor on every cold readcon-db sample.
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
and the bytes stored are still that span (zstd when it shrinks). On this
host the million-atom prepare was about 0.41 s when it built one atom
per site and about 0.15 s from the span, of which zstd was about 0.08 s
and the commit about 0.01 s. The fair insert of that frame is the
10^6-atom row in the generated table below. Texts at or below 512 KiB
keep the full parse, and a malformed frame is rejected at both sizes.

Cold readcon-db times include a readonly open. The previous cold rows
were slower than ASE on the small corpora because the timed close
called `gc.collect()` and the open was writable, so it could commit.
The read itself was not that slow: a one-frame open plus column read
is a few hundred microseconds once the collection is outside the
timer. The default map stays 8 MiB, and reopening an existing corpus
still maps at least twice `data.mdb`.

## Warm full read

The old full read copied the span out of LMDB, decompressed it into a
Python `str`, then `readcon.read_con_string` built a frame object and
the check walked one Python atom per site. On the 54,086,580-byte
million-atom CON that split was about 0.055 s to materialise the text
and 0.148 s to build the frame, and the atom walk took the measured
warm read to 1.22 s. readcon-core's own serial parse of a CON this size
is about 0.039 s, so the gap was the copy and the per-atom objects.

`ConCorpus.read_columns` parses the stored bytes in place when they are
uncompressed, decompresses a zstd group once, and fills column buffers
with the same fast-float kernel as readcon-core. NumPy arrays view those
buffers. The GIL is released for the scan. Field checks stay on the
float64 bytes.

<!-- fair-docs:begin fairness-tables -->
Warm full read, cold full read, and cold single-frame read from `results/fair-db29/campaign.json`. Each cell is the type-7 median. n is the sample count stored for that row.

| workload | op | cache | readcon-db | ASE | faster |
| --- | --- | --- | --- | --- | --- |
| 10^3 atoms, 1 frame | full_read | warm | 1.6555e-05 (n=6) | 7.9115e-04 (n=12) | readcon-db |
| 10^3 atoms, 1 frame | full_read | cold | 3.1883e-05 (n=6) | 2.0304e-03 (n=12) | readcon-db |
| 10^3 atoms, 1 frame | selected | cold | 3.1240e-05 (n=6) | 1.7569e-03 (n=6) | readcon-db |
| 10^4 atoms, 1 frame | full_read | warm | 2.7152e-04 (n=6) | 4.3440e-03 (n=6) | readcon-db |
| 10^4 atoms, 1 frame | full_read | cold | 2.9294e-04 (n=6) | 5.2235e-03 (n=12) | readcon-db |
| 10^4 atoms, 1 frame | selected | cold | 2.9313e-04 (n=6) | 5.5761e-03 (n=6) | readcon-db |
| 10^5 atoms, 1 frame | full_read | warm | 2.5745e-03 (n=6) | 3.4530e-02 (n=12) | readcon-db |
| 10^5 atoms, 1 frame | full_read | cold | 6.2759e-03 (n=12) | 3.3129e-02 (n=6) | readcon-db |
| 10^5 atoms, 1 frame | selected | cold | 7.4136e-03 (n=9) | 3.9297e-02 (n=6) | readcon-db |
| 10^6 atoms, 1 frame | full_read | warm | 2.2889e-02 (n=6) | 3.0143e-01 (n=6) | readcon-db |
| 10^6 atoms, 1 frame | full_read | cold | 6.7073e-02 (n=12) | 3.5340e-01 (n=12) | readcon-db |
| 10^6 atoms, 1 frame | selected | cold | 6.7658e-02 (n=12) | 3.5220e-01 (n=6) | readcon-db |
| 8 atoms, 1 frame | full_read | warm | 1.1674e-05 (n=6) | 1.5213e-04 (n=6) | readcon-db |
| 8 atoms, 1 frame | full_read | cold | 2.3269e-05 (n=12) | 5.7678e-04 (n=8) | readcon-db |
| 8 atoms, 1 frame | selected | cold | 2.1031e-05 (n=6) | 6.4928e-04 (n=12) | readcon-db |
| 8 atoms, 100 frames | full_read | warm | 4.4052e-05 (n=6) | 9.2584e-03 (n=12) | readcon-db |
| 8 atoms, 100 frames | full_read | cold | 6.8926e-05 (n=6) | 1.0129e-02 (n=6) | readcon-db |
| 8 atoms, 100 frames | selected | cold | 2.2400e-05 (n=12) | 9.0448e-04 (n=12) | readcon-db |
| 8 atoms, 10^4 frames | full_read | warm | 6.3478e-03 (n=12) | 1.0671e+00 (n=6) | readcon-db |
| 8 atoms, 10^4 frames | full_read | cold | 6.4131e-03 (n=6) | 1.0325e+00 (n=6) | readcon-db |
| 8 atoms, 10^4 frames | selected | cold | 4.6303e-05 (n=6) | 5.1330e-03 (n=12) | readcon-db |

ASE is faster on the 10^4-atom, 10^5-atom, and 10^6-atom inserts (10^4 atoms, 1 frame: readcon-db 3.8324e-03 (n=6), ASE 3.6268e-03 (n=12); 10^5 atoms, 1 frame: readcon-db 2.9094e-02 (n=6), ASE 2.7197e-02 (n=6); 10^6 atoms, 1 frame: readcon-db 3.0821e-01 (n=6), ASE 2.5567e-01 (n=10)).
readcon-db is faster on the read rows in this table and on the other insert rows.

## This machine

Regenerated from `results/fair-db29/`. Commit `67dc0235955e6c0f50a8e814d4d489fbb387ec65`. The host line in the log is the neutral label.

The published-harness rerun in that directory, one warmup discarded, is readcon-db 5.1147e-03 (IQR 3.5265e-03, n=6) and ASE 5.3779e-02 (IQR 2.5180e-03, n=6).

The fair 40×8 insert, one commit each, is readcon-db 2.2871e-03 and ASE 5.6017e-03.
<!-- fair-docs:end fairness-tables -->
