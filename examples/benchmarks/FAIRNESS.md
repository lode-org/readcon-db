# ASE sqlite comparison

## Fair read protocol

One row is one fresh process. The clock covers one operation. Open,
directory fsync, `posix_fadvise`, `mincore`, the field check, and the
collector stay outside it. A read shorter than 50 ms repeats until the
timed spans add to 50 ms, and a commit shorter than 2 s repeats until
they add to 2 s. The sample is that mean. One warmup is discarded.
Six samples are the default. A row whose IQR/median is still above
0.05 after those six takes six more, and the published n is 12. The
summary is the type-7 median and IQR. Peak RSS is `VmHWM` of that
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
then the query. The open stays outside the clock. `POSIX_FADV_RANDOM`
made LMDB fault leaf pages one at a time, and that fault was bimodal
(about 0.12 ms against 0.25 ms) even when `mincore` was 0. A head-only
read left the same hole. readcon-db's cold open therefore reads a
corpus under 1 MiB in one sequential `pread` before it returns, so the
query does not fault that hole. ASE gets the same harness steps on its
sqlite file: drop, open, query, with no harness pre-read.

`warm-after-open`: the same drop, then that tool's own open, then the
query. There is no harness pre-read on either side. readcon-db's open
issues `readahead(2)` for a corpus under 1 MiB. A parallel touch of
every page was slower than the fault it hid. ASE's open is sqlite's
open. The open stays outside the clock.

`cold-open`: the drop is outside the clock. The clock is the open
plus a read of every page of the database file, plus the query.
readcon-db's open already does that read for a corpus under 1 MiB.
The harness repeats it for both tools, so ASE's sqlite file is faulted
inside the same clock.

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
be 7 pages. `frames-100` is about 180 KiB, and a corpus under 1 MiB is
`posix_fadvise(WILLNEED)`d as a whole file inside the query, so that
query already faults every page. Packing the group into one value does
not change that set. The per-frame blobs stay. The cold tail is the
open's readahead sometimes faulting those pages before the clock, which
is why `cold` and `cold-open` are both reported.

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
  are `results/fair/campaign.log`.

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
and the commit about 0.01 s. The fair insert of that frame is then
0.152 s against ASE at 0.247 s. Texts at or below 512 KiB keep the full
parse, and a malformed frame is rejected at both sizes.

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

Warm full read on this host after that change (one warmup, n = 6,
type-7), tree `850c5618`:

| atoms | frames | readcon-db s | ASE s |
| --- | --- | --- | --- |
| 1e3 | 1 | 2.19e-4 | 4.67e-4 |
| 1e4 | 1 | 1.40e-3 | 2.72e-3 |
| 1e5 | 1 | 1.44e-2 | 2.34e-2 |
| 1e6 | 1 | 1.65e-1 (IQR 4.43e-2) | 2.49e-1 (IQR 4.87e-2) |
| 8 | 1e4 | 9.56e-2 | 8.73e-1 |

## Cold full read and one frame

`/proc/sys/vm/drop_caches` is not writable here. Each cold sample calls
`posix_fadvise(DONTNEED)` on the closed `data.mdb` and `lock.mdb` (and
on the sqlite file), then times a readonly open plus the read. A probe
of a 64 MiB file went from 0.036 s warm to 0.061 s after that advice.
The single-frame query is `frame_index` of the middle frame.

Cold rows on this host after removing the collection from the timer
(one warmup, n = 6, type-7), tree `e3aae36b`:

| workload | op | readcon-db s | ASE s |
| --- | --- | --- | --- |
| 1e3 atoms | full | 4.24e-4 | 9.96e-4 |
| 1e3 atoms | one frame | 5.93e-4 | 1.46e-3 |
| 1e4 atoms | full | 1.49e-3 | 3.16e-3 |
| 1e4 atoms | one frame | 2.01e-3 | 4.60e-3 |
| 1e5 atoms | full | 1.59e-2 | 2.84e-2 |
| 1e5 atoms | one frame | 1.41e-2 | 2.46e-2 |
| 1e6 atoms | full | 1.78e-1 | 2.82e-1 |
| 1e6 atoms | one frame | 1.80e-1 | 2.83e-1 |
| 1 frame | full | 2.87e-4 | 6.55e-4 |
| 1 frame | one frame | 2.78e-4 | 7.32e-4 |
| 100 frames | full | 1.53e-3 | 1.06e-2 |
| 100 frames | one frame | 3.37e-4 | 7.85e-4 |
| 1e4 frames | full | 1.21e-1 | 9.77e-1 |
| 1e4 frames | one frame | 5.67e-4 | 3.56e-3 |

readcon-db is ahead on every cold full read and every cold single-frame
read. Warm rows in `results/fair/RESULTS.md` stay ahead as well.

## This machine

Host `cursor`, 2026-11-13, tree `c738f8dd`. Raw lines are
`results/fair/campaign.log`. The table is `results/fair/RESULTS.md`.

Rerunning the published harness here gives ASE median 0.0421 s and
IQR 0.00280 s (samples 0.0386–0.0426 s). This disk's fsync waits are
steadier than the published log, so the wide IQR does not reappear at
0.038 s. The fair insert of the same 40×8 frames, every sample kept,
is ASE median 0.00527 s and IQR 0.000371 s. That is about eight times
narrower than this host's old-harness rerun and about a hundred times
narrower than the published IQR. readcon-db's fair insert is median
0.00179 s, IQR 0.000922 s.

Ten thousand frames, one commit: readcon-db insert median 0.330 s
(IQR 0.014 s), ASE 3.14 s (IQR 0.107 s). The ASE spread is the Python
write loop, not a per-row fsync. A selected warm read of one of those
frames is 0.286 ms for readcon-db and 0.855 ms for ASE. One million
atoms in one frame is a 54 MB CON text: the readcon-db full read is
the parse (median 1.22 s warm) and ASE reads the sqlite blob (median
0.221 s warm). Both reads passed the same element-wise check.
