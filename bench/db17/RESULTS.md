# traj-100k cold A/B, db15 binary against this tree

One corpus, written by the db15 binary (`9e568240`). `data.mdb` is
342003712 bytes, the same size as the db15 and db16 traj-100k files.
The two readers are release wheels of that commit and of this tree.
Samples alternate on one host: for each op, index 0 is db15 then
db17, through index 11. n=12. The fair cold row is unchanged: the
file is mincore 0 before the open, the open is outside the clock,
and `sample_operation` times only the query. Automatic collection
is off for both processes. `record_table_usage` is not in this run.

The blobs are the db15 per-frame values. Nothing in the read is a
cooked group.

| op | db15 median s | db17 median s | db17/db15 | db15 IQR s |
| --- | --- | --- | --- | --- |
| selected | 6.561408e-03 | 6.357138e-03 | 0.969 | 1.003353e-03 |
| range | 1.663252e-01 | 1.636844e-01 | 0.984 | 3.214212e-02 |
| metadata | 2.863644e-04 | 2.905288e-04 | 1.014 | 3.007661e-05 |

Selected and range are under the db15 binary. The separate db16
campaign had range at 2.0671e-1 against 1.7021e-1. That gap is not
there when the two binaries share the file and alternate.

Metadata's select source is the db15 function. Its median is 4.2 µs
higher in this order (9 µs on the paired differences, db17 slower on
10 of 12 pairs). A second block with the order reversed, db17 then
db15, is in `ab.json` under `metadata_reverse_order`:

| order | db15 median s | db17 median s | db17/db15 |
| --- | --- | --- | --- |
| db17 then db15 | 3.088023e-04 | 3.089034e-04 | 1.000 |

The reversed medians differ by 0.1 µs, and db17 is slower on 7 of 12
pairs. The campaign's 27 µs metadata gap does not survive the
interleave. Samples are in `ab.json`.
