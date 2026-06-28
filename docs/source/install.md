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

Sibling checkout next to `readcon-core` (optional **path** dep for LODE):

```bash
git clone https://github.com/lode-org/readcon-core
git clone https://github.com/lode-org/readcon-db
cd readcon-db
cargo test --locked
cargo build --release   # libreadcon_db + CLI readcon-db
```

Python extension from a checkout:

```bash
cd readcon-db/python
maturin develop --release --features python
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
