# C and C++ API

Header: [`include/readcon-db.h`](https://github.com/lode-org/readcon-db/blob/main/include/readcon-db.h).

```c
#include "readcon-db.h"

size_t id;
rkrdb_open("/tmp/corpus", &id);
uint32_t n;
rkrdb_append_trajectory(id, 1, "run.con", &n);
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
```

Status: `RKRDB_OK` (0), `RKRDB_ERR` (-1), `RKRDB_NOT_FOUND` (-2), `RKRDB_NULL` (-3).

Link `libreadcon_db` from `cargo build --release` (`cdylib` / `staticlib`).

C++ RAII:

```cpp
#include "readcon-db.h"
readcon_db::Corpus db("/tmp/corpus");
db.append_trajectory(1, "run.con");
db.select_basic(1, "Cu", 1, 100000, 0);
db.select_meta(-1, "Cu", 1, 100000, -50.0, 0.0, 1, 1u, 0);
```

## Cooked SoA (RCSO)

See `docs/orgmode/cooked-soa.org`. Tier is opt-in; CON text remains authority. Bindings expose cook / delete / has-valid / positions (and forces on C/Python/Rust).
