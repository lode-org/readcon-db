# How-to: annotate bonded topology

xxHash3 on stored CON bytes is exact identity. The bonded graph up to
relabelling is a coarser identity: two minima that differ by a permutation or
a small relaxation share a topology key when the bonds match.

d-SEAMS writes that key (`seams fingerprint FILE --format json`). The neighbour
cutoff is per-corpus. Default graph is `cutoff` and hops is 2.

## Annotate a corpus

```bash
readcon-db ingest /tmp/corpus run.con
readcon-db annotate-topology /tmp/corpus --cutoff 3.0
# optional: --graph cutoff --hops 2 --seams /path/to/seams
```

`--seams`, then `SEAMS`, then `PATH` locate the binary. A missing
`seams fingerprint` exits nonzero and names that command on stderr.

Every annotated trajectory records the same cutoff, graph, hop count, and method
(`nauty` or `wl`). The command refuses a later run with a different method.

## Select and look up

```bash
readcon-db select /tmp/corpus --topo-key abcdef
readcon-db find-by-topology /tmp/corpus permuted.con
```

`find-by-topology` runs the fingerprint on the file with the recorded corpus
parameters and returns matching frame keys. A permuted copy of an annotated
frame hits the original. A frame with a broken bond does not.

## Rust

```rust
use readcon_db::{AnnotateTopologyOpts, ConCorpus, Select};

let db = ConCorpus::open("/tmp/corpus")?;
db.append_trajectory_path(1, "run.con")?;
db.annotate_topology(&AnnotateTopologyOpts::new(3.0))?;
let keys = db.select(&Select::new().topo_key("abcdef"))?;
let hits = db.find_by_topology_path("permuted.con")?;
```

## Reindex

`readcon-db reindex` rebuilds the existing secondary indexes from CON blobs.
`idx_topo` is rebuilt from stored `topo_by_frame` keys when every
annotated trajectory records the same parameters. Disagreeing methods or
cutoffs return an error without a partial topology rebuild. Unannotated
corpora leave `idx_topo` empty. The rebuild does not call `seams`.
