# Workflows

## CON-native (default)

Optimizers → **CON files** → `readcon-db` ingest → `select` / `get_frame` / C/`readcon` decode.
No ASE on this path.

```bash
readcon-db ingest-dir /data/corpus /data/neb_runs
readcon-db select /data/corpus --symbol Cu --require-forces \
  --energy-min -50 --energy-max 0
```

Metadata predicates use secondary indexes documented in [architecture](architecture.md)
(`idx_energy`, `idx_flags` alongside `idx_natoms` / `idx_symbol`).

## XYZ and other formats

Use **readcon-core chemfiles ingress** (`read_chemfiles`, Rust/C equivalents) to obtain
`ConFrame`s, write CON if needed, then ingest. Do **not** use ASE as the XYZ reader
for this stack. Peer docs: [readcon-core](https://lode-org.github.io/readcon-core/).

## Optional XYZ *export*

`export_extxyz` / CLI `dedup-export` only for external tools that demand XYZ on disk.
Implementation does not call ASE.

## ASE `.db` comparison (measurement only)

See repository `examples/benchmarks/` and the CPC manuscript CSE section. Those timings
are **unequal workloads** (lightweight ASE `Cu2` stand-ins vs full CON parse+index on
readcon-db)—CSE orientation for multi-reader behaviour, **not** a fair store-vs-store
parity claim and **not** “store Atoms in ASE.db” as the product path.
