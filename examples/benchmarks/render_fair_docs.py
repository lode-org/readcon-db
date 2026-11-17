"""Fill the fair-doc markers from committed records.

Numbers come from ``campaign.json``, ``bench/db8/noise.json``, and the
fair protocol's sample count. Who is faster is the comparison of the
stored medians. A sentence those records do not support is not written.
"""

from __future__ import annotations

import ast
import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DEFAULT_RESULTS = REPO / "examples" / "benchmarks" / "fair-db30"

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
LABELS = dict(LADDER)
LABELS["paper40x8"] = "40×8"


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


def pair(
    rows: list[dict], workload: str, op: str, cache: str
) -> tuple[dict, dict, str]:
    db = pick(rows, workload, op, cache, "readcon-db")
    ase = pick(rows, workload, op, cache, "ase.db")
    return db, ase, faster(db, ase)


def detail(label: str, db: dict, ase: dict) -> str:
    return (
        f"{label}: readcon-db {fmt(db['median_s'])} (n={db['n']}), "
        f"ASE {fmt(ase['median_s'])} (n={ase['n']})"
    )


def insert_claim(rows: list[dict]) -> str:
    """Who is faster on the three large atom inserts, from the medians."""
    found = []
    for workload, label in INSERTS:
        db, ase, who = pair(rows, workload, "insert", "cold-file")
        found.append((label, who, db, ase))
    names = {who for _label, who, _db, _ase in found}
    bits = "; ".join(detail(label, db, ase) for label, _who, db, ase in found)
    if names == {"ASE"}:
        return (
            "ASE is faster on the 10^4-atom, 10^5-atom, and 10^6-atom inserts "
            f"({bits})."
        )
    if names == {"readcon-db"}:
        return (
            "readcon-db is faster on the 10^4-atom, 10^5-atom, "
            f"and 10^6-atom inserts ({bits})."
        )
    if names == {"tie"}:
        return f"The 10^4-atom, 10^5-atom, and 10^6-atom inserts tie ({bits})."
    pieces = [
        f"{detail(label, db, ase)}, {who} is faster" for label, who, db, ase in found
    ]
    return "The large inserts do not share a winner (" + "; ".join(pieces) + ")."


def other_inserts(rows: list[dict]) -> list[tuple[str, str, str, str]]:
    """Insert rows outside the three large atom inserts."""
    large = {workload for workload, _label in INSERTS}
    seen: list[tuple[str, str]] = []
    for row in rows:
        if row["op"] != "insert" or row["tool"] != "readcon-db":
            continue
        if row["workload"] in large:
            continue
        key = (row["workload"], row["cache"])
        if key not in seen:
            seen.append(key)
    return [
        (workload, LABELS.get(workload, workload), "insert", cache)
        for workload, cache in seen
    ]


def scope_claim(
    rows: list[dict], specs: list[tuple[str, str, str, str]], scope: str
) -> str:
    found = []
    for workload, label, op, cache in specs:
        db, ase, who = pair(rows, workload, op, cache)
        found.append((f"{label} {op} {cache}", who, db, ase))
    names = {who for _label, who, _db, _ase in found}
    if len(names) == 1:
        who = names.pop()
        if who == "tie":
            return f"The rows tie on {scope}."
        return f"{who} is faster on {scope}."
    bits = "; ".join(
        f"{detail(label, db, ase)}, {who} is faster" for label, who, db, ase in found
    )
    return f"The rows differ on {scope} ({bits})."


def read_and_other_claim(
    rows: list[dict], reads: list[tuple[str, str, str, str]]
) -> str:
    read_specs = [(workload, label, op, cache) for workload, label, op, cache in reads]
    others = other_inserts(rows)
    read_text = scope_claim(rows, read_specs, "the read rows in this table")
    other_text = scope_claim(rows, others, "the other insert rows")
    if (
        read_text == "readcon-db is faster on the read rows in this table."
        and other_text == "readcon-db is faster on the other insert rows."
    ):
        return (
            "readcon-db is faster on the read rows in this table "
            "and on the other insert rows."
        )
    return read_text + " " + other_text


def query_claim(rows: list[dict], specs: list[tuple[str, str, str, str, str]]) -> str:
    queries = [
        (workload, label, op, cache)
        for workload, label, op, cache, _op_label in specs
        if op != "insert"
    ]
    return scope_claim(
        rows, queries, "every full read and single-frame query in this table"
    )


def sample_count() -> int:
    """The fair protocol's default n. Below 12 is refused at import."""
    path = REPO / "examples" / "benchmarks" / "fair_ase_protocol.py"
    tree = ast.parse(path.read_text())
    for node in tree.body:
        if not isinstance(node, ast.Assign):
            continue
        if not any(
            isinstance(t, ast.Name) and t.id == "N_SAMPLES" for t in node.targets
        ):
            continue
        call = node.value
        if not isinstance(call, ast.Call) or not call.args:
            break
        get = call.args[0]
        if not isinstance(get, ast.Call) or len(get.args) < 2:
            break
        n = int(ast.literal_eval(get.args[1]))
        if n < 12:
            raise SystemExit(f"fair protocol n is {n}, below 12")
        return n
    raise SystemExit("N_SAMPLES default is not in fair_ase_protocol.py")


def fairness_n_sentence() -> str:
    return f"Samples are serial, the two tools alternate, n = {sample_count()}."


def n_word() -> str:
    n = sample_count()
    return {12: "Twelve"}.get(n, str(n))


def readme_n_sentence() -> str:
    return f"{n_word()} serial samples alternate the two tools."


def protocol_n_sentence() -> str:
    return f"{n_word()} samples are kept for every row."


def cpc_block(data: dict, results_name: str) -> str:
    published = data["published_db_v2"]
    db = published["readcon-db"]
    ase = published["ase.db"]
    samples = ", ".join(f"{fmt(value)} s" for value in ase["samples_s"])
    return (
        "The published CPC samples in "
        f"`{results_name}/campaign.json` (`published_db_v2`, "
        "40 frames of 8 Cu) give readcon-db a type-7 median "
        f"**{fmt(db['median_s'])} s** (n={db['n']}). ASE's {ase['n']} samples "
        f"are {samples}. Type-7 median **{fmt(ase['median_s'])} s**, "
        f"Q1 {fmt(ase['q1_s'])} s, Q3 {fmt(ase['q3_s'])} s, "
        f"IQR **{fmt(ase['iqr_s'])} s**."
    )


def readme_cpc(data: dict) -> str:
    published = data["published_db_v2"]
    db = published["readcon-db"]
    ase = published["ase.db"]
    return (
        "The published CPC samples in `published_db_v2` "
        f"(40 frames of 8 Cu, n={db['n']}) are readcon-db "
        f"{fmt(db['median_s'])} s and ASE {fmt(ase['median_s'])} s "
        f"(IQR {fmt(ase['iqr_s'])} s)."
    )


def db8_block() -> str:
    path = REPO / "bench" / "db8" / "noise.json"
    data = json.loads(path.read_text())
    return (
        "The db8 frames-100 selected cold row, open outside the clock, is "
        f"n={data['n']}, median {fmt(data['median_s'])} s, p90 of those means "
        f"{fmt(data['p90_of_sample_means_s'])} s, "
        f"IQR/median {data['iqr_over_median']:.6g}. "
        "Its per-call distribution is `bench/db8/noise.json`."
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
            f"`{results_name}/campaign.json`. Each cell is the type-7 median. "
            "n is the sample count stored for that row."
        ),
        "",
        *comparison_table(rows, reads),
        "",
        insert_claim(rows),
        read_and_other_claim(rows, reads),
        "",
        "## This machine",
        "",
        (
            f"Regenerated from `{results_name}/`. "
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
        "Type-7 medians from `examples/benchmarks/"
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
        query_claim(rows, specs),
        (
            "The full ladder is "
            f"[`examples/benchmarks/{results_name}/RESULTS.md`]"
            f"(examples/benchmarks/{results_name}/RESULTS.md)."
        ),
        "",
    ]
    return "\n".join(lines)


def overview_block(results_name: str) -> str:
    return (
        f"*Benchmarks:* the fair ASE sqlite comparison is "
        f"=examples/benchmarks/fair_ase_protocol.py= "
        f"(protocol in =examples/benchmarks/FAIRNESS.md=, numbers generated from "
        f"=examples/benchmarks/{results_name}/=). "
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
        "fairness-db8",
        db8_block(),
        "md",
    )
    splice(
        REPO / "examples" / "benchmarks" / "FAIRNESS.md",
        "fairness-cpc",
        cpc_block(data, name),
        "md",
    )
    splice(
        REPO / "examples" / "benchmarks" / "FAIRNESS.md",
        "fairness-n",
        fairness_n_sentence(),
        "md",
    )
    splice(
        REPO / "examples" / "benchmarks" / "FAIRNESS.md",
        "fairness-n-protocol",
        protocol_n_sentence(),
        "md",
    )
    splice(
        REPO / "docs" / "orgmode" / "howto" / "workflows.org",
        "workflows-n",
        readme_n_sentence(),
        "org",
    )
    splice(
        REPO / "README.md",
        "readme-n",
        readme_n_sentence(),
        "md",
    )
    splice(
        REPO / "README.md",
        "readme-cpc",
        readme_cpc(data),
        "md",
    )
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
