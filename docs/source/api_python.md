# Python API

Build the extension (from a checkout next to `readcon-core`):

```bash
cd python
maturin develop --features python
```

```python
from readcon_db import ConCorpus

db = ConCorpus("/tmp/corpus")
db.append_trajectory(1, "run.con")
keys = db.select(
    traj_id=1,
    symbol="Cu",
    natoms_min=1,
    natoms_max=10_000,
    energy_min=-50.0,
    energy_max=0.0,
    require_forces=True,
)
h = db.frame_hash(1, 0)          # bytes(16)
assert db.find_by_hash(h) == (1, 0)
text = db.get_frame_text(1, 0)
raw = ConCorpus.xxh3_128(b"blob")
```

Optional `select` kwargs: `exact_hash=` (16-byte LE xxh3-128), `energy_min` /
`energy_max`, `require_forces`, `require_velocities`, `require_energy`, `limit`.

## MPI: pack on root, Bcast on the caller communicator

`bcast_packed_frame(comm, corpus_dir, traj_id, frame_idx, root=0)` takes
an **mpi4py** `Comm` — LAMMPS `lmp.world`, `COMM_WORLD.Split(...)`, a
`Dup`. mpi4py already called `MPI_Init`; the helper never does, and
never names the process-wide world handle.

```python
from mpi4py import MPI
from readcon_db import ConCorpus, bcast_packed_frame, bcast_packed_frames

# Host-owned comm. A LAMMPS Python fix passes lmp.world (or a split).
comm = MPI.COMM_WORLD.Dup()
blob = bcast_packed_frame(comm, "/scratch/corpus", traj_id=1, frame_idx=0)
xyz = ConCorpus.unpack_positions(blob)
comm.Free()
```

Many frames, one collective:

```python
from readcon_db import ConCorpus, bcast_packed_frames
blob = bcast_packed_frames(comm, "/scratch/corpus", [(1, 0), (1, 1), (1, 2)])
frames = ConCorpus.unpack_batch(blob)
```

Cooked H5MD 1.1 interchange (h5py; fixed `natoms`; CON stays authority):

```python
db.export_h5md(traj_id=1, path="traj.h5")
```

The file has `/h5md` version `[1,1]` with `author`/`creator`,
`particles/all/position/value` of shape `[T][N][3]`,
`position/step` and `position/time` of shape `[T]`,
`box/edges/value` of shape `[T][3][3]`, integer-Z `species`,
optional `particles/all/velocity` `[T][N][3]` (dest `Angstrom ps-1`),
and unit attributes for one engine system (like metatomic model vs
engine): length `Angstrom`, time `ps`, force `kJ mol-1 Angstrom-1`,
velocity `Angstrom ps-1`.
CON `units.length` / `units.time` / `units.energy` convert through
`unit_conversion_factor` (SI). Time is CON `header.time()`, or
`i * timestep`, else the frame index, all in `ps`.
`unit_conversion_factor(from, to)` and `canonicalize_unit(expr)` are
on the Python module. Callers write aliases (`A`, `ev`, `femtosecond`);
`append_trajectory(..., units={...})`, `extend_trajectory(..., units={...})`,
`ingest_directory(..., units={...})`, and `set_units(traj_id, {...})`
store canonical names (`angstrom`, `eV`, `fs`) in CON metadata. `append` / `extend` stamp the incoming numbers;
`set_units` converts stored numbers so the new label is honest.
`get_units(traj_id, frame_idx)` returns the stored units JSON as a string.
Mixed-force trajectories write a full `[T][N][3]` force dataset
(zeros on frames without forces). Box `boundary` follows CON `pbc`
(periodic when absent). Time is CON `header.time()`, or `i * timestep`,
else the frame index. Missing `units.time` is CON default `fs` (then
converted to dest `ps`). `author`/`creator`/`boundary` attrs are
fixed-length ASCII. Physical `unit` attrs are short H5MD strings
(MDAnalysis 2.10 indexes them as dict keys).

Standalone: `examples/mpi_bcast_frame.py`.

## Cooked SoA (RCSO)

See `docs/orgmode/cooked-soa.org`. Tier is opt-in; CON text remains authority. Bindings expose cook / delete / has-valid / positions (and forces on C/Python/Rust).
