# How-to: topology keys for near-duplicate frames

xxHash3 on stored CON bytes is exact identity. The bonded graph up to
relabelling is a coarser identity: two minima or saddles that share a
graph and ring census share a topology key even when coordinates differ
by relaxation or a permutation.

d-SEAMS computes that key (`seams fingerprint FILE --format json`). This
crate stores the frame key; it does not reimplement the certificate.

## Prerequisites

A writeable corpus directory needs at least one ingested trajectory. The
`seams` binary must be on `PATH`, or set `SEAMS=/path/to/seams`, or pass
`--seams PATH`. The engine must be seams-core at `44f91bad` or later so
the `fingerprint` command exists.

A missing binary makes `annotate-topology` and `find-by-topology` exit
nonzero and name the `seams fingerprint` command on stderr.

## Annotate a corpus

The cutoff is required and is per-corpus. The default graph is `cutoff`.
The default hop count is 2.

```bash
readcon-db ingest /tmp/corpus resources/test/tiny_cuh2.con
readcon-db annotate-topology /tmp/corpus --cutoff 3.0
# optional:
#   --graph cutoff --hops 2 --seams /path/to/seams
```

Rust:

```rust
use readcon_db::{AnnotateTopologyOpts, ConCorpus};

let db = ConCorpus::open("/tmp/corpus")?;
db.append_trajectory_path(1, "tiny_cuh2.con")?;
db.annotate_topology(AnnotateTopologyOpts::new(3.0))?;
```

Each annotated trajectory records `topo_cutoff`, `topo_graph`,
`topo_hops`, and `topo_method` on `TrajMeta`. A later run that would mix
methods, or disagree on cutoff, graph, or hops, is refused.

On `tiny_cuh2.con`, a 3.0 A cutoff yields a non-empty graph (Cu-Cu is
about 2.56 A; H-H is about 0.74 A).

## Select by topology key

```bash
readcon-db select /tmp/corpus --topo-key HEX
```

```rust
let keys = db.select(&Select::new().topo_key(hex))?;
```

`select` prefix-scans `idx_topo` the same way `exact_composition` scans
`idx_formula`. A readonly open of an older corpus that lacks `idx_topo`
treats the index as empty.

## Look up a candidate file

The lookup fingerprints the file with the recorded corpus parameters and
returns matching `FrameKey`s:

```bash
readcon-db find-by-topology /tmp/corpus candidate.con
```

```rust
let hits = db.find_by_topology_path("candidate.con")?;
```

A permuted copy of a stored frame (swap two Cu rows) hits the original.
A frame with a broken bond (one H displaced by about 10 A) does not.

A corpus with no recorded topology parameters errors and names
`annotate-topology`.

## Reindex

`readcon-db reindex` rebuilds the existing secondary indexes. It rebuilds
`idx_topo` from stored `topo_by_frame` values when every annotated
trajectory records the same cutoff/graph/hops/method. Mixed
parameters return an error and do not half-rebuild. An unannotated
corpus leaves `idx_topo` empty. A matching rebuild does not need the
engine binary.

## Why this identity

On an EON / kinetic ART catalogue, a saddle search that finds a
configuration whose topology key already exists is a revisited state up
to symmetry. See the architecture note on topology keys, and Trochet et
al., Phys. Rev. B 91, 224106 (2015).
