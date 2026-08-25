# FAQ

## Why is this a separate crate?

[readcon-core](https://lode-org.github.io/readcon-core/) is the decoder
and writer for one stream or file. **readcon-db** owns the LMDB tree,
secondary indexes, and multi-reader mmap. Once structures are CON text,
the same files ingest here without a second structure dialect.

## What is authoritative?

UTF-8 CON text. Hashes, dedup, join/split, and `reindex` run on the
stored blobs. Cooked SoA / RCSO / H5MD arrays are derived and
discardable.

## Is this SQL?

No. `Select` is an explicit builder (symbol, \(N\), energy, flags, exact
hash) over B-tree postings. There is no planner.

## Where do large campaigns go?

CON text stays the structure contract. This crate indexes it. Multi-frame
CON files and `iter_con` in core cover trajectory-style loads. For many
writers, use {doc}`campaign` (one writer per shard).

## How do XYZ, PDB, GRO fit in?

They are ingress in **readcon-core** (chemfiles). Convert to CON, then
ingest. Optional `to_ase` is calculator hand-off only; ASE is not on the
read path.

## How do I install it?

{doc}`getting-started`. Rust: `cargo add readcon-db`. Python:
`pip install readcon-db`. C/Fortran: {doc}`install`.
