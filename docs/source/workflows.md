# Workflows

## metatrain / ASE training sets

See [examples/workflows/metatrain_from_con.md](https://github.com/lode-org/readcon-db/blob/main/examples/workflows/metatrain_from_con.md) in the repository.

Summary: `ingest-dir` → `dedup-export --symbol … -o train.xyz` → metatrain `training_set.systems.read_from: train.xyz`.

## Deduplication

Identical CON content (after canonical re-serialize) shares xxHash3-128. `unique_frame_keys` / CLI `dedup-export` keep the first representative key per hash — useful when the same saddle is written from multiple NEB restarts.
