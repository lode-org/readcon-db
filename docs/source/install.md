# Install

## From source (developers)

```bash
git clone https://github.com/lode-org/readcon-db   # when published
# or use LODE checkout beside readcon-core
cd readcon-db
cargo test
cargo build --release
cargo doc --no-deps --open
```

`readcon-core` is a **path dependency** (`../readcon-core`) until both crates are on crates.io.

## Documentation site

```bash
cd docs
# pip install sphinx furo myst-parser
sphinx-build -b html source _build/html
```

## Static marketing page

Open `website/index.html` (or serve the `website/` directory). Assets live under `website/assets/` and `assets/logo/`.
