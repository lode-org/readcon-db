"""Generated fair prose only says what the stored medians support."""

from __future__ import annotations

import render_fair_docs as docs


def _row(workload: str, op: str, cache: str, tool: str, median: float) -> dict:
    return {
        "workload": workload,
        "op": op,
        "cache": cache,
        "tool": tool,
        "median_s": median,
        "n": 12,
        "rayon_num_threads": "1",
    }


def test_large_inserts_are_reported_as_ase_wins() -> None:
    data = docs.load(docs.DEFAULT_RESULTS)
    text = docs.readme_block(data, "fair-db29") + docs.fairness_block(data, "fair-db29")
    assert "ASE is faster on the 10^4-atom, 10^5-atom, and 10^6-atom inserts" in text
    assert "ahead on every insert" not in text
    assert "0.152 s" not in text
    assert "1.22 s" not in text


def test_insert_claim_follows_the_medians() -> None:
    rows = []
    for workload, _label in docs.INSERTS:
        rows.append(_row(workload, "insert", "cold-file", "readcon-db", 0.1))
        rows.append(_row(workload, "insert", "cold-file", "ase.db", 0.2))
    text = docs.insert_claim(rows)
    assert "readcon-db is faster" in text
    assert "ASE is faster" not in text


def test_sample_sentences_use_the_protocol_count(monkeypatch) -> None:
    monkeypatch.setattr(docs, "sample_count", lambda: 12)
    assert docs.fairness_n_sentence() == (
        "Samples are serial, the two tools alternate, n = 12."
    )
    assert docs.readme_n_sentence() == "Twelve serial samples alternate the two tools."
