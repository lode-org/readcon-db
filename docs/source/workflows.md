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


## Fair ASE.db vs readcon-db campaign

Use **`examples/benchmarks/fair_campaign.py`**: builds a multi-frame CON ladder from a real fixture,
loads **the same frames** into ASE `.db` (via readcon geometry → `Atoms`) and **readcon-db**,
records insert/extract/competitive select/8-reader timings, and checks **hit-count agreement**
for symbol `Cu` and `natoms` range. Results: JSON `ase_fair_campaign_{run}.json` and markdown table.

```bash
# venv with ase + maturin-developed readcon / readcon_db
python examples/benchmarks/fair_campaign.py --out /tmp/fair_out --run-id 1
python examples/benchmarks/test_fair_select_parity.py
```

Legacy `bench_ase_db.py` Cu2 timings are **unequal-workload** artifacts only.
Interchange axis (parse CON): campaign JSON field `interchange` (readcon vs `ase.io.read`).
