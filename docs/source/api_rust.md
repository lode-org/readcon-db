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

## Topology keys

Exact match stays xxHash3 on CON bytes. The coarser identity is the bonded
graph up to relabelling from `seams fingerprint FILE --format json`.

```rust
use readcon_db::{AnnotateTopologyOpts, ConCorpus, Select, TrajMeta};

db.annotate_topology(AnnotateTopologyOpts::new(3.0))?;
let hits = db.select(&Select::new().topo_key(hex))?;
let same = db.find_by_topology_path("perm.con")?;
let meta: TrajMeta = db.traj_meta(1)?.unwrap();
// meta.topo_cutoff / topo_graph / topo_hops / topo_method
```

| API | Role |
|-----|------|
| `ConCorpus::annotate_topology` | Shells `seams fingerprint FILE --format json --cutoff C --graph G --hops H`. Resolves the binary from `--seams` / `SEAMS` / `PATH`. Default graph `cutoff`, hops 2. Cutoff required. Refuses a mixed method. |
| `ConCorpus::find_by_topology_path` / `find_by_topology_text` | Fingerprint a file (or CON text) with the recorded corpus params and look up `idx_topo`. Errors if nothing is annotated or if seams is missing. |
| `Select::topo_key` | Prefix-scan `idx_topo` (topology-key utf-8 \|\| `0xff` \|\| `FrameKey`). |
| `TrajMeta` | Optional `topo_cutoff`, `topo_graph`, `topo_hops`, `topo_method` (serde default; old JSON still loads). Ingest/extend preserve these fields. |

CLI:

```text
readcon-db annotate-topology <corpus_dir> --cutoff A [--graph G] [--hops N] [--seams PATH]
readcon-db select <corpus_dir> --topo-key HEX
readcon-db find-by-topology <corpus_dir> <file.con>
```

Missing seams: nonzero exit and a stderr message that names `seams fingerprint`.
How-to: {doc}`howto-topology`.

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
