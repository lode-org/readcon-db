"""Generated fair prose only says what the stored medians support."""

from __future__ import annotations

import pytest
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


def test_committed_docs_state_the_computed_insert_winner() -> None:
    data = docs.load(docs.DEFAULT_RESULTS)
    text = docs.readme_block(data, "fair-db29") + docs.fairness_block(data, "fair-db29")
    assert docs.insert_claim(data["fair"]) in text
    assert "ahead on every insert" not in text
    assert "0.152 s" not in text
    assert "1.22 s" not in text


def test_mixed_winners_are_not_summarized_as_one() -> None:
    rows = [
        _row("atoms-1000", "full_read", "warm", "readcon-db", 0.1),
        _row("atoms-1000", "full_read", "warm", "ase.db", 0.2),
        _row("atoms-10000", "full_read", "warm", "readcon-db", 0.3),
        _row("atoms-10000", "full_read", "warm", "ase.db", 0.2),
    ]
    specs = [
        ("atoms-1000", "a", "full_read", "warm"),
        ("atoms-10000", "b", "full_read", "warm"),
    ]
    text = docs.scope_claim(rows, specs, "these rows")
    assert text.startswith("The rows differ on these rows")
    assert "readcon-db is faster" in text
    assert "ASE is faster" in text


def test_sample_count_below_twelve_is_refused(tmp_path, monkeypatch) -> None:
    bench = tmp_path / "examples" / "benchmarks"
    bench.mkdir(parents=True)
    (bench / "fair_ase_protocol.py").write_text(
        'import os\nN_SAMPLES = int(os.environ.get("READCON_DB_N_SAMPLES", "6"))\n'
    )
    monkeypatch.setattr(docs, "REPO", tmp_path)
    with pytest.raises(SystemExit, match="below 12"):
        docs.sample_count()


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


def test_committed_docs_match_the_renderer(tmp_path, monkeypatch) -> None:
    rels = (
        "README.md",
        "examples/benchmarks/FAIRNESS.md",
        "examples/benchmarks/fair_ase_protocol.py",
        "docs/orgmode/howto/workflows.org",
        "docs/orgmode/explanation/overview.org",
        "bench/db8/noise.json",
    )
    for rel in rels:
        dest = tmp_path / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes((docs.REPO / rel).read_bytes())
    repo = docs.REPO
    monkeypatch.setattr(docs, "REPO", tmp_path)
    docs.render(docs.DEFAULT_RESULTS)
    for rel in rels:
        assert (tmp_path / rel).read_bytes() == (repo / rel).read_bytes(), rel
