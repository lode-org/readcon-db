"""Fill the fair-doc markers from a results directory.

Numbers come from ``campaign.json``. A sentence is written only when the
medians support it. ASE is faster on the 10^4, 10^5, and 10^6 atom inserts
in fair-db29, so those rows are not described as a readcon-db win.
"""

from __future__ import annotations

import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DEFAULT_RESULTS = REPO / "examples" / "benchmarks" / "results" / "fair-db29"

LADDER = (
    ("atoms-1000", "10^3 atoms, 1 frame"),
    ("atoms-10000", "10^4 atoms, 1 frame"),
    ("atoms-100000", "10^5 atoms, 1 frame"),
    ("atoms-1000000", "10^6 atoms, 1 frame"),
    ("frames-1", "8 atoms, 1 frame"),
    ("frames-100", "8 atoms, 100 frames"),
    ("frames-10000", "8 atoms, 10^4 frames"),
)
INSERTS = (
    ("atoms-10000", "10^4 atoms, 1 frame"),
    ("atoms-100000", "10^5 atoms, 1 frame"),
    ("atoms-1000000", "10^6 atoms, 1 frame"),
)


def fmt(value: float) -> str:
    return f"{value:.4e}"


def load(results: Path) -> dict:
    return json.loads((results / "campaign.json").read_text())


def pick(rows: list[dict], workload: str, op: str, cache: str, tool: str) -> dict:
    matches = [
        row
        for row in rows
        if row["workload"] == workload
        and row["op"] == op
        and row["cache"] == cache
        and row["tool"] == tool
        and row.get("rayon_num_threads", "1") in ("1", "unset")
    ]
    # Prefer the paired one-thread row when both exist.
    pinned = [row for row in matches if row.get("rayon_num_threads") == "1"]
    chosen = pinned or matches
    if len(chosen) != 1:
        raise SystemExit(f"{workload} {op} {cache} {tool}: {len(chosen)} rows")
    return chosen[0]


def faster(left: dict, right: dict) -> str:
    if left["median_s"] < right["median_s"]:
        return "readcon-db"
    if right["median_s"] < left["median_s"]:
        return "ASE"
    return "tie"


def cell(row: dict) -> str:
    return f"{fmt(row['median_s'])} (n={row['n']})"


def comparison_table(
    rows: list[dict], specs: tuple[tuple[str, str, str, str], ...]
) -> list[str]:
    lines = [
        "| workload | op | cache | readcon-db | ASE | faster |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for workload, label, op, cache in specs:
        db = pick(rows, workload, op, cache, "readcon-db")
        ase = pick(rows, workload, op, cache, "ase.db")
        lines.append(
            f"| {label} | {op} | {cache} | {cell(db)} | {cell(ase)} | {faster(db, ase)} |"
        )
    return lines


def insert_claim(rows: list[dict]) -> str:
    winners = []
    for workload, label in INSERTS:
        db = pick(rows, workload, "insert", "cold-file", "readcon-db")
        ase = pick(rows, workload, "insert", "cold-file", "ase.db")
        winners.append((label, faster(db, ase), db, ase))
    if any(name != "ASE" for _label, name, _db, _ase in winners):
        raise SystemExit(f"expected ASE to win the large inserts, got {winners}")
    bits = []
    for label, _name, db, ase in winners:
        bits.append(
            f"{label}: readcon-db {fmt(db['median_s'])} (n={db['n']}), "
            f"ASE {fmt(ase['median_s'])} (n={ase['n']})"
        )
    return (
        "ASE is faster on the 10^4-atom, 10^5-atom, and 10^6-atom inserts "
        "(" + "; ".join(bits) + ")."
    )


def fairness_block(data: dict, results_name: str) -> str:
    rows = data["fair"]
    paper = data["local_paper_harness"]
    reads = []
    for workload, label in LADDER:
        for op, cache in (
            ("full_read", "warm"),
            ("full_read", "cold"),
            ("selected", "cold"),
        ):
            reads.append((workload, label, op, cache))
    lines = [
        (
            "Warm full read, cold full read, and cold single-frame read from "
            f"`results/{results_name}/campaign.json`. Each cell is the type-7 median. "
            "n is the sample count stored for that row."
        ),
        "",
        *comparison_table(rows, reads),
        "",
        insert_claim(rows),
        "readcon-db is faster on the read rows in this table and on the other insert rows.",
        "",
        "## This machine",
        "",
        (
            f"Regenerated from `results/{results_name}/`. "
            f"Commit `{data['head']}`. The host line in the log is the neutral label."
        ),
        "",
        (
            "The published-harness rerun in that directory, one warmup discarded, is "
            f"readcon-db {fmt(paper['readcon-db']['median_s'])} "
            f"(IQR {fmt(paper['readcon-db']['iqr_s'])}, n={paper['readcon-db']['n']}) and "
            f"ASE {fmt(paper['ase.db']['median_s'])} "
            f"(IQR {fmt(paper['ase.db']['iqr_s'])}, n={paper['ase.db']['n']})."
        ),
        "",
        (
            "The fair 40×8 insert, one commit each, is "
            f"readcon-db {fmt(pick(rows, 'paper40x8', 'insert', 'cold-file', 'readcon-db')['median_s'])} and "
            f"ASE {fmt(pick(rows, 'paper40x8', 'insert', 'cold-file', 'ase.db')['median_s'])}."
        ),
        "",
    ]
    return "\n".join(lines)


def readme_block(data: dict, results_name: str) -> str:
    rows = data["fair"]
    specs = []
    for workload, label in LADDER:
        if workload in ("frames-1", "frames-100"):
            continue
        for op, cache, op_label in (
            ("full_read", "warm", "full read"),
            ("full_read", "cold", "full read"),
        ):
            specs.append((workload, label, op, cache, op_label))
        if workload == "atoms-1000000":
            specs.append((workload, label, "selected", "cold", "one frame"))
        if workload == "frames-10000":
            specs.append((workload, label, "selected", "cold", "one frame"))
            specs.append((workload, label, "insert", "cold-file", "insert"))
        if workload == "atoms-1000000":
            specs.append((workload, label, "insert", "cold-file", "insert"))
    lines = [
        "Type-7 medians from `examples/benchmarks/results/"
        + results_name
        + "/campaign.json`, in seconds. n is the stored sample count.",
        "",
        "| workload | op | cache | readcon-db | ASE sqlite | faster |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for workload, label, op, cache, op_label in specs:
        db = pick(rows, workload, op, cache, "readcon-db")
        ase = pick(rows, workload, op, cache, "ase.db")
        lines.append(
            f"| {label} | {op_label} | {cache} | {cell(db)} | {cell(ase)} | {faster(db, ase)} |"
        )
    lines += [
        "",
        insert_claim(rows),
        "readcon-db is faster on every full read and single-frame query in this table.",
        (
            "The full ladder is "
            f"[`examples/benchmarks/results/{results_name}/RESULTS.md`]"
            f"(examples/benchmarks/results/{results_name}/RESULTS.md)."
        ),
        "",
    ]
    return "\n".join(lines)


def overview_block(results_name: str) -> str:
    return (
        f"*Benchmarks:* the fair ASE sqlite comparison is "
        f"=examples/benchmarks/fair_ase_protocol.py= "
        f"(protocol in =examples/benchmarks/FAIRNESS.md=, numbers generated from "
        f"=examples/benchmarks/results/{results_name}/=). "
        "Select-count parity remains =examples/benchmarks/fair_campaign.py=. "
        "See @@rst::doc:`/howto/workflows`@@."
    )


def splice(path: Path, name: str, body: str, kind: str) -> None:
    if kind == "md":
        begin = f"<!-- fair-docs:begin {name} -->"
        end = f"<!-- fair-docs:end {name} -->"
    else:
        begin = f"# fair-docs:begin {name}"
        end = f"# fair-docs:end {name}"
    text = path.read_text()
    if begin not in text or end not in text:
        raise SystemExit(f"{path} is missing markers for {name}")
    pre, rest = text.split(begin, 1)
    _old, post = rest.split(end, 1)
    path.write_text(pre + begin + "\n" + body.rstrip() + "\n" + end + post)


def render(results: Path = DEFAULT_RESULTS) -> None:
    data = load(results)
    name = results.name
    splice(
        REPO / "examples" / "benchmarks" / "FAIRNESS.md",
        "fairness-tables",
        fairness_block(data, name),
        "md",
    )
    splice(REPO / "README.md", "readme-fair", readme_block(data, name), "md")
    splice(
        REPO / "docs" / "orgmode" / "howto" / "workflows.org",
        "workflows-summary",
        readme_block(data, name),
        "org",
    )
    splice(
        REPO / "docs" / "orgmode" / "explanation" / "overview.org",
        "overview-bench",
        overview_block(name),
        "org",
    )


def main() -> None:
    render()


if __name__ == "__main__":
    main()
