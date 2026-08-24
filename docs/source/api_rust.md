# Rust API

```rust
use readcon_db::{ConCorpus, Select};

let db = ConCorpus::open("/tmp/corpus")?;
db.append_trajectory_path(1, "run.con")?;

let keys = db.select(
    &Select::new()
        .trajectory(1)
        .require_symbol("Cu")
        .natoms_range(1, 10_000)
        .limit(100),
)?;

let h = db.frame_hash(keys[0])?;
assert_eq!(db.find_by_hash(h)?, Some(keys[0]));

let text = db.get_frame_text(keys[0])?;
let frame = db.get_frame(keys[0])?; // ConFrame
```

Exact match:

```rust
let sel = Select::new().exact_hash(h.to_bytes());
let hits = db.select(&sel)?;
```

See crate docs (`cargo doc --open`) for `Error` variants and `ContentHash::to_hex`.

```rust
use readcon_db::{
    canonicalize_unit, join_drained_roots, ConCorpus, ShardedConCorpus,
};

let blob = db.pack_frames(&keys)?;
let a = db.collect_h5md(1)?;
db.append_trajectory_path_units(1, "run.con", Some(serde_json::json!({
    "length": "A", "energy": "ev", "time": "femtosecond"
})))?;
db.set_trajectory_units(1, serde_json::json!({"length": "nm", "energy": "eV"}))?;
let _ = canonicalize_unit("A")?;
ShardedConCorpus::drain_to(src, dest)?;
join_drained_roots(&[dest_a, dest_b], joined)?;
```

`set_trajectory_units` converts stored numbers. `append_trajectory_path_units`
and `extend_trajectory_path_units` stamp incoming frames. Missing
`units.time` is CON `fs` when `collect_h5md` converts to dest `ps`.

## Cooked SoA (RCSO)

See `docs/orgmode/cooked-soa.org`. Tier is opt-in; CON text remains authority. Bindings expose cook / delete / has-valid / positions / forces / velocities.
