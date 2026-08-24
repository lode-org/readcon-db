# C and C++ API

Header: [`include/readcon-db.h`](https://github.com/lode-org/readcon-db/blob/main/include/readcon-db.h).

```c
#include "readcon-db.h"

size_t id;
rkrdb_open("/tmp/corpus", &id);
uint32_t n;
rkrdb_append_trajectory(id, 1, "run.con", &n);
rkrdb_append_trajectory_units(id, 1, "run.con",
    "{\"length\":\"A\",\"energy\":\"ev\"}", &n);
rkrdb_extend_trajectory_units(id, 1, "more.con",
    "{\"length\":\"A\",\"energy\":\"ev\"}", &n);
rkrdb_set_units(id, 1, "{\"length\":\"nm\",\"energy\":\"eV\"}", &n);
char ubuf[256];
rkrdb_frame_units(id, 1, 0, ubuf, sizeof ubuf);
double t[64]; uint32_t nt = 0;
rkrdb_h5md_times(id, 1, t, 64, &nt);
uint32_t nf = 0, na = 0;
rkrdb_h5md_shape(id, 1, &nf, &na);
double xyz[4096];
rkrdb_h5md_positions(id, 1, xyz, 4096, &nf, &na);
double edges[64], frc[4096], vel[4096];
rkrdb_h5md_edges(id, 1, edges, 64);
rkrdb_h5md_forces(id, 1, frc, 4096);
rkrdb_h5md_velocities(id, 1, vel, 4096);
int32_t z[256]; uint32_t nz = 0;
rkrdb_h5md_species(id, 1, z, 256, &nz);
uint32_t npos = 0, nfrc = 0, nvel = 0; uint8_t has_f = 0, has_v = 0;
rkrdb_get_positions(id, 1, 0, xyz, 256, &npos);
rkrdb_get_forces(id, 1, 0, frc, 256, &nfrc, &has_f);
rkrdb_get_velocities(id, 1, 0, vel, 256, &nvel, &has_v);
rkrdb_cook_frame(id, 1, 0);
rkrdb_recook_all(id);
rkrdb_select_basic(id, 1, "Cu", 1, 100000, 0);
/* Metadata filters: flags bit0=forces, bit1=velocities, bit2=energy present */
rkrdb_select_meta(id, /*traj*/ -1, "Cu", 1, 100000,
                  /*energy*/ -50.0, 0.0, /*use_energy_range*/ 1,
                  /*flags*/ 1u /* forces */, /*limit*/ 0);
int m = rkrdb_result_count(id);
uint64_t traj; uint32_t frame;
rkrdb_result_key(id, 0, &traj, &frame);
uint8_t hash[16];
rkrdb_frame_hash(id, traj, frame, hash);
rkrdb_select_hash(id, hash);
rkrdb_close(id);
rkrdb_open_readonly("/tmp/corpus", &id);
rkrdb_close(id);
```

Status: `RKRDB_OK` (0), `RKRDB_ERR` (-1), `RKRDB_NOT_FOUND` (-2), `RKRDB_NULL` (-3).

Link `libreadcon_db` from `cargo build --release` (`cdylib` / `staticlib`).

C++ RAII:

```cpp
#include "readcon-db.h"
readcon_db::Corpus db("/tmp/corpus");
readcon_db::Corpus ro("/tmp/corpus", /*readonly=*/true);
db.append_trajectory(1, "run.con", "{\"length\":\"A\",\"energy\":\"ev\"}");
db.extend_trajectory(1, "more.con", "{\"length\":\"A\",\"energy\":\"ev\"}");
db.set_units(1, "{\"length\":\"nm\",\"energy\":\"eV\"}");
char ubuf[256];
db.frame_units(1, 0, ubuf, sizeof ubuf);
uint32_t nf = 0, na = 0;
db.h5md_shape(1, &nf, &na);
double t[64];
db.h5md_times(1, t, 64);
double xyz[4096], edges[64], frc[4096], vel[4096];
db.h5md_positions(1, xyz, 4096, &nf, &na);
db.h5md_edges(1, edges, 64);
db.h5md_forces(1, frc, 4096);
db.h5md_velocities(1, vel, 4096);
int32_t z[256];
db.h5md_species(1, z, 256);
bool has_f = false, has_v = false;
db.get_positions(1, 0, xyz, 256);
db.get_forces(1, 0, frc, 256, &has_f);
db.get_velocities(1, 0, vel, 256, &has_v);
db.cook_frame(1, 0);
db.recook_all();
db.select_basic(1, "Cu", 1, 100000, 0);
db.select_meta(-1, "Cu", 1, 100000, -50.0, 0.0, 1, 1u, 0);
```

## MPI: pack on root, Bcast on the caller communicator

Optional header [`include/readcon-db-mpi.h`](https://github.com/lode-org/readcon-db/blob/main/include/readcon-db-mpi.h)
(not linked into `libreadcon_db`). Pass the communicator the host already
uses. LAMMPS pair/fix code passes `lmp->world` or a sub-comm; a plugin
must not call `MPI_Init` if the host already did.

```c
#include "readcon-db-mpi.h"   /* needs <mpi.h> */

uint8_t *buf = NULL;
int nbytes = 0;
/* comm is lmp->world, a Dup, a Split — not substituted by this helper */
int st = rkrdb_bcast_packed_frame(comm, /*root*/ 0, "/scratch/corpus",
                                  /*traj*/ 1, /*frame*/ 0, &buf, &nbytes);
uint32_t natoms = 0;
double xyz[3 * 4096];
rkrdb_unpack_positions(buf, (size_t)nbytes, xyz, 4096, &natoms);
free(buf);
```

Many frames, one Bcast:

```c
int st = rkrdb_bcast_packed_frames(comm, 0, dir, trajs, frames, nkeys, &buf, &nbytes);
uint32_t nfr = 0;
rkrdb_unpack_batch_nframes(buf, (size_t)nbytes, &nfr);
uint32_t natoms = 0;
double xyz[3 * 4096];
rkrdb_unpack_batch_item(buf, (size_t)nbytes, 0, xyz, 4096, &natoms);
free(buf);
```

C++ `readcon_db::Corpus::pack_frames` / `unpack_batch_nframes` / `unpack_batch_item`.
Fortran INTEGER comms: `rkrdb_bcast_packed_frame_f` and
`rkrdb_bcast_packed_frames_f` (`MPI_Comm_f2c` of the `MPI_Fint`).
Standalone driver: `examples/mpi_bcast_frame.c` (`MPI_Initialized` +
`MPI_Comm_dup`).

## Cooked SoA (RCSO)

See `docs/orgmode/cooked-soa.org`. Tier is opt-in; CON text remains authority. Bindings expose cook / delete / has-valid / positions / forces / velocities.
