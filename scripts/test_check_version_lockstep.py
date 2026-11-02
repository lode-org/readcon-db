#!/usr/bin/env python3
"""Drift that the sync patterns leave alone still fails the parser check.

Each case edits a copy of the tracked files the check reads. When cogapp is
importable, each case also runs scripts/sync-version.py on that copy and
asserts that the edit survives, so `git diff --exit-code` alone would pass it.
"""

import contextlib
import importlib.util
import io
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_version_lockstep as lockstep

try:
    import cogapp  # noqa: F401
except ImportError:
    sync = None
else:
    spec = importlib.util.spec_from_file_location(
        "sync_version", lockstep.ROOT / "scripts" / "sync-version.py"
    )
    sync = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sync)


class Drift(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        for rel, _ in lockstep.sources(lockstep.ROOT):
            (self.root / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(lockstep.ROOT / rel, self.root / rel)
        self.v = lockstep.cargo_version(self.root)

    def text(self, rel):
        return (self.root / rel).read_text(encoding="utf-8")

    def edit(self, rel, old, new):
        text = self.text(rel)
        self.assertIn(old, text)
        (self.root / rel).write_text(text.replace(old, new, 1), encoding="utf-8")

    def assertFails(self, rel):
        if sync is not None:
            before = self.text(rel)
            sync.ROOT = self.root
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(sync.main(), 0)
            self.assertEqual(self.text(rel), before, "the sync rewrote the edit")
        errors = lockstep.check(self.root)
        self.assertTrue(any(e.startswith(f"{rel}: ") for e in errors), errors)

    def test_clean_copy_passes(self):
        self.assertEqual(lockstep.check(self.root), [])

    def test_reformatted_cargo_lock(self):
        entry = f'name = "readcon-db"\nversion = "{self.v}"'
        self.edit("Cargo.lock", entry, "name = 'readcon-db'\nversion = '9.9.9'")
        self.assertFails("Cargo.lock")

    def test_literal_release_in_sphinx_conf(self):
        rel = "docs/source/conf.py"
        self.edit(rel, "release = crate_version()", 'release = "9.9.9"')
        self.assertFails(rel)

    def test_second_version_key_in_pixi(self):
        self.edit("pixi.toml", "# [[[end]]]\n", '# [[[end]]]\nversion = "8.8.8"\n')
        self.assertFails("pixi.toml")

    def test_second_version_key_in_citation(self):
        self.edit("CITATION.cff", "license: MIT\n", "license: MIT\nversion: 8.8.8\n")
        self.assertFails("CITATION.cff")

    def test_off_allowlist_tag_in_cargo_lock(self):
        entry = f'name = "readcon-db"\nversion = "{self.v}"'
        self.edit("Cargo.lock", entry, f'{entry[:-1]}+local"')
        self.assertFails("Cargo.lock")


if __name__ == "__main__":
    unittest.main()
