# Developer workflow and release

## Tools

- [cocogitto](https://github.com/cocogitto/cocogitto) (`cog`) — conventional commits and `CHANGELOG.md`
- [prek](https://prek.j178.dev) — git hooks (`prek.toml`)
- [lychee](https://github.com/lycheeverse/lychee) — link check on built Sphinx HTML
- Sphinx (`docs/requirements.txt`) — `sphinx-build -b html docs/source docs/_build/html`

```bash
prek install
prek run -a
cog check
pip install -r docs/requirements.txt
sphinx-build -b html docs/source docs/_build/html
lychee --config lychee.toml 'docs/_build/html/**/*.html'
```

## Continuous integration

| Workflow | File | Trigger | Purpose |
|----------|------|---------|---------|
| Prek | `ci_prek.yml` | push, PR | `prek run -a` |
| Documentation | `ci_docs.yml` | push, PR | Sphinx HTML + lychee |
| Pages | `pages.yml` | push to main | Deploy site + Sphinx docs |
| Lint | `lint.yml` | PR | Conventional commits + large-file audit |
| C/C++ dist | `ci_cxx.yml` | push, PR | CMake/Meson/pkg-config without cbindgen |
| crates.io | `crates_publish.yml` | `v*` tag | `cargo publish --locked` |
| Python wheels | `python-wheels.yml` | `v*` tag, PR | maturin matrix → PyPI |
| cxx tarball | `cxx_tarball.yml` | GitHub Release | slim + vendor C/C++ source tarballs |

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
        └─► workflow "cxx source tarball": after a GitHub Release exists
```

```bash
prek run -a
sphinx-build -b html docs/source docs/_build/html
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
