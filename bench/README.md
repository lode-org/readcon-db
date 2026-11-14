# Benchmark records

Each `dbN` directory is one campaign. Older directories are not edited.

`ConCorpus::open` prefetches a database file under 1 MiB with one
`pread` before it returns. Selected cold is the query after that
prefetching open: the open stays outside the clock. `cold-open` is the
end-to-end number: the page cache is dropped outside the clock, and
the clock is the open plus the query. The harness drops the cache
before each sample and does not read the file itself.
