"""Bisect corpora are reused only for the commit that wrote them."""

from __future__ import annotations

import pytest

pytest.importorskip("readcon")
pytest.importorskip("readcon_db")

import bisect_raw_full as run


def test_corpus_needs_a_matching_writer_stamp(tmp_path) -> None:
    dest = tmp_path / "750b197" / "corpus"
    dest.mkdir(parents=True)
    (dest / "data.mdb").write_bytes(b"")
    sha = "7" * 40
    assert not run.corpus_ready(dest, sha)
    (dest.parent / "corpus.commit").write_text("a" * 40 + "\n")
    assert not run.corpus_ready(dest, sha)
    (dest.parent / "corpus.commit").write_text(sha + "\n")
    assert run.corpus_ready(dest, sha)
