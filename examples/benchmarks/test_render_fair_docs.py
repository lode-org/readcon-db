"""Generated fair prose only says what the stored medians support."""

from __future__ import annotations

import render_fair_docs as docs


def test_large_inserts_are_reported_as_ase_wins() -> None:
    data = docs.load(docs.DEFAULT_RESULTS)
    text = docs.readme_block(data, "fair-db29") + docs.fairness_block(data, "fair-db29")
    assert "ASE is faster on the 10^4-atom, 10^5-atom, and 10^6-atom inserts" in text
    assert "ahead on every insert" not in text
    assert "0.152 s" not in text
    assert "1.22 s" not in text
