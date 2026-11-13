# ASE sqlite comparison

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
* A cold read closes the store, calls `posix_fadvise(DONTNEED)` on the
  file, reopens, and times the read. A warm read is the next read on
  the open store. One warmup is discarded for every series. Samples
  are serial, the two tools alternate, n = 6. The summary is the
  type-7 median and IQR (NumPy `percentile` method `linear`).
* After every insert, and inside every read, each stored field is
  compared by its float64 bytes. A mismatch aborts the run. Raw lines
  are `results/fair/campaign.log`.

The size ladder is one frame of 10³, 10⁴, 10⁵, and 10⁶ atoms, and 1,
100, and 10⁴ frames of 8 atoms. Each point is an insert, a full read,
and a selected-row query (`frame_index` of the middle frame), cold and
warm for the reads.

Cold readcon-db times include opening the environment. The default map
is 2 GiB of address space, so a one-frame cold read is mostly that
open. The warm row is the read with the environment already mapped.

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
