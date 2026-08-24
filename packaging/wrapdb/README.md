# wrapdb submission for readcon-db

Wrap name is `readcon-db`, matching the pkg-config file and
`meson.override_dependency('readcon-db', ...)`.

The wrap points at the cxx source tarball (`readcon-db-cxx-$VERSION.tar.gz`).
Upstream `meson.build` is the wrap; no `packagefiles/` overlay.

```bash
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
SHA=$(cut -d' ' -f1 /tmp/cxx/readcon-db-cxx-${VERSION}.tar.gz.sha256)
sed -e "s/@VERSION@/${VERSION}/g" -e "s/@SHA256@/${SHA}/g" \
  packaging/wrapdb/readcon-db.wrap.in > packaging/wrapdb/readcon-db.wrap
```

Attach the tarball to the GitHub Release for `v$VERSION`.

The prebuilt C ABI (`readcon-db-clib-$VERSION-$target.tar.gz`) is a
separate asset (`scripts/package-clib.sh`). Meson wrap still uses the
cxx *source* tarball.
