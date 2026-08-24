# Developer workflow and release

## Tools

- [cocogitto](https://github.com/cocogitto/cocogitto) (`cog`) — conventional commits and `CHANGELOG.md`
- [prek](https://prek.j178.dev) — git hooks (`prek.toml`)
- [lychee](https://github.com/lycheeverse/lychee) — link check on built Sphinx HTML
- Sphinx (`pixi.toml` `[feature.docs]`, locked by `pixi.lock`) — `pixi r -e docs docbld`

```bash
prek install
prek run -a
cog check
pixi r -e docs docbld
lychee --config lychee.toml 'docs/_build/html/**/*.html'
```

Refresh the documentation lock after changing `pixi.toml` `[feature.docs]` or `docs/requirements.txt`:

```bash
pixi lock
```

## Continuous integration

| Workflow | File | Trigger | Purpose |
|----------|------|---------|---------|
| Prek | `ci_prek.yml` | push, PR | `prek run -a` |
| Documentation | `ci_docs.yml` | push, PR | Locked Sphinx HTML (`pixi.lock`) + lychee |
| Pages | `pages.yml` | push to main | Deploy site + locked Sphinx docs |
| Lint | `lint.yml` | PR | Conventional commits + large-file audit |
| C/C++ dist | `ci_cxx.yml` | push, PR | CMake/Meson/pkg-config without cbindgen |
| crates.io | `crates_publish.yml` | `v*` tag | `cargo publish --locked` |
| Python wheels | `python-wheels.yml` | `v*` tag, PR | maturin matrix → PyPI |
| cxx tarball | `cxx_tarball.yml` | GitHub Release + `workflow_dispatch` tag | slim + vendor C/C++ source tarballs |
| clib tarball | `c_lib_tarball.yml` | GitHub Release + `workflow_dispatch` tag | Attach prebuilt C ABI (`readcon-db-clib-$VER-$target`) |

## Release

```text
  scripts/release-prep.sh X.Y.Z
        │  prek, sphinx + lychee, version bump, cog CHANGELOG, cxx-dist gate
        ▼
  commit: maint: bump to vX.Y.Z   ──►  open Pull Request to main
        ▼
  merge PR to main  ──►  git tag -s vX.Y.Z && git push origin vX.Y.Z
        │
        ├─► workflow "Publish to crates.io": cargo publish --locked
        ├─► workflow "Python wheels": maturin matrix → PyPI
        ├─► workflow "cxx source tarball": after a GitHub Release exists
        └─► workflow "C ABI library tarball": prebuilt clib (or dispatch tag later)
```

```bash
prek run -a
pixi r -e docs docbld
lychee --config lychee.toml 'docs/_build/html/**/*.html'
scripts/release-prep.sh X.Y.Z
git commit -m "maint: bump to vX.Y.Z"
# open PR, merge, then:
git checkout main && git pull
git tag -s vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

Do not hand-edit the generated `CHANGELOG.md` section. Extend `cog.toml`
`[commit_types]` if a historical type blocks `cog changelog`. Push only the
version tag (`git push origin vX.Y.Z`), not every local tag.

Attach prebuilt C ABI to an existing tag: Actions → **C ABI library
tarball** → run from a branch that has `scripts/package-clib.sh` → set
`tag` to `vX.Y.Z`. The packager comes from the workflow ref; sources
and the Release come from `inputs.tag`. The cxx source tarball
workflow accepts the same `tag` input.

The CPC manuscript is the readcon-core paper; this crate is the companion
campaign store. Appendix timings, if used, are the freeze under
`paper/cpc/freeze/`. Check the generated table without re-running the
campaign:

```bash
python paper/cpc/scripts/gen_fair_table.py --check
```
