#!/usr/bin/env python3
"""Figures from the evaluation CSVs. Type 42 fonts so the PDFs stay editable."""

from __future__ import annotations

import csv
import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
matplotlib.rcParams["pdf.fonttype"] = 42
matplotlib.rcParams["ps.fonttype"] = 42
matplotlib.rcParams["font.size"] = 10

import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parents[2]
RESULTS = ROOT / "eval" / "results"
FIG = RESULTS / "figures"

COLORS = {
    "readcon-db": "#0072B2",
    "ase.db": "#E69F00",
    "con-directory": "#009E73",
}
STORES = [
    ("raw_con", "CON text"),
    ("gzip", "gzip"),
    ("zstd", "zstd"),
    ("readcon-db", "readcon-db"),
    ("ase.db", "ase.db"),
    ("con_directory", "CON directory"),
]
STORE_COLORS = ["#4D4D4D", "#999999", "#BDBDBD", "#0072B2", "#E69F00", "#009E73"]


def read_csv(name: str) -> list[dict]:
    path = RESULTS / name
    if not path.is_file():
        return []
    with path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def ok(row) -> bool:
    return str(row.get("ok", "")).lower() == "true"


def summary_lookup(rows):
    out = {}
    for row in rows:
        if row["median"] == "":
            continue
        key = (row["kind"], int(row["n_frames"]), row["system"], row["query"], row["cache"])
        out[key] = row
    return out


def err_from(row):
    med = float(row["median"])
    lo = float(row["minimum"])
    hi = float(row["maximum"])
    return med, max(med - lo, 0.0), max(hi - med, 0.0)


def series(summary, kind, system, query="", cache=""):
    xs, ys, lo, hi = [], [], [], []
    frames = sorted(
        {
            key[1]
            for key in summary
            if key[0] == kind and key[2] == system and key[3] == query and key[4] == cache
        }
    )
    for n in frames:
        row = summary[(kind, n, system, query, cache)]
        if int(row["n"]) == 0:
            continue
        med, elo, ehi = err_from(row)
        xs.append(n)
        ys.append(med)
        lo.append(elo)
        hi.append(ehi)
    return np.array(xs), np.array(ys), np.array(lo), np.array(hi)


def style_log_ax(ax, ylabel):
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("frames")
    ax.set_ylabel(ylabel)
    ax.grid(True, which="both", linewidth=0.3, alpha=0.5)


def plot_rate(summary, kind, ylabel, filename):
    fig, ax = plt.subplots(figsize=(5.2, 3.6))
    for system in ("readcon-db", "ase.db"):
        xs, ys, lo, hi = series(summary, kind, system)
        if xs.size == 0:
            continue
        ax.errorbar(
            xs,
            ys,
            yerr=np.vstack([lo, hi]),
            marker="o",
            color=COLORS[system],
            label=system,
            capsize=3,
            linewidth=1.2,
        )
    style_log_ax(ax, ylabel)
    ax.legend(frameon=False)
    fig.tight_layout()
    fig.savefig(FIG / filename)
    plt.close(fig)


def plot_queries(summary):
    fig, axes = plt.subplots(2, 3, figsize=(9.2, 5.6), sharex=True)
    for col, query in enumerate(("composition", "energy", "fmax")):
        for row, cache in enumerate(("cold", "warm")):
            ax = axes[row, col]
            for system in ("readcon-db", "ase.db", "con-directory"):
                xs, ys, lo, hi = series(summary, "query_s", system, query, cache)
                if xs.size == 0:
                    continue
                ax.errorbar(
                    xs,
                    ys,
                    yerr=np.vstack([lo, hi]),
                    marker="o",
                    color=COLORS[system],
                    label=system,
                    capsize=2,
                    linewidth=1.1,
                )
            ax.set_title(f"{query}, {cache}")
            style_log_ax(ax, "seconds" if col == 0 else "")
    handles, labels = axes[0, 0].get_legend_handles_labels()
    fig.legend(handles, labels, loc="upper center", ncol=3, frameon=False, bbox_to_anchor=(0.5, 1.02))
    fig.tight_layout()
    fig.savefig(FIG / "query_latency.pdf", bbox_inches="tight")
    plt.close(fig)


def plot_disk(size_rows):
    frames = sorted({int(row["n_frames"]) for row in size_rows})
    if not frames:
        return
    fig, ax = plt.subplots(figsize=(7.2, 3.8))
    x = np.arange(len(frames))
    width = 0.12
    for i, (store, label) in enumerate(STORES):
        heights = []
        for n in frames:
            match = [row for row in size_rows if int(row["n_frames"]) == n and row["store"] == store]
            heights.append(float(match[0]["allocated_bytes"]) if match else np.nan)
        ax.bar(x + (i - 2.5) * width, heights, width, label=label, color=STORE_COLORS[i])
    ax.set_xticks(x)
    ax.set_xticklabels([f"{n:.0e}" for n in frames])
    ax.set_yscale("log")
    ax.set_ylabel("allocated bytes")
    ax.set_xlabel("frames")
    ax.legend(frameon=False, ncol=3, fontsize=8)
    ax.grid(True, axis="y", which="both", linewidth=0.3, alpha=0.5)
    fig.tight_layout()
    fig.savefig(FIG / "disk_footprint.pdf")
    plt.close(fig)


def plot_dedup():
    path = RESULTS / "dedup.json"
    if not path.is_file():
        return
    data = json.loads(path.read_text())
    if not data.get("ok"):
        return
    fig, axes = plt.subplots(1, 2, figsize=(6.4, 3.4))
    axes[0].bar(
        ["stored frames", "unique hashes"],
        [data["n_frames"], data["n_unique_hashes"]],
        color=["#0072B2", "#56B4E9"],
    )
    axes[0].set_ylabel("frames")
    axes[1].bar(
        ["full corpus", "unique texts"],
        [data["corpus_allocated_bytes"], data["unique_corpus_allocated_bytes"]],
        color=["#0072B2", "#56B4E9"],
    )
    axes[1].set_ylabel("allocated bytes")
    for ax in axes:
        ax.grid(True, axis="y", linewidth=0.3, alpha=0.5)
    fig.tight_layout()
    fig.savefig(FIG / "dedup.pdf")
    plt.close(fig)


def main():
    FIG.mkdir(parents=True, exist_ok=True)
    summary = summary_lookup(read_csv("summary.csv"))
    plot_rate(summary, "frames_per_s", "frames / s", "ingest_throughput.pdf")
    plot_rate(summary, "mb_per_s", "MB / s", "ingest_bandwidth.pdf")
    plot_queries(summary)
    plot_disk(read_csv("sizes.csv"))
    plot_dedup()


if __name__ == "__main__":
    main()
