# readcon-db

**Mmap-backed CON/convel corpus store** — LMDB via Heed, non-SQL selection, xxHash3 exact match, and bindings for **Rust, C, C++, Python, and Fortran**.

Part of the **readcon ecosystem** with [readcon-core](https://github.com/lode-org/readcon-core) ([core Sphinx](https://lode-org.github.io/readcon-core/)): **core** owns format fidelity and multi-language interchange; **db** owns corpus scale (many trajectories, selective access, OS page-cache residency).

```{toctree}
:maxdepth: 2
:caption: Contents

overview
architecture
api_rust
api_c
api_python
api_fortran
install
workflows
changelog_link
```

## At a glance

| Need | Use |
|------|-----|
| Parse/write one `.con` / stream | [readcon-core](https://github.com/lode-org/readcon-core) |
| Thousands of frames; filter by symbol / \(N\) / **energy** / **forces·velocities** / exact content | **`readcon-db`** |
| SQL | Not provided (by design) |

```bash
cargo add readcon-db   # when published; path dep: LODE/readcon-db
cargo test -p readcon-db
```

```{admonition} Logo
The logo (teal tile, page stack, B-tree, hash spark) is under `assets/logo/` — SVG for docs and the project website.
```
