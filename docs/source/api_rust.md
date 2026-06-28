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

## Cooked SoA (RCSO)

See `docs/orgmode/cooked-soa.org`. Tier is opt-in; CON text remains authority. Bindings expose cook / delete / has-valid / positions (and forces on C/Python/Rust).
