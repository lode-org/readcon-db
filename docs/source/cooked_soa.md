# Cooked SoA (RCSO)

```{admonition} CON text is the sole authority
:class: important

RCSO in `frames_soa` is **non-authoritative**. UTF-8 CON text in `frames`
is the **sole authority** for hash, dedup, join/split, `reindex`, CON
export, and numeric extract. If a valid RCSO blob disagrees with CON,
`get_positions` / `get_forces` / `get_velocities` / `collect_h5md` return
CON. Do not omit the text tier when cooked exists.
```

Optional cooked SoA is opt-in (`cook_frame` / `recook_all` / cook-on-ingest).
Default ingest stores CON text only.

## Two tiers

| Tier | LMDB DB | Role |
|------|---------|------|
| Authority | `frames` | UTF-8 CON text (span at ingest when possible) |
| Optional numerics | `frames_soa` | RCSO v1 little-endian POD (positions $N\times 3$ `f64`, optional forces/velocities) |

RCSO is **not** fully equivalent to CON: it omits element symbols, masses,
cell/angles, constraints, JSON metadata, and exact on-disk bytes. Those
are required for xxHash3 dedup, symbol/formula indexes, `reindex`,
join/split, and CON export.

## Numeric extract

`get_positions` / `get_forces` / `get_velocities` and `collect_h5md` parse
CON text when `frames` has a blob. A valid RCSO cache is used only if CON
is missing (unsupported storage). Pack/bcast may skip parse on a valid
cooked hit. Authority APIs (`frame_hash`, `find_by_hash`, `get_frame_text`,
`reindex`, shard join) never read `frames_soa`.

CI runs `numeric_extract_prefers_con_when_rcso_disagrees` and
`collect_h5md_prefers_con_when_rcso_disagrees`: a valid but disagreeing
RCSO blob must not win.

## Language surfaces

See [architecture](architecture.md) and `docs/orgmode/cooked-soa.org` for
the cook / delete / has-valid / getter table (Rust, Python, C/C++,
Fortran, CLI).
