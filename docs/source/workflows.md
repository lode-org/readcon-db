# Workflows

## CON-native (default)

Optimizers → **CON files** → `readcon-db` ingest → `select` / `get_frame` / C/`readcon` decode.
No ASE on this path.

## XYZ and other formats

Use **readcon-core chemfiles ingress** (`read_chemfiles`, Rust/C equivalents) to obtain
`ConFrame`s, write CON if needed, then ingest. Do **not** use ASE as the XYZ reader
for this stack.

## Optional XYZ *export*

`export_extxyz` / CLI `dedup-export` only for external tools that demand XYZ on disk.
Implementation does not call ASE.

## ASE `.db` comparison

See repository `examples/benchmarks/` and the CPC manuscript CSE section: mmap multi-reader
CON store vs SQLite `Atoms` store—not “ASE cannot open CON.”
