# Fortran API

Module: `fortran/ReadConDb/src/readcon_db.f90` (`bind(C)` to `rkrdb_*`).

1. `cargo build --release` in the crate root.
2. Link `libreadcon_db` and use the module:

```fortran
use readcon_db
integer(c_size_t) :: id
integer(c_int) :: status
integer(c_int32_t) :: n
call db_open("/tmp/corpus"//c_null_char, id, status)
call db_append(id, 1_c_int64_t, "run.con", n, status)
call db_select_basic(id, 1_c_int64_t, "Cu", 1, 100000, 0, status)
```

See helpers `db_open`, `db_append`, `db_select_basic`, `db_result_count`, `db_result_key`, `db_frame_hash`, `db_xxh3_128` in the module source. Point your build system at `include/` for the C header if needed and `target/release/`.

## Cooked SoA (RCSO)

See `docs/orgmode/cooked-soa.org`. Tier is opt-in; CON text remains authority. Bindings expose cook / delete / has-valid / positions (and forces on C/Python/Rust).
