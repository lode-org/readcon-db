# Fortran API

Module: `fortran/ReadConDb/src/readcon_db.f90` (`bind(C)` to `rkrdb_*`).

Link `libreadcon_db` from a checkout (`cargo build --release`) or from
the prebuilt `readcon-db-clib-$VERSION-$target.tar.gz` Release asset
(no local cargo). See [install](install.md) and `fortran/README.md`.

```fortran
use readcon_db
integer(c_size_t) :: id
integer(c_int) :: status
integer(c_int32_t) :: n
call db_open("/tmp/corpus"//c_null_char, id, status)
call db_append(id, 1_c_int64_t, "run.con", n, status)
call db_append_units(id, 1_c_int64_t, "run.con", &
  '{"length":"A","energy":"ev"}', n, status)
call db_extend_units(id, 1_c_int64_t, "more.con", &
  '{"length":"angstrom","energy":"eV"}', n, status)
call db_select_basic(id, 1_c_int64_t, "Cu", 1, 100000, 0, status)
```

See helpers `db_open`, `db_append`, `db_select_basic`, `db_result_count`, `db_result_key`, `db_frame_hash`, `db_xxh3_128` in the module source. Point your build system at `include/` for the C header if needed and `target/release/` (or the clib prefix `lib/` + `PKG_CONFIG_PATH`). Campaign shard/drain/join is the CLI; a Fortran rank that owns one shard opens `root/shard_XXXX` with `db_open` ([campaign ops](campaign.md)).

## MPI: pack on root, Bcast on the caller INTEGER communicator

Fortran MPI communicators are `INTEGER`. Pass the handle the host already
has (LAMMPS world / sub-comm). `MPI_Bcast` on that integer; do not
substitute `MPI_COMM_WORLD` unless that is the handle the host passed.
Do not `MPI_Init` if the host already did (`MPI_Initialized`).

```fortran
use mpi
use readcon_db
! comm is the INTEGER the host owns (LAMMPS world / a Dup)
if (rank == 0) then
  call db_open_readonly(corpus, id, status)
  call db_pack_frame(id, traj, frame, buf, buflen, nbytes, status)
  call db_close(id, status)
end if
call MPI_Bcast(nbytes, 1, MPI_INTEGER, 0, comm, ierr)
call MPI_Bcast(buf, nbytes, MPI_BYTE, 0, comm, ierr)
call db_unpack_positions(buf, int(nbytes, c_size_t), xyz, cap, natoms, status)
```

C helpers from Fortran INTEGER: `rkrdb_bcast_packed_frame_f` and
`rkrdb_bcast_packed_frames_f` in `include/readcon-db-mpi.h`
(`MPI_Comm_f2c`). Batched helper packs an RCSB envelope on root and
`MPI_Bcast`s it on `comm`. Module `db_pack_frame` packs one RCSO blob (`buflen=0` size-queries).
`db_pack_frames` packs an RCSB envelope of many frames (`buflen=0`
size-queries). Unpack with `db_unpack_batch_nframes` and
`db_unpack_batch_item`. Standalone: `examples/mpi_bcast_frame.f90`.

H5MD interchange and caller units: `db_append_units`, `db_set_units`,
`db_h5md_times`, `db_h5md_shape`, `db_h5md_positions` (dest `ps` / Å).
C symbols are `rkrdb_append_trajectory_units`,
`rkrdb_extend_trajectory_units`, `rkrdb_set_units`, `rkrdb_frame_units`,
`rkrdb_h5md_times`, `rkrdb_h5md_shape`, `rkrdb_h5md_positions`,
`rkrdb_h5md_edges`, `rkrdb_h5md_forces`, `rkrdb_h5md_velocities`,
`rkrdb_h5md_species`. Fortran also has `db_extend`, `db_extend_units`, `db_frame_units`,
`db_h5md_edges`, `db_h5md_forces`, `db_h5md_velocities`, `db_h5md_species`,
`db_get_positions`, `db_get_forces`, `db_get_velocities`, `db_cook_frame`,
`db_delete_cooked`, `db_has_valid_cooked`, and `db_recook_all`. Status
constants include `rkrdb_ok`, `rkrdb_err`, and `rkrdb_not_found`.
CON line-2 `units` is the authority.

## Cooked SoA (RCSO)

See `docs/orgmode/cooked-soa.org`. Tier is opt-in; CON text remains authority. Fortran exposes cook / delete / has-valid / `db_get_positions` / `db_get_forces` / `db_get_velocities`.
