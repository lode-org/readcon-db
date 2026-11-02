#!/usr/bin/env python3
"""Fail when a copy of the version disagrees with Cargo.toml.

Each copy is read with a parser for its format: tomllib for TOML, ast for
the Sphinx conf.py and Meson's own parser for meson.build. A reformatted
line, a second version key or a pre-release tag outside the sync pattern
therefore fails here even when scripts/sync-version.py leaves the file
alone. The check also requires the exact readcon-core pin.

Python 3.11 or newer, standard library only. meson.build is read when the
mesonbuild package is importable, and the Lint workflow installs it.
"""

import ast
import subprocess
import sys
import tomllib
from pathlib import Path

from crate_version import crate_version

try:
    from mesonbuild import mparser
except ImportError:
    mparser = None

ROOT = Path(__file__).resolve().parent.parent
# The published writer whose CON and JSON round trips the tests rely on.
READCON_CORE_PIN = "=0.17.1"


def read(root, rel):
    return (root / rel).read_text(encoding="utf-8")


def load_toml(root, rel):
    return tomllib.loads(read(root, rel))


def load_cff(root, rel):
    """Top-level scalar keys of a CFF file, with a repeated key rejected."""
    keys = {}
    for number, line in enumerate(read(root, rel).splitlines(), 1):
        if not line.strip() or line[0] in " \t#-":
            continue
        key, colon, value = line.partition(":")
        key = key.strip().strip("'\"")
        if not colon:
            raise ValueError(f"line {number} is not a key")
        if key in keys:
            raise ValueError(f"line {number} repeats the {key!r} key")
        value = value.split(" #")[0].strip()
        if len(value) > 1 and value[0] == value[-1] and value[0] in "'\"":
            value = value[1:-1]
        keys[key] = value
    return keys


def cargo_version(root):
    return load_toml(root, "Cargo.toml")["package"]["version"]


def helper_version(root):
    try:
        return crate_version(root / "Cargo.toml")
    except SystemExit as err:
        raise ValueError(str(err)) from None


def cargo_toml(root, rel, version):
    pin = load_toml(root, rel)["dependencies"]["readcon-core"]
    yield "readcon-core pin", pin, READCON_CORE_PIN


def helper(root, rel, version):
    yield "version it reads from Cargo.toml", helper_version(root), version


def cargo_lock(root, rel, version):
    entries = [p for p in load_toml(root, rel)["package"] if p["name"] == "readcon-db"]
    if len(entries) != 1:
        raise ValueError(f"{len(entries)} readcon-db entries, expected 1")
    yield "readcon-db version", entries[0]["version"], version


def toml_key(*keys):
    def check(root, rel, version):
        value = load_toml(root, rel)
        for key in keys:
            value = value[key]
        yield ".".join(keys), value, version

    return check


def pyproject(root, rel, version):
    project = load_toml(root, rel)["project"]
    yield "project.version", project.get("version"), None
    yield "version in project.dynamic", "version" in project["dynamic"], True


def cff(root, rel, version):
    yield "version", load_cff(root, rel)["version"], version


def sphinx_conf(root, rel, version):
    assigned = {}
    for node in ast.parse(read(root, rel), rel).body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name):
                    assigned[target.id] = node.value

    def evaluate(node):
        if isinstance(node, ast.Constant):
            return node.value
        if isinstance(node, ast.Name) and node.id in assigned:
            return evaluate(assigned[node.id])
        if ast.unparse(node) == "crate_version()":
            return helper_version(root)
        raise ValueError(f"cannot evaluate {ast.unparse(node)}")

    if "release" not in assigned:
        raise ValueError("no release assignment")
    for name in ("release", "version"):
        if name in assigned:
            yield name, evaluate(assigned[name]), version


def meson_build(root, rel, version):
    project = mparser.Parser(read(root, rel), rel).parse().lines[0]
    if project.func_name.value != "project":
        raise ValueError("the first statement is not project()")
    kwargs = {key.value: value for key, value in project.args.kwargs.items()}
    node = kwargs["version"]
    if isinstance(node, mparser.StringNode):
        yield "project() version", node.value, version
        return
    methods = []
    while isinstance(node, mparser.MethodNode):
        methods.append(node.name.value)
        node = node.source_object
    args = node.args.arguments
    call = (node.func_name.value, args[0].value if args else None, methods)
    wanted = ("run_command", "scripts/crate_version.py", ["strip", "stdout"])
    yield "project() version expression", call, wanted
    out = subprocess.run(
        [sys.executable, str(root / wanted[1])],
        capture_output=True,
        check=True,
        cwd=root,
        text=True,
    )
    yield "project() version", out.stdout.strip(), version


def sources(root):
    """(file, check) for every copy of the version under root."""
    found = [
        ("Cargo.toml", cargo_toml),
        ("scripts/crate_version.py", helper),
        ("Cargo.lock", cargo_lock),
        ("pixi.toml", toml_key("workspace", "version")),
        ("fortran/ReadConDb/fpm.toml", toml_key("version")),
        ("python/pyproject.toml", pyproject),
        ("CITATION.cff", cff),
        ("docs/source/conf.py", sphinx_conf),
    ]
    if mparser is not None:
        found.append(("meson.build", meson_build))
    return found


def check(root=ROOT):
    """One message per copy under root that disagrees with Cargo.toml."""
    try:
        version = cargo_version(root)
    except (OSError, LookupError, ValueError) as err:
        return [f"Cargo.toml: cannot read [package].version: {err!r}"]
    errors = []
    for rel, read_copies in sources(root):
        try:
            for what, found, wanted in read_copies(root, rel, version):
                if found != wanted:
                    errors.append(f"{rel}: {what} is {found!r}, expected {wanted!r}")
        except (OSError, LookupError, TypeError, ValueError, SyntaxError) as err:
            errors.append(f"{rel}: cannot read: {err!r}")
        except subprocess.CalledProcessError as err:
            errors.append(f"{rel}: {err.cmd} failed: {err.stderr.strip()}")
    return errors


def main():
    if mparser is None:
        print("skip meson.build: mesonbuild is not importable", file=sys.stderr)
    errors = check()
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    if errors:
        print("check_version_lockstep: FAILED", file=sys.stderr)
        return 1
    print(f"check_version_lockstep: every copy matches {cargo_version(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
