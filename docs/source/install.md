# Install

## Rust (crates.io)

```bash
cargo add readcon-db
# library + CLI binary from the same crate:
cargo install readcon-db --locked
```

Requires [readcon-core](https://crates.io/crates/readcon-core) **^0.14** (pulled automatically).

## Python (PyPI)

```bash
pip install readcon-db
python -c "import readcon_db"
```

Install [readcon](https://pypi.org/project/readcon/) (core) as well when you need CON parse/write outside the corpus API.

## From source (developers)

```bash
git clone https://github.com/lode-org/readcon-db
cd readcon-db
cargo test --locked
cargo build --release   # libreadcon_db + CLI readcon-db
```

`readcon-core` comes from crates.io (`^0.14`). To develop against a local core tree, add an **untracked** `.cargo/config.toml`:

```toml
[patch.crates-io]
readcon-core = { path = "../readcon-core" }
```

Python extension from a checkout:

```bash
pip install maturin
maturin develop --release --features python --manifest-path python/pyproject.toml
```

C / C++: build with `cargo build --release` and use `include/readcon-db.h`.  
Fortran: `fortran/ReadConDb` (`bind(C)` against the C ABI).

## Documentation

- User site: <https://lode-org.github.io/readcon-db/>
- Rust API: <https://docs.rs/readcon-db>
- Design notes: [`docs/design.md`](https://github.com/lode-org/readcon-db/blob/main/docs/design.md) in the repository

```bash
cd docs
pip install -r requirements.txt
sphinx-build -b html source _build/html
```

## Static marketing page

Open `website/index.html` (or serve `website/`). Logos under `assets/logo/`.
