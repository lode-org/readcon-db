# Where the full-read cold-open spread came from

db13 full-read cold-open was n=12, median 5.95e-4 s, IQR/median 0.322.
Every sample was `mincore` 0 before the open, so the drop had finished.
The means sat in two clusters, about 0.50 ms and 0.76 ms.

That was readahead completion, not a failed drop. `Env::open` faulted
the meta page and started readahead. The named-database walks and a
`pread` after the map was created sometimes waited on the rest of that
readahead and sometimes did not. A `pread` of a corpus under 1 MiB now
runs before the environment is mapped. The file read itself, timed
alone, was about 0.22 ms with IQR/median under 0.10.

A collection between iterations is outside the clock, but it returns
heap pages and the next open faults them. That stepped the twelve
means from about 0.33 ms to 0.43 ms. Automatic collection is off for
every read row, for both tools, and restored when the row returns.

The harness still drops the page cache before every sample and does
not read the file. Selected cold is the query after the prefetching
open. cold-open is the open plus the query. In this record the
full-read cold-open row is median 3.67e-4 s, IQR/median 0.061, against
ASE at 9.74e-3 s, IQR/median 0.015.
