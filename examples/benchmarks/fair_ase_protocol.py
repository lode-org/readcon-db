#!/usr/bin/env python3
"""Fair readcon-db vs ASE sqlite comparison.

The CPC ingest in readcon-core ``paper/cpc/bench/db_v2.py`` (log
``paper/cpc/logs/db.log``) times 40 frames of 8 Cu atoms. readcon-db's
six samples have type-7 median 0.00467 s. ASE's six samples run from
0.054 s to 0.104 s (type-7 median 0.0723 s, IQR 0.0377 s).

That ASE spread is the harness. Each ASE ``write`` commits and fsyncs,
so a sample of 40 frames is 40 fsyncs, while readcon-db commits once.
The six samples alternate which store runs first, with no discarded
warmup, and ASE parses the CON file inside the timer. The CON file
stays in the page cache; the sqlite file is a new inode only after the
child exits. fsync latency then tracks the other store's writeback.

This script keeps every sample. Each sample unlinks and creates the
database file, both stores use one write transaction and one commit
fsync, parsing stays outside the timer, and one warmup is discarded.
A long readcon-db trajectory still puts one 64-frame group at a time
and drops that group's cooked blob and decompressed text after the
put. Those puts share the one transaction, so a failed insert leaves
no frame visible. Reads are a cold series, and on frames-100 also a
cold-open series, then a warm series. Cold advises ``DONTNEED`` and
checks ``mincore`` is 0, then opens outside the clock and times the
query. cold-open does the same advice outside the clock and times the
open plus the query, for both tools. A sample that stays resident is
labeled and left out of the cold median. n = 6, serial, type-7 median
and IQR. Every stored field is checked; a mismatch aborts.
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.util
import gc
import hashlib
import json
import math
import os
import platform
import shutil
import socket
import sqlite3
import subprocess
import sys
import time
from contextlib import contextmanager
from pathlib import Path

import numpy as np
import readcon
from ase import Atoms
from ase.calculators.singlepoint import SinglePointCalculator
from ase.db import connect
from ase.db.sqlite import SQLite3Database
from readcon_db import ConCorpus

REPO = Path(__file__).resolve().parents[2]
WARMUP = 1
N_SAMPLES = int(os.environ.get("READCON_DB_N_SAMPLES", "6"))
# A row whose first samples are still wide keeps going until the ratio
# falls or n reaches EXTEND_N. The published wide row is n=12.
EXTEND_AT = 0.05
EXTEND_N = max(12, N_SAMPLES)
# A single sub-millisecond fsync or query is mostly timer noise. Repeat it
# until the timed spans cover this long, and publish the mean as one sample.
# Reads repeat until the timed spans cover this. A 50 ms mean is enough
# once open, advice, and the field check are outside the clock.
SAMPLE_S = float(os.environ.get("READCON_DB_READ_SPAN_S", "0.050"))
# Commit fsyncs are correlated (lag-1 about 0.37 on a one-frame insert).
# A 50 ms or 250 ms mean of those commits still has IQR/median above 0.15.
# Means of about 2000 commits, roughly two seconds, fall under that line.
INSERT_SPAN_S = float(os.environ.get("READCON_DB_INSERT_SPAN_S", "2.0"))
PUBLISHED_READCON = [
    0.006240564998734044,
    0.005746310998802073,
    0.003665125997940777,
    0.00556730500102276,
    0.0037774779993924312,
    0.0033583500007807743,
]
PUBLISHED_ASE = [
    0.10351818699928117,
    0.08124185300039244,
    0.10350268600086565,
    0.0591945019987179,
    0.06339448499784339,
    0.05423903599876212,
]

_LIBC = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
_LIBC.posix_fadvise.argtypes = [
    ctypes.c_int,
    ctypes.c_int64,
    ctypes.c_int64,
    ctypes.c_int,
]
_LIBC.posix_fadvise.restype = ctypes.c_int
_LIBC.mmap.argtypes = [
    ctypes.c_void_p,
    ctypes.c_size_t,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_int64,
]
_LIBC.mmap.restype = ctypes.c_void_p
_LIBC.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
_LIBC.munmap.restype = ctypes.c_int
_LIBC.mincore.argtypes = [
    ctypes.c_void_p,
    ctypes.c_size_t,
    ctypes.POINTER(ctypes.c_ubyte),
]
_LIBC.mincore.restype = ctypes.c_int
_POSIX_FADV_DONTNEED = 4
_PROT_READ = 1
_MAP_PRIVATE = 2
_PAGE = os.sysconf("SC_PAGE_SIZE")
_COLD_TRIES = 8


def type7(values: list[float], p: float) -> float:
    """Hyndman-Fan type-7 quantile. NumPy ``method='linear'``."""
    arr = np.sort(np.asarray(values, dtype=np.float64))
    n = int(arr.size)
    if n == 0:
        raise ValueError("empty sample")
    if n == 1:
        return float(arr[0])
    h = (n - 1) * p
    lo = int(np.floor(h))
    hi = int(np.ceil(h))
    w = h - lo
    return float(arr[lo] * (1.0 - w) + arr[hi] * w)


def summarize(values: list[float]) -> dict:
    q1 = type7(values, 0.25)
    mid = type7(values, 0.50)
    q3 = type7(values, 0.75)
    np_q = np.percentile(
        np.asarray(values, dtype=np.float64), [25, 50, 75], method="linear"
    )
    # One ulp between the hand-rolled type-7 and NumPy is the same quantile.
    if not np.allclose([q1, mid, q3], np_q, rtol=1e-12, atol=1e-15):
        raise AssertionError(([q1, mid, q3], np_q.tolist()))
    return {
        "n": len(values),
        "warmup": WARMUP,
        "median_s": mid,
        "q1_s": q1,
        "q3_s": q3,
        "iqr_s": q3 - q1,
        "min_s": float(min(values)),
        "max_s": float(max(values)),
        "samples_s": [float(v) for v in values],
    }


def _map_failed(addr) -> bool:
    if addr is None:
        return True
    failed = ctypes.c_void_p(-1).value
    return int(addr) == int(failed) if failed is not None else False


def resident_fraction(path: Path) -> float:
    """Fraction of pages resident, via ``mincore``. The mapping is not read."""
    size = path.stat().st_size
    if size <= 0:
        return 0.0
    pages = (size + _PAGE - 1) // _PAGE
    fd = os.open(path, os.O_RDONLY)
    try:
        addr = _LIBC.mmap(None, size, _PROT_READ, _MAP_PRIVATE, fd, 0)
        if _map_failed(addr):
            err = ctypes.get_errno()
            raise OSError(err, f"mmap failed on {path}")
        try:
            vec = (ctypes.c_ubyte * pages)()
            rc = _LIBC.mincore(ctypes.c_void_p(addr), ctypes.c_size_t(size), vec)
            if rc != 0:
                err = ctypes.get_errno()
                raise OSError(err, f"mincore failed on {path}")
            resident = sum(byte & 1 for byte in vec)
            return resident / pages
        finally:
            _LIBC.munmap(ctypes.c_void_p(addr), ctypes.c_size_t(size))
    finally:
        os.close(fd)


def advise_dontneed(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY)
    try:
        try:
            os.fsync(fd)
        except OSError:
            pass
        rc = _LIBC.posix_fadvise(fd, 0, 0, _POSIX_FADV_DONTNEED)
        if rc != 0:
            err = ctypes.get_errno()
            raise OSError(err, f"posix_fadvise DONTNEED failed on {path}")
    finally:
        os.close(fd)


def advise_cold(path: Path) -> tuple[float, str]:
    """Retry ``DONTNEED`` until ``mincore`` reports no resident page.

    Returns ``(resident_fraction, "verified")`` or the last fraction with
    ``"resident"`` when the advice cannot clear the cache.
    """
    last = resident_fraction(path)
    for _ in range(_COLD_TRIES):
        advise_dontneed(path)
        last = resident_fraction(path)
        if last == 0.0:
            return 0.0, "verified"
    return last, "resident"


def drop_page_cache(path: Path) -> None:
    advise_dontneed(path)


def fsync_dir(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def paper_con_text(n_frames: int = 40, n_atoms: int = 8) -> str:
    """The db_v2.py trajectory: one component, coordinates only."""
    lines = [""]
    for frame in range(n_frames):
        lines += [
            '{"con_spec_version":3,"con_spec_minor":1,"units":{"length":"angstrom","energy":"eV"}}',
            "10.0 10.0 10.0",
            "90.0 90.0 90.0",
            "",
            "",
            "1",
            str(n_atoms),
            "63.546",
            "Cu",
            "Coordinates of Component 1",
        ]
        for atom in range(n_atoms):
            lines.append(f"{atom + frame * 0.01:.4f} 0.1 0.2 0 {atom}")
        lines.append("")
    return "\n".join(lines)


def connect_fair(path: Path) -> tuple[SQLite3Database, dict]:
    """SQLite backend, journal_mode=DELETE, synchronous=FULL, one commit.

    DELETE and FULL are SQLite's defaults. They are set on every
    connection so the log can quote the values the connection is using.
    ``synchronous=FULL`` fsyncs on commit, which is LMDB's default
    (``MDB_NOSYNC`` is not set). The 5000-row intermediate commit inside
    ASE's ``managed_connection`` is disabled for this connection so the
    write is one transaction, matching one ``append_trajectory_str``.
    """
    commits = {"n": 0}
    db = connect(str(path), type="db")

    class _CountConn(sqlite3.Connection):
        def commit(self):
            commits["n"] += 1
            return super().commit()

    def _connect():
        con = sqlite3.connect(db.filename, timeout=20, factory=_CountConn)
        con.execute("PRAGMA journal_mode=DELETE")
        con.execute("PRAGMA synchronous=FULL")
        return con

    db._connect = _connect  # type: ignore[method-assign]
    base = SQLite3Database.managed_connection

    @contextmanager
    def managed(self, commit_frequency: int = 2**31 - 1):
        with base(self, commit_frequency=commit_frequency) as con:
            yield con

    db.managed_connection = managed.__get__(db, type(db))  # type: ignore[method-assign]
    # Schema create and its commit stay outside the timed section.
    with db.managed_connection() as _con:
        pass
    probe = sqlite3.connect(path)
    try:
        journal = probe.execute("PRAGMA journal_mode").fetchone()[0]
        sync = int(probe.execute("PRAGMA synchronous").fetchone()[0])
    finally:
        probe.close()
    sync_name = {0: "OFF", 1: "NORMAL", 2: "FULL", 3: "EXTRA"}.get(sync, str(sync))
    if journal != "delete" or sync != 2:
        raise RuntimeError(
            f"unexpected sqlite settings journal={journal} synchronous={sync}"
        )
    db._fair_commits = commits  # type: ignore[attr-defined]
    return db, {
        "backend": "sqlite",
        "sqlite_version": sqlite3.sqlite_version,
        "ase_version": __import__("ase").__version__,
        "journal_mode": journal,
        "synchronous": sync_name,
        "transactions": "one commit for the whole insert (ASE's 5000-row commit disabled)",
        "fsync": "synchronous=FULL on that commit",
    }


def reset_commits(db) -> None:
    db._fair_commits["n"] = 0


def assert_one_commit(db) -> None:
    n = db._fair_commits["n"]
    if n != 1:
        raise RuntimeError(f"ASE insert committed {n} times, expected 1")


def close_lmdb(db) -> None:
    """Drop the corpus. A process-wide ``gc.collect()`` is not required.

    CPython frees the extension object when this reference goes away, and
    that releases the LMDB environment. Collecting the whole heap from
    inside the timed close was an 8 ms floor on every cold read.
    """
    del db


def float_token(value: float) -> str:
    """Shortest round-trip spelling, including a signed zero."""
    value = float(value)
    if value == 0.0 and math.copysign(1.0, value) < 0.0:
        return "-0.0"
    return repr(value)


def synthetic_arrays(frame_index: int, n_atoms: int):
    """The shared geometry. Both tools' records come from this."""
    pos = np.zeros((n_atoms, 3), dtype=np.float64)
    pos[:, 0] = np.arange(n_atoms, dtype=np.float64) * np.float64(0.125)
    pos[:, 1] = np.float64(0.5)
    pos[:, 2] = np.float64(1.0 + frame_index)
    pos[0, 1] = np.float64(np.pi)
    if frame_index == 0:
        pos[0, 0] = np.float64(-0.0)
    if n_atoms > 1:
        pos[1, 2] = np.nextafter(np.float64(1.0), np.float64(2.0))
    forces = np.array(pos * np.float64(0.5), dtype=np.float64)
    forces[0, 0] = np.float64(-0.0)
    masses = np.full(n_atoms, np.float64(63.546), dtype=np.float64)
    energy = np.float64(-42.5) + np.float64(frame_index)
    cell = np.diag(np.array([10.5, 12.25, 8.0], dtype=np.float64))
    return pos, forces, masses, energy, cell


def record_from_arrays(pos, forces, masses, energy, cell, frame_index: int) -> dict:
    n_atoms = int(pos.shape[0])
    symbols = ["Cu"] * n_atoms
    return {
        "symbols": symbols,
        "symbol_runs": _symbol_runs(symbols),
        "positions": pos,
        "cell": cell,
        "pbc": (True, True, False),
        "masses": masses,
        "forces": forces,
        "energy": energy,
        "frame_index": frame_index,
    }


def _write_frame(buf: bytearray, frame_index: int, pos, forces, energy: float) -> None:
    if frame_index:
        buf.extend(b"\n")
    n_atoms = int(pos.shape[0])
    header = (
        '{"con_spec_minor":1,"con_spec_version":3,"energy":'
        + float_token(energy)
        + ',"frame_index":'
        + str(frame_index)
        + ',"lattice_vectors":[[10.5,0.0,0.0],[0.0,12.25,0.0],[0.0,0.0,8.0]],'
        + '"pbc":[true,true,false],"sections":["forces"],'
        + '"units":{"energy":"eV","length":"angstrom","mass":"amu",'
        + '"time":"(amu*angstrom^2/eV)^0.5"}}\n'
        + "10.5 12.25 8.0\n90.0 90.0 90.0\n\n\n1\n"
        + str(n_atoms)
        + "\n63.546\nCu\nCoordinates of Component 1\n"
    )
    buf.extend(header.encode())
    for i in range(n_atoms):
        x, y, z = pos[i]
        buf.extend(
            f"{float_token(x)} {float_token(y)} {float_token(z)} 0 {i}\n".encode()
        )
    buf.extend(b"\nCu\nForces of Component 1\n")
    for i in range(n_atoms):
        x, y, z = forces[i]
        buf.extend(
            f"{float_token(x)} {float_token(y)} {float_token(z)} 0 {i}\n".encode()
        )


def build_text_records(n_frames: int, n_atoms: int) -> tuple[str, list[dict]]:
    """CON text and the arrays it must round-trip to, without a ConFrame."""
    buf = bytearray(b"\n")
    records = []
    for i in range(n_frames):
        pos, forces, masses, energy, cell = synthetic_arrays(i, n_atoms)
        _write_frame(buf, i, pos, forces, float(energy))
        records.append(record_from_arrays(pos, forces, masses, energy, cell, i))
    return buf.decode(), records


def build_atom_records(n_frames: int, n_atoms: int) -> tuple[list[Atoms], list[dict]]:
    """ASE rows only. The CON text is not built in this process."""
    atoms_out: list[Atoms] = []
    records = []
    for i in range(n_frames):
        pos, forces, masses, energy, cell = synthetic_arrays(i, n_atoms)
        symbols = ["Cu"] * n_atoms
        atoms = Atoms(
            symbols,
            positions=pos.copy(),
            cell=cell.copy(),
            pbc=(True, True, False),
            masses=masses.copy(),
        )
        atoms.calc = SinglePointCalculator(
            atoms, energy=float(energy), forces=forces.copy()
        )
        atoms.info["frame_index"] = i
        atoms_out.append(atoms)
        records.append(record_from_arrays(pos, forces, masses, energy, cell, i))
    return atoms_out, records


def build_records(n_frames: int, n_atoms: int) -> tuple[str, list[Atoms], list[dict]]:
    """Same rows for both stores: geometry, masses, forces, energy, frame index, pbc."""
    frames = []
    atoms_out: list[Atoms] = []
    records: list[dict] = []
    cell = np.diag(np.array([10.5, 12.25, 8.0], dtype=np.float64))
    pbc = (True, True, False)
    for i in range(n_frames):
        pos = np.zeros((n_atoms, 3), dtype=np.float64)
        pos[:, 0] = np.arange(n_atoms, dtype=np.float64) * np.float64(0.125)
        pos[:, 1] = np.float64(0.5)
        pos[:, 2] = np.float64(1.0 + i)
        pos[0, 1] = np.float64(np.pi)
        if i == 0:
            pos[0, 0] = np.float64(-0.0)
        if n_atoms > 1:
            pos[1, 2] = np.nextafter(np.float64(1.0), np.float64(2.0))
        forces = np.array(pos * np.float64(0.5), dtype=np.float64)
        forces[0, 0] = np.float64(-0.0)
        masses = np.full(n_atoms, np.float64(63.546), dtype=np.float64)
        energy = np.float64(-42.5) + np.float64(i)
        symbols = ["Cu"] * n_atoms
        atoms = Atoms(
            symbols,
            positions=pos.copy(),
            cell=cell.copy(),
            pbc=pbc,
            masses=masses.copy(),
        )
        atoms.calc = SinglePointCalculator(
            atoms, energy=float(energy), forces=forces.copy()
        )
        fr = readcon.ConFrame.from_ase(atoms)
        fr.set_energy(float(energy))
        fr.set_frame_index(i)
        frames.append(fr)
        atoms_out.append(atoms)
        records.append(
            {
                "symbols": symbols,
                "symbol_runs": _symbol_runs(symbols),
                "positions": pos,
                "cell": cell.copy(),
                "pbc": pbc,
                "masses": masses,
                "forces": forces,
                "energy": energy,
                "frame_index": i,
            }
        )
    text = readcon.write_con_string(frames)
    return text, atoms_out, records


def _bits(a, b) -> bool:
    return (
        np.asarray(a, dtype=np.float64).tobytes()
        == np.asarray(b, dtype=np.float64).tobytes()
    )


def assert_con_matches(frame, rec: dict) -> None:
    got_sym = [atom.symbol for atom in frame.atoms]
    if got_sym != rec["symbols"]:
        raise AssertionError("symbols")
    if not _bits(frame.coords_array(), rec["positions"]):
        raise AssertionError("positions")
    if not _bits(frame.forces_array(), rec["forces"]):
        raise AssertionError("forces")
    if not _bits([atom.mass for atom in frame.atoms], rec["masses"]):
        raise AssertionError("masses")
    if np.float64(frame.energy).tobytes() != np.float64(rec["energy"]).tobytes():
        raise AssertionError(("energy", frame.energy, rec["energy"]))
    if int(frame.frame_index) != rec["frame_index"]:
        raise AssertionError("frame_index")
    if tuple(bool(x) for x in frame.metadata["pbc"]) != tuple(
        bool(x) for x in rec["pbc"]
    ):
        raise AssertionError("pbc")
    if not _bits(frame.cell, np.diag(rec["cell"])):
        raise AssertionError(("cell", frame.cell, np.diag(rec["cell"])))


def assert_ase_matches(row, rec: dict) -> None:
    atoms = row.toatoms()
    if atoms.get_chemical_symbols() != rec["symbols"]:
        raise AssertionError("ase symbols")
    if not _bits(atoms.positions, rec["positions"]):
        raise AssertionError("ase positions")
    if not _bits(atoms.cell.array, rec["cell"]):
        raise AssertionError("ase cell")
    if tuple(bool(x) for x in atoms.pbc) != tuple(bool(x) for x in rec["pbc"]):
        raise AssertionError("ase pbc")
    if not _bits(atoms.get_masses(), rec["masses"]):
        raise AssertionError("ase masses")
    if row.forces is None or not _bits(row.forces, rec["forces"]):
        raise AssertionError("ase forces")
    if np.float64(row.energy).tobytes() != np.float64(rec["energy"]).tobytes():
        raise AssertionError(("ase energy", row.energy, rec["energy"]))
    if int(row.key_value_pairs["frame_index"]) != rec["frame_index"]:
        raise AssertionError("ase frame_index")


def read_con_frames(db, n_frames: int):
    keys = [(1, i) for i in range(n_frames)]
    texts = db.get_frame_texts(keys)
    frames = []
    for text in texts:
        got = readcon.read_con_string(text)
        if len(got) != 1:
            raise AssertionError(len(got))
        frames.append(got[0])
    return frames


def read_ase_rows(db, n_frames: int):
    rows = list(db.select())
    rows.sort(key=lambda row: int(row.key_value_pairs["frame_index"]))
    if len(rows) != n_frames:
        raise AssertionError(len(rows))
    return rows


def validate_pair(con_frames, ase_rows, records: list[dict]) -> None:
    if len(con_frames) != len(records) or len(ase_rows) != len(records):
        raise AssertionError((len(con_frames), len(ase_rows), len(records)))
    for frame, row, rec in zip(con_frames, ase_rows, records):
        assert_con_matches(frame, rec)
        assert_ase_matches(row, rec)
        if not _bits(frame.coords_array(), row.toatoms().positions):
            raise AssertionError("cross positions")


class Log:
    def __init__(self, path: Path):
        path.parent.mkdir(parents=True, exist_ok=True)
        self.path = path
        self.fh = path.open("w", encoding="utf-8")

    def line(self, text: str) -> None:
        self.fh.write(text + "\n")
        self.fh.flush()

    def close(self) -> None:
        self.fh.close()


def git_head() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=REPO, text=True
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def header_lines() -> list[str]:
    cpu = ""
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    except OSError:
        cpu = ""
    return [
        f"rev_parse {git_head()}",
        f"date_utc {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}",
        f"host {socket.gethostname()}",
        f"uname {platform.platform()}",
        f"machine {platform.machine()}",
        f"cpu {cpu}",
        f"nproc {os.cpu_count()}",
        f"python {sys.version.split()[0]}",
        f"warmup {WARMUP}",
        f"samples {N_SAMPLES} (12 when IQR/median exceeds {EXTEND_AT})",
        "quantile type-7 (numpy percentile method=linear)",
        "order serial, tools alternate each sample index, no parallel samples",
        "timer: directory fsync, fadvise, mincore, the element-wise check, and the collector stay outside; the clock covers the commit, or the query; cold-open also times the open",
        "timer_short: a read shorter than 50 ms is repeated until the timed spans add to 50 ms; the sample is that mean",
        "timer_insert: a commit shorter than 2 s is repeated until the timed spans add to 2 s, because commit fsyncs stay correlated past 250 ms",
        "page_cache insert: unlink and create the database file outside the timer, one commit fsync inside",
        "page_cache read_cold: posix_fadvise DONTNEED, then mincore, outside the timer; retry until resident fraction is 0",
        "page_cache read_cold_label: a sample that stays resident is labeled and omitted from the cold median",
        "page_cache drop_caches: /proc/sys/vm/drop_caches is not writable here, so it is not used",
        "page_cache read_warm: one warmup read on the open store, then time the read",
        "lmdb MDB_NOSYNC unset, one write transaction and one commit fsync for the whole insert; a long trajectory puts one 64-frame group at a time and drops that group's cooked blob and decompressed text after the put",
        "ase one write transaction, journal_mode=DELETE, synchronous=FULL, one commit fsync; ASE's 5000-row commit is disabled",
        "cold: page cache dropped and mincore 0 outside the clock, then open outside the clock, then the query",
        "cold-open: page cache dropped outside the clock, then open and the query inside the clock, both tools",
        "validation element-wise outside the timer; a mismatch aborts the run",
    ]


def fresh_dir(root: Path, name: str) -> Path:
    path = root / name
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True)
    fsync_dir(root)
    fsync_dir(path)
    return path


def time_call(fn):
    t0 = time.perf_counter()
    out = fn()
    return time.perf_counter() - t0, out


def vm_hwm() -> int:
    """Peak resident set since this process started, in bytes."""
    try:
        for line in Path("/proc/self/status").read_text().splitlines():
            if line.startswith("VmHWM:"):
                return int(line.split()[1]) * 1024
    except OSError:
        return 0
    return 0


def vm_rss() -> int:
    """Current resident set, in bytes."""
    try:
        for line in Path("/proc/self/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) * 1024
    except OSError:
        return 0
    return 0


def iqr_over(values: list[float], limit: float = EXTEND_AT) -> bool:
    """True when six or more means are wider than ``limit`` and n is still 6."""
    if len(values) < N_SAMPLES or len(values) >= EXTEND_N:
        return False
    stat = summarize(values)
    med = stat["median_s"]
    if med <= 0:
        return False
    return stat["iqr_s"] / med > limit


def sample_operation(run, setup=None, teardown=None, span: float | None = None) -> dict:
    """Time ``run`` only. ``setup`` and ``teardown`` stay outside the clock.

    The variance on a sub-millisecond row was the LMDB or sqlite open, the
    directory fsync, and a single commit fsync. A collection during the
    call is the same kind of noise, so the collector is disabled for the
    clock and enabled again before teardown. A call that already covers
    ``span`` is the sample. A shorter call is repeated until the timed
    spans add up to it (reads use 50 ms, inserts use two seconds), and the
    sample is their mean. A single commit that spikes past 5 ms does not
    end the insert early: that spike stays in the mean. Both tools use this.

    Peak RSS is the kernel high-water mark. A polling thread next to a
    sub-millisecond cold read moved the sample means between runs.
    """
    total = 0.0
    count = 0
    collect = gc.isenabled()
    limit = SAMPLE_S if span is None else span
    try:
        while True:
            if setup is not None:
                setup()
            try:
                if collect:
                    gc.disable()
                t0 = time.perf_counter()
                run()
                dt = time.perf_counter() - t0
            finally:
                if collect:
                    gc.enable()
                if teardown is not None:
                    teardown()
            total += dt
            count += 1
            # One slow commit inside a stream of short commits stays in the
            # mean. The loop ends when the spans cover the budget, or when a
            # single call was already longer than that budget.
            if count >= 100_000 or total >= limit:
                break
    finally:
        if collect:
            gc.enable()
    return {
        "seconds": total / count if count else 0.0,
        "inner_n": count,
        "timed_span_s": total,
        "peak_rss_bytes": vm_hwm(),
    }


def paper_protocol(log: Log, work: Path) -> dict:
    """The published harness, on this machine: per-row ASE commit, parse inside the timer."""
    text = paper_con_text()
    src = work / "paper.con"
    src.write_text(text)
    n_frames = 40

    def one(kind: str, dest: Path) -> float:
        if dest.exists():
            shutil.rmtree(dest)
        dest.mkdir(parents=True)
        t0 = time.perf_counter()
        if kind == "readcon-db":
            db = ConCorpus(str(dest / "corpus"))
            n = db.append_trajectory_str(1, text, source="traj.con")
            frames = len(db.select(traj_id=1))
            if int(n) != n_frames or frames != n_frames:
                raise AssertionError((n, frames))
            close_lmdb(db)
        else:
            from ase.io import read as ase_read

            atoms = ase_read(src, index=":", format="eon")
            db = connect(dest / "atoms.db")
            for frame in atoms:
                db.write(frame)
            frames = db.count()
            if frames != n_frames:
                raise AssertionError(frames)
        return time.perf_counter() - t0

    log.line("protocol paper_db_v2 repeats 6 warmup 0 interleaved per-row ase commit")
    samples = {"readcon-db": [], "ase.db": []}
    kinds = ["readcon-db", "ase.db"]
    for repeat in range(N_SAMPLES):
        order = kinds if repeat % 2 == 0 else list(reversed(kinds))
        for kind in order:
            dest = work / f"paper-{kind}-{repeat}"
            seconds = one(kind, dest)
            samples[kind].append(seconds)
            log.line(
                f"sample protocol=paper workload=paper40x8 op=insert tool={kind} i={repeat} seconds={seconds}"
            )
            shutil.rmtree(dest, ignore_errors=True)
    return {kind: summarize(values) for kind, values in samples.items()}


def assert_geometry_con(frame, rec: dict) -> None:
    if [atom.symbol for atom in frame.atoms] != rec["symbols"]:
        raise AssertionError("symbols")
    if not _bits(frame.coords_array(), rec["positions"]):
        raise AssertionError("positions")
    if not _bits(frame.cell, np.diag(rec["cell"])):
        raise AssertionError("cell")


def assert_geometry_ase(row, rec: dict) -> None:
    atoms = row.toatoms()
    if atoms.get_chemical_symbols() != rec["symbols"]:
        raise AssertionError("ase symbols")
    if not _bits(atoms.positions, rec["positions"]):
        raise AssertionError("ase positions")
    if not _bits(atoms.cell.array, rec["cell"]):
        raise AssertionError("ase cell")


def check_stored(kind: str, got, rec: dict, geometry_only: bool) -> None:
    if geometry_only:
        if kind == "readcon-db":
            assert_geometry_con(got, rec)
        else:
            assert_geometry_ase(got, rec)
        return
    if kind == "readcon-db":
        assert_con_matches(got, rec)
    else:
        assert_ase_matches(got, rec)


def timed_insert(kind: str, name: str, payload: dict, work: Path) -> dict:
    """One sample: create the database outside the clock, time the commit."""
    holder: dict = {}
    disk = {"n": None}

    def setup() -> None:
        dest = fresh_dir(work, f"{name}-{kind}")
        holder["dest"] = dest
        if kind == "readcon-db":
            holder["db"] = ConCorpus(str(dest / "corpus"))
        else:
            dest.mkdir(parents=True, exist_ok=True)
            db, _settings = connect_fair(dest / "atoms.db")
            db.connection = db._connect()
            db.change_count = 0
            reset_commits(db)
            holder["db"] = db

    def run() -> None:
        db = holder["db"]
        if kind == "readcon-db":
            n = db.append_trajectory_str(1, payload["text"], source="traj.con")
            if int(n) != payload["n_frames"]:
                raise AssertionError(n)
        else:
            for atoms in payload["atoms"]:
                db.write(atoms, frame_index=int(atoms.info["frame_index"]))
            db.connection.commit()

    def teardown() -> None:
        dest = holder["dest"]
        db = holder["db"]
        if disk["n"] is None:
            disk["n"] = disk_bytes(kind, dest)
        geometry_only = bool(payload.get("geometry_only"))
        if kind == "readcon-db":
            # A second ConFrame of a million atoms, next to the builder's
            # text and atoms, was the resident set ASE did not hold. The
            # numeric arrays are the same check without that object graph.
            if geometry_only:
                frames = read_con_frames(db, payload["n_frames"])
                for frame, rec in zip(frames, payload["records"]):
                    check_stored(kind, frame, rec, True)
            else:
                check_materialized(db.read_arrays(1), payload["records"])
            close_lmdb(db)
        else:
            assert_one_commit(db)
            got = read_ase_rows(db, payload["n_frames"])
            for row, rec in zip(got, payload["records"]):
                check_stored(kind, row, rec, geometry_only)
            close_ase(db)
        holder["db"] = None
        # The LMDB env stays mapped until the last Python reference is
        # collected. That has to happen before the next open, and it is
        # outside the clock.
        if kind == "readcon-db":
            gc.collect()
        shutil.rmtree(dest, ignore_errors=True)

    stat = sample_operation(run, setup, teardown, span=INSERT_SPAN_S)
    stat["disk_bytes"] = disk["n"]
    return stat


def insert_fair(kind: str, payload: dict, dest: Path) -> float:
    geometry_only = bool(payload.get("geometry_only"))
    if kind == "readcon-db":
        corpus = dest / "corpus"
        db = ConCorpus(str(corpus))

        def run():
            n = db.append_trajectory_str(1, payload["text"], source="traj.con")
            if int(n) != payload["n_frames"]:
                raise AssertionError(n)

        seconds, _ = time_call(run)
        frames = read_con_frames(db, payload["n_frames"])
        for frame, rec in zip(frames, payload["records"]):
            check_stored(kind, frame, rec, geometry_only)
        close_lmdb(db)
        gc.collect()
        return seconds
    dest.mkdir(parents=True, exist_ok=True)
    path = dest / "atoms.db"
    db, _settings = connect_fair(path)
    # Hold one connection outside the timer. ``with db`` would open another
    # and commit on the way out, and the pragma setup would sit inside the
    # sample. Writes then share this connection and do not commit until the
    # explicit commit below (ASE's 5000-row commit is disabled).
    db.connection = db._connect()
    db.change_count = 0
    reset_commits(db)

    def run():
        for atoms in payload["atoms"]:
            db.write(atoms, frame_index=int(atoms.info["frame_index"]))
        db.connection.commit()

    seconds, _ = time_call(run)
    assert_one_commit(db)
    rows = read_ase_rows(db, payload["n_frames"])
    for row, rec in zip(rows, payload["records"]):
        check_stored(kind, row, rec, geometry_only)
    close_ase(db)
    return seconds


def stamp_frame_index(atoms_list: list[Atoms]) -> None:
    for i, atoms in enumerate(atoms_list):
        atoms.info["frame_index"] = i


def open_readcon(path: Path):
    """Read-only open. A writable open commits, and a read must not fsync."""
    return ConCorpus(str(path), readonly=True)


def measure_read(
    kind: str, dest: Path, op: str, n_frames: int, records: list[dict], mid: int
):
    """Open, read one full trajectory or one frame, close. No ``gc.collect``."""
    if kind == "readcon-db":
        db = open_readcon(dest / "corpus")
    else:
        db = open_ase_reader(dest / "atoms.db")
    try:
        if op == "full_read":
            if kind == "readcon-db":
                return full_read_readcon(db, n_frames, records)
            return full_read_ase(db, n_frames, records)
        rec = records[mid]
        if kind == "readcon-db":
            return selected_readcon(db, mid, rec)
        return selected_ase(db, mid, rec)
    finally:
        close_reader(kind, db)


def open_ase_reader(path: Path):
    """Read connection. No commit, so a timed read does not fsync."""
    db = connect(str(path), type="db")

    def _connect():
        con = sqlite3.connect(db.filename, timeout=20)
        con.execute("PRAGMA query_only=ON")
        return con

    db._connect = _connect  # type: ignore[method-assign]
    base = SQLite3Database.managed_connection

    @contextmanager
    def managed(self, commit_frequency: int = 2**31 - 1):
        with base(self, commit_frequency=commit_frequency) as con:
            yield con

    db.managed_connection = managed.__get__(db, type(db))  # type: ignore[method-assign]
    db.connection = db._connect()
    db.change_count = 0
    return db


def close_ase(db) -> None:
    con = db.connection
    db.connection = None
    if con is not None:
        con.close()


def _symbol_runs(symbols: list[str]) -> list[tuple[str, int]]:
    runs: list[tuple[str, int]] = []
    prev = None
    count = 0
    for symbol in symbols:
        if symbol == prev:
            count += 1
        else:
            if prev is not None:
                runs.append((prev, count))
            prev = symbol
            count = 1
    if prev is not None:
        runs.append((prev, count))
    return runs


def materialize_ase(rows) -> dict:
    """Same arrays `read_arrays` returns. Built inside the timed read."""
    positions = []
    velocities = []
    masses = []
    numbers = []
    constraints = []
    cells = []
    forces = []
    energy = []
    energy_set = []
    frame_index = []
    frame_index_set = []
    pbc = []
    runs = []
    for row in rows:
        atoms = row.toatoms()
        n = len(atoms)
        positions.append(np.array(atoms.positions, dtype=np.float64, copy=True))
        masses.append(np.array(atoms.get_masses(), dtype=np.float64, copy=True))
        numbers.append(np.array(atoms.get_atomic_numbers(), dtype=np.uint32, copy=True))
        constraints.append(np.zeros((n, 3), dtype=np.uint8))
        vel = atoms.get_velocities()
        if vel is None:
            velocities.append(np.zeros((n, 3), dtype=np.float64))
        else:
            velocities.append(np.array(vel, dtype=np.float64, copy=True))
        cells.append(np.array(atoms.cell.array, dtype=np.float64, copy=True))
        if getattr(row, "forces", None) is None:
            forces.append(None)
        else:
            forces.append(np.array(row.forces, dtype=np.float64, copy=True))
        has_energy = row.energy is not None
        energy.append(np.float64(row.energy if has_energy else 0.0))
        energy_set.append(has_energy)
        if "frame_index" in row.key_value_pairs:
            frame_index.append(np.float64(int(row.key_value_pairs["frame_index"])))
            frame_index_set.append(True)
        else:
            frame_index.append(np.float64(0.0))
            frame_index_set.append(False)
        pbc.append(tuple(bool(x) for x in atoms.pbc))
        runs.append(_symbol_runs(atoms.get_chemical_symbols()))
    force_arr = None
    if forces and all(item is not None for item in forces):
        force_arr = np.stack(forces, axis=0)
    return {
        "n_frames": len(rows),
        "positions": np.stack(positions, axis=0) if positions else np.zeros((0, 0, 3)),
        "velocities": np.stack(velocities, axis=0)
        if velocities
        else np.zeros((0, 0, 3)),
        "masses": np.stack(masses, axis=0) if masses else np.zeros((0, 0)),
        "numbers": np.stack(numbers, axis=0)
        if numbers
        else np.zeros((0, 0), dtype=np.uint32),
        "constraints": np.stack(constraints, axis=0)
        if constraints
        else np.zeros((0, 0, 3), dtype=np.uint8),
        "cell": np.stack(cells, axis=0) if cells else np.zeros((0, 3, 3)),
        "forces": force_arr,
        "energy": np.asarray(energy, dtype=np.float64),
        "energy_set": energy_set,
        "frame_index": np.asarray(frame_index, dtype=np.float64),
        "frame_index_set": frame_index_set,
        "pbc": pbc,
        "symbol_runs": runs,
    }


def _as_u32(values) -> np.ndarray:
    return np.asarray(values, dtype=np.uint32)


def _frame_at(values, index: int, n_frames: int):
    """One frame as a view. A single stored frame may already be unbatched."""
    arr = np.asarray(values)
    if n_frames == 1 and arr.shape[:1] != (1,):
        return arr
    return arr[index]


def check_materialized(got: dict, records: list[dict]) -> None:
    """Both tools return this shape. A mismatch aborts the campaign.

    Frames are compared one at a time so a million-atom check does not
    allocate a second copy of every column.
    """
    from ase.data import atomic_numbers

    n_frames = len(records)
    pos = got["positions"]
    masses = got["masses"]
    numbers = got["numbers"]
    constraints = got["constraints"]
    velocities = got["velocities"]
    cell = got["cell"]
    forces = got.get("forces")
    for i, rec in enumerate(records):
        if (
            _frame_at(pos, i, n_frames).tobytes()
            != np.asarray(rec["positions"]).tobytes()
        ):
            raise AssertionError("positions")
        if (
            _frame_at(masses, i, n_frames).tobytes()
            != np.asarray(rec["masses"]).tobytes()
        ):
            raise AssertionError("masses")
        got_z = _frame_at(numbers, i, n_frames).astype(np.uint32, copy=False)
        exp_z = np.array(
            [atomic_numbers[sym] for sym in rec["symbols"]], dtype=np.uint32
        )
        if got_z.tobytes() != exp_z.tobytes():
            raise AssertionError("numbers")
        got_c = _frame_at(constraints, i, n_frames)
        if rec.get("constraints") is None:
            if np.any(got_c):
                raise AssertionError("constraints")
        elif (
            got_c.tobytes() != np.asarray(rec["constraints"], dtype=np.uint8).tobytes()
        ):
            raise AssertionError("constraints")
        got_v = _frame_at(velocities, i, n_frames)
        if rec.get("velocities") is None:
            if np.any(got_v):
                raise AssertionError("velocities")
        elif got_v.tobytes() != np.asarray(rec["velocities"]).tobytes():
            raise AssertionError("velocities")
        if _frame_at(cell, i, n_frames).tobytes() != np.asarray(rec["cell"]).tobytes():
            raise AssertionError("cell")
        if rec.get("forces") is not None:
            if forces is None:
                raise AssertionError("forces missing")
            if (
                _frame_at(forces, i, n_frames).tobytes()
                != np.asarray(rec["forces"]).tobytes()
            ):
                raise AssertionError("forces")
    energy = np.asarray(got["energy"], dtype=np.float64)
    for i, rec in enumerate(records):
        if rec.get("energy") is None:
            continue
        if not got["energy_set"][i]:
            raise AssertionError("energy missing")
        if np.float64(energy[i]).tobytes() != np.float64(rec["energy"]).tobytes():
            raise AssertionError(("energy", float(energy[i]), rec["energy"]))
        if int(got["frame_index"][i]) != rec["frame_index"]:
            raise AssertionError("frame_index")
        if tuple(bool(x) for x in got["pbc"][i]) != tuple(bool(x) for x in rec["pbc"]):
            raise AssertionError("pbc")
        expect_runs = rec.get("symbol_runs") or _symbol_runs(rec["symbols"])
        if [tuple(run) for run in got["symbol_runs"][i]] != list(expect_runs):
            raise AssertionError("symbols")


def _check_columns(cols: dict, records: list[dict], geometry_only: bool) -> None:
    pos = np.asarray(cols["positions"], dtype=np.float64)
    exp = np.stack([rec["positions"] for rec in records], axis=0)
    if pos.shape != exp.shape or pos.tobytes() != exp.tobytes():
        raise AssertionError(("positions", pos.shape, exp.shape))
    if not geometry_only:
        forces = cols["forces"]
        if forces is None:
            raise AssertionError("forces missing")
        got_f = np.asarray(forces, dtype=np.float64)
        exp_f = np.stack([rec["forces"] for rec in records], axis=0)
        if got_f.shape != exp_f.shape or got_f.tobytes() != exp_f.tobytes():
            raise AssertionError("forces")
        masses = np.asarray(cols["masses"], dtype=np.float64)
        exp_m = np.stack([rec["masses"] for rec in records], axis=0)
        if masses.tobytes() != exp_m.tobytes():
            raise AssertionError("masses")
        energy = np.asarray(cols["energy"], dtype=np.float64)
        for i, rec in enumerate(records):
            if not cols["energy_set"][i]:
                raise AssertionError("energy missing")
            if np.float64(energy[i]).tobytes() != np.float64(rec["energy"]).tobytes():
                raise AssertionError(("energy", float(energy[i]), rec["energy"]))
            if int(cols["frame_index"][i]) != rec["frame_index"]:
                raise AssertionError("frame_index")
            if tuple(bool(x) for x in cols["pbc"][i]) != tuple(
                bool(x) for x in rec["pbc"]
            ):
                raise AssertionError("pbc")
            expect_runs = rec.get("symbol_runs")
            if expect_runs is None:
                expect_runs = _symbol_runs(rec["symbols"])
            if [tuple(run) for run in cols["symbol_runs"][i]] != list(expect_runs):
                raise AssertionError("symbols")
    cell = np.asarray(cols["cell"], dtype=np.float64)
    for i, rec in enumerate(records):
        lengths = np.diag(np.asarray(rec["cell"], dtype=np.float64))
        if cell[i, :3].tobytes() != lengths.tobytes():
            raise AssertionError(("cell", cell[i, :3], lengths))


def full_read_readcon(db, n_frames: int, records: list[dict]):
    cols = db.read_columns(1)
    if int(cols["n_frames"]) != n_frames:
        raise AssertionError(cols["n_frames"])
    geometry_only = (
        records[0].get("geometry_only", False) or records[0].get("forces") is None
    )
    _check_columns(cols, records, geometry_only)
    return cols


def full_read_ase(db, n_frames: int, records: list[dict]):
    rows = read_ase_rows(db, n_frames)
    for row, rec in zip(rows, records):
        assert_ase_matches(row, rec)
    return rows


def selected_readcon(db, mid: int, rec: dict):
    cols = db.read_columns(1, mid)
    if int(cols["n_frames"]) != 1:
        raise AssertionError(cols["n_frames"])
    geometry_only = rec.get("geometry_only", False) or rec.get("forces") is None
    _check_columns(cols, [rec], geometry_only)
    return cols


def selected_ase(db, mid: int, rec: dict):
    rows = list(db.select(f"frame_index={mid}"))
    if len(rows) != 1:
        raise AssertionError(len(rows))
    assert_ase_matches(rows[0], rec)
    return rows[0]


def close_reader(kind: str, db) -> None:
    if kind == "readcon-db":
        close_lmdb(db)
    else:
        close_ase(db)


def lmdb_files(corpus: Path) -> list[Path]:
    return [p for p in (corpus / "data.mdb", corpus / "lock.mdb") if p.is_file()]


def disk_bytes(kind: str, dest: Path) -> int:
    if kind == "readcon-db":
        path = dest / "corpus" / "data.mdb"
    else:
        path = dest / "atoms.db"
    return path.stat().st_size if path.is_file() else 0


def ase_files(path: Path) -> list[Path]:
    extra = [path]
    for suffix in ("-journal", "-wal", "-shm"):
        cand = Path(str(path) + suffix)
        if cand.is_file():
            extra.append(cand)
    return extra


def drop_files(paths: list[Path]) -> None:
    for path in paths:
        drop_page_cache(path)


def cold_data_files(kind: str, dest: Path) -> list[Path]:
    if kind == "readcon-db":
        return [p for p in (dest / "corpus" / "data.mdb",) if p.is_file()]
    return [p for p in (dest / "atoms.db",) if p.is_file()]


def prepare_cold_files(kind: str, dest: Path) -> tuple[float, str]:
    """Advise every database file. The recorded fraction is the data file."""
    if kind == "readcon-db":
        drop_files(lmdb_files(dest / "corpus"))
    else:
        drop_files(ase_files(dest / "atoms.db"))
    data = cold_data_files(kind, dest)
    if not data:
        return 0.0, "verified"
    worst = 0.0
    label = "verified"
    for path in data:
        frac, one = advise_cold(path)
        if frac > worst:
            worst = frac
            label = one
    return worst, label


def fetch_materialized(kind: str, db, op: str, mid: int):
    """Arrays for one read. Both tools build them inside the clock."""
    if kind == "readcon-db":
        if op == "full_read":
            return db.read_arrays(1)
        return db.read_arrays(1, int(mid))
    if op == "full_read":
        rows = list(db.select())
        rows.sort(key=lambda row: int(row.key_value_pairs["frame_index"]))
    else:
        rows = list(db.select(f"frame_index={int(mid)}"))
    return materialize_ase(rows)


def _array_digest(value) -> str:
    if isinstance(value, list):
        blob = json.dumps(value).encode()
    else:
        blob = np.ascontiguousarray(value).tobytes()
    return hashlib.sha256(blob).hexdigest()


def digests_of(got: dict) -> dict:
    keys = (
        "positions",
        "velocities",
        "masses",
        "numbers",
        "constraints",
        "cell",
        "energy",
        "energy_set",
        "frame_index",
        "frame_index_set",
        "pbc",
        "symbol_runs",
    )
    out = {key: _array_digest(got[key]) for key in keys}
    forces = got.get("forces")
    out["forces"] = None if forces is None else _array_digest(forces)
    return out


def assert_digests(got: dict, expected: dict) -> None:
    got_d = digests_of(got)
    for key, value in expected.items():
        if got_d.get(key) != value:
            raise AssertionError(("digest", key))


def execute_insert_row(request: dict) -> dict:
    spec = request["spec"]
    kind = request["kind"]
    name = spec["name"]
    work = Path(request["work"])
    if spec.get("paper_text"):
        text = paper_con_text(spec["n_frames"], spec["n_atoms"])
        frames = readcon.read_con_string(text)
        atoms_list = []
        records = []
        for i, frame in enumerate(frames):
            atoms = frame.to_ase()
            atoms.info["frame_index"] = i
            pos = np.array(frame.coords_array(), dtype=np.float64)
            cell = np.diag(np.array(frame.cell, dtype=np.float64))
            masses = np.array([atom.mass for atom in frame.atoms], dtype=np.float64)
            records.append(
                {
                    "symbols": [atom.symbol for atom in frame.atoms],
                    "positions": pos,
                    "cell": cell,
                    "pbc": tuple(bool(x) for x in atoms.pbc),
                    "masses": masses,
                    "forces": None,
                    "energy": None,
                    "frame_index": i,
                    "geometry_only": True,
                }
            )
            atoms_list.append(atoms)
        payload = {
            "text": text,
            "atoms": atoms_list,
            "records": records,
            "n_frames": spec["n_frames"],
            "geometry_only": True,
        }
    elif kind == "readcon-db":
        # The CON text is streamed from the column arrays. A ConFrame of
        # 10^6 atoms, next to those arrays, was the 1.6 GB peak.
        text, records = build_text_records(spec["n_frames"], spec["n_atoms"])
        payload = {
            "text": text,
            "atoms": [],
            "records": records,
            "n_frames": spec["n_frames"],
            "geometry_only": False,
        }
    else:
        atoms_list, records = build_atom_records(spec["n_frames"], spec["n_atoms"])
        payload = {
            "text": "",
            "atoms": atoms_list,
            "records": records,
            "n_frames": spec["n_frames"],
            "geometry_only": False,
        }
    lines = [f"warmup protocol=fair workload={name} op=insert tool={kind}"]
    timed_insert(kind, name, payload, work)
    values = []
    inner = []
    disk = None
    i = 0
    while i < N_SAMPLES or iqr_over(values):
        stat = timed_insert(kind, name, payload, work)
        values.append(stat["seconds"])
        inner.append(stat["inner_n"])
        disk = stat["disk_bytes"]
        lines.append(
            f"sample protocol=fair workload={name} op=insert tool={kind} "
            f"cache=cold-file i={i} seconds={stat['seconds']} "
            f"inner_n={stat['inner_n']} timed_span_s={stat['timed_span_s']:.6f} "
            f"disk_bytes={stat['disk_bytes']}"
        )
        i += 1
    row = {
        "protocol": "fair",
        "workload": name,
        "n_frames": spec["n_frames"],
        "n_atoms": spec["n_atoms"],
        "atoms_total": spec["n_frames"] * spec["n_atoms"],
        "op": "insert",
        "cache": "cold-file",
        "tool": kind,
        "fidelity": "exact",
        "disk_bytes": disk,
        "peak_rss_bytes": vm_hwm(),
        "inner_n": inner,
        "span_s": INSERT_SPAN_S,
        "log": lines,
        **summarize(values),
    }
    return row


def execute_read_row(request: dict) -> dict:
    kind = request["kind"]
    op = request["op"]
    cache = request["cache"]
    name = request["name"]
    dest = Path(request["dest"])
    n_frames = int(request["n_frames"])
    mid = n_frames // 2
    expected = request["digests"]
    lines = []
    samples = []
    cold_meta = []
    holder = {}

    def open_kind():
        if kind == "readcon-db":
            return open_readcon(dest / "corpus")
        return open_ase_reader(dest / "atoms.db")

    def prepare():
        if kind in holder:
            close_reader(kind, holder.pop("db"))
        return prepare_cold_files(kind, dest)

    if cache == "warm":
        holder["db"] = open_kind()
        fetch_materialized(kind, holder["db"], op, mid)
        assert_digests(fetch_materialized(kind, holder["db"], op, mid), expected)
        lines.append(
            f"warmup protocol=fair workload={name} op={op} tool={kind} cache={cache}"
        )
    else:
        prepare()
        db = open_kind()
        try:
            got = fetch_materialized(kind, db, op, mid)
            assert_digests(got, expected)
        finally:
            close_reader(kind, db)
            db = None
            if kind == "readcon-db":
                gc.collect()
        lines.append(
            f"warmup protocol=fair workload={name} op={op} tool={kind} cache={cache}"
        )

    i = 0
    while i < N_SAMPLES or iqr_over(samples):
        state: dict = {}
        labels: list[tuple[float, str]] = []

        def setup(labels=labels, state=state) -> None:
            frac, label = prepare()
            labels.append((frac, label))
            # cold-open times the open. Opening here would fault pages
            # before the clock and then open a second handle inside it.
            if cache != "cold-open":
                state["db"] = open_kind()

        def teardown(state=state) -> None:
            assert_digests(state["got"], expected)
            close_reader(kind, state["db"])
            state["db"] = None
            state["got"] = None
            if kind == "readcon-db":
                gc.collect()

        def run_timed(state=state) -> None:
            state["got"] = fetch_materialized(kind, state["db"], op, mid)

        if cache == "cold-open":
            # The open is inside the clock, so neither tool keeps readahead
            # from an open that ran before the timer.
            def run_open(state=state) -> None:
                state["db"] = open_kind()
                state["got"] = fetch_materialized(kind, state["db"], op, mid)

            stat = sample_operation(run_open, setup, teardown)
            resident = max(frac for frac, _label in labels)
            cold_label = (
                "verified"
                if all(label == "verified" for _frac, label in labels)
                else "resident"
            )
            cold_meta.append(
                {
                    "resident_fraction": resident,
                    "cold_label": cold_label,
                    "seconds": stat["seconds"],
                    "inner_n": stat["inner_n"],
                }
            )
            if cold_label == "verified":
                samples.append(stat["seconds"])
            extra = f" resident_fraction={resident:.6f} cold_label={cold_label}"
        elif cache == "cold":
            stat = sample_operation(run_timed, setup, teardown)
            resident = max(frac for frac, _label in labels)
            cold_label = (
                "verified"
                if all(label == "verified" for _frac, label in labels)
                else "resident"
            )
            cold_meta.append(
                {
                    "resident_fraction": resident,
                    "cold_label": cold_label,
                    "seconds": stat["seconds"],
                    "inner_n": stat["inner_n"],
                }
            )
            if cold_label == "verified":
                samples.append(stat["seconds"])
            extra = f" resident_fraction={resident:.6f} cold_label={cold_label}"
        else:

            def run_warm(state=state) -> None:
                state["got"] = fetch_materialized(kind, holder["db"], op, mid)

            def check_warm(state=state) -> None:
                assert_digests(state["got"], expected)
                state["got"] = None

            stat = sample_operation(run_warm, teardown=check_warm)
            samples.append(stat["seconds"])
            extra = ""
        lines.append(
            f"sample protocol=fair workload={name} op={op} tool={kind} "
            f"cache={cache} i={i} seconds={stat['seconds']} "
            f"inner_n={stat['inner_n']} timed_span_s={stat['timed_span_s']:.6f}{extra}"
        )
        i += 1
    if "db" in holder:
        close_reader(kind, holder["db"])
    used = samples or [item["seconds"] for item in cold_meta]
    row = {
        "protocol": "fair",
        "workload": name,
        "n_frames": n_frames,
        "n_atoms": int(request["n_atoms"]),
        "atoms_total": n_frames * int(request["n_atoms"]),
        "op": op,
        "cache": cache,
        "tool": kind,
        "fidelity": "exact",
        "disk_bytes": disk_bytes(kind, dest),
        "peak_rss_bytes": vm_hwm(),
        "span_s": SAMPLE_S,
        "log": lines,
        **summarize(used),
    }
    if cache in ("cold", "cold-open"):
        row["cold_verified_n"] = len(samples)
        row["cold_samples"] = cold_meta
        row["inner_n"] = [item["inner_n"] for item in cold_meta]
    return row


def execute_row(request: dict) -> dict:
    if request["what"] == "insert":
        return execute_insert_row(request)
    return execute_read_row(request)


def spawn_row(request: dict) -> dict:
    """One table row, in a process whose high-water mark starts at zero."""
    env = os.environ.copy()
    env["READCON_DB_ROW"] = "1"
    proc = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "--row"],
        check=False,
        input=json.dumps(request),
        text=True,
        capture_output=True,
        env=env,
    )
    if proc.returncode != 0:
        tail = (proc.stderr or "")[-4000:] + (proc.stdout or "")[-2000:]
        raise RuntimeError(tail)
    return json.loads(proc.stdout)


def digests_for_records(records: list[dict]) -> dict:
    """sha256 of the arrays both tools must return for these frames."""
    probe = {
        "positions": np.stack([rec["positions"] for rec in records]),
        "masses": np.stack([rec["masses"] for rec in records]),
        "cell": np.stack([rec["cell"] for rec in records]),
        "velocities": np.zeros(
            (len(records), records[0]["positions"].shape[0], 3), dtype=np.float64
        ),
        "constraints": np.zeros(
            (len(records), records[0]["positions"].shape[0], 3), dtype=np.uint8
        ),
        "numbers": np.stack(
            [
                np.array(
                    [__import__("ase").data.atomic_numbers[s] for s in rec["symbols"]],
                    dtype=np.uint32,
                )
                for rec in records
            ]
        ),
        "energy": np.array(
            [np.float64(rec["energy"] or 0.0) for rec in records],
            dtype=np.float64,
        ),
        "energy_set": [rec.get("energy") is not None for rec in records],
        "frame_index": np.array(
            [np.float64(rec["frame_index"]) for rec in records],
            dtype=np.float64,
        ),
        "frame_index_set": [True for _ in records],
        "pbc": [tuple(bool(x) for x in rec["pbc"]) for rec in records],
        "symbol_runs": [
            [
                list(run)
                for run in (rec.get("symbol_runs") or _symbol_runs(rec["symbols"]))
            ]
            for rec in records
        ],
        "forces": None
        if records[0].get("forces") is None
        else np.stack([rec["forces"] for rec in records]),
    }
    for rec, vel in zip(records, probe["velocities"]):
        if rec.get("velocities") is not None:
            vel[...] = rec["velocities"]
    return digests_of(probe)


def fair_campaign(log: Log, work: Path, workloads: list[dict]) -> list[dict]:
    rows = []
    for spec in workloads:
        name = spec["name"]
        log.line(
            f"workload {name} frames={spec['n_frames']} atoms={spec['n_atoms']} build_start"
        )
        if spec.get("paper_text"):
            text = paper_con_text(spec["n_frames"], spec["n_atoms"])
            frames = readcon.read_con_string(text)
            atoms_list = []
            records = []
            for i, frame in enumerate(frames):
                atoms = frame.to_ase()
                atoms.info["frame_index"] = i
                pos = np.array(frame.coords_array(), dtype=np.float64)
                cell = np.diag(np.array(frame.cell, dtype=np.float64))
                masses = np.array([atom.mass for atom in frame.atoms], dtype=np.float64)
                records.append(
                    {
                        "symbols": [atom.symbol for atom in frame.atoms],
                        "positions": pos,
                        "cell": cell,
                        "pbc": tuple(bool(x) for x in atoms.pbc),
                        "masses": masses,
                        "forces": None,
                        "energy": None,
                        "frame_index": i,
                        "geometry_only": True,
                    }
                )
                atoms_list.append(atoms)
            payload = {
                "text": text,
                "atoms": atoms_list,
                "records": records,
                "n_frames": spec["n_frames"],
                "geometry_only": True,
            }
        else:
            text, atoms_list, records = build_records(spec["n_frames"], spec["n_atoms"])
            stamp_frame_index(atoms_list)
            payload = {
                "text": text,
                "atoms": atoms_list,
                "records": records,
                "n_frames": spec["n_frames"],
                "geometry_only": False,
            }
        log.line(f"workload {name} bytes={len(text)} build_done")
        # One fresh process per row, so peak RSS does not carry over.
        for kind in ("readcon-db", "ase.db"):
            row = spawn_row(
                {"what": "insert", "kind": kind, "spec": spec, "work": str(work)}
            )
            for line in row.pop("log"):
                log.line(line)
            rows.append(row)
        if spec.get("insert_only"):
            continue
        # One store of each tool for reads. Built outside the read timer.
        stored = {}
        for kind in ("readcon-db", "ase.db"):
            dest = fresh_dir(work, f"{name}-read-{kind}")
            insert_fair(kind, payload, dest)
            gc.collect()
            stored[kind] = dest
        mid = spec["n_frames"] // 2
        ops = ("full_read", "selected")
        caches = ("cold", "warm")
        if spec["name"] == "frames-100":
            caches = ("cold", "cold-open", "warm")
        for cache in caches:
            for op in ops:
                used = (
                    payload["records"]
                    if op == "full_read"
                    else [payload["records"][mid]]
                )
                digests = digests_for_records(used)
                for kind in ("readcon-db", "ase.db"):
                    row = spawn_row(
                        {
                            "what": "read",
                            "kind": kind,
                            "op": op,
                            "cache": cache,
                            "name": name,
                            "dest": str(stored[kind]),
                            "n_frames": spec["n_frames"],
                            "n_atoms": spec["n_atoms"],
                            "digests": digests,
                        }
                    )
                    for line in row.pop("log"):
                        log.line(line)
                    rows.append(row)
        for dest in stored.values():
            shutil.rmtree(dest, ignore_errors=True)
    return rows


def fmt(x: float) -> str:
    if x == 0:
        return "0"
    return f"{x:.4e}"


def write_markdown(
    path: Path, published: dict, paper_local: dict, fair_rows: list[dict]
) -> None:
    lines = [
        "# readcon-db vs ASE sqlite",
        "",
        "Type-7 median and IQR. One discarded warmup and six serial samples",
        "for every fair row. The published log has no discarded warmup.",
        "A mismatch of any stored field aborts the run, so every sample below",
        "passed an element-wise check (fidelity exact).",
        "Cold medians use samples whose mincore resident fraction is 0.",
        "Full read and the selected frame share each sample index.",
        "",
        "## Published ingest (`paper/cpc/logs/db.log`)",
        "",
        "40 frames, 8 Cu atoms. ASE commits every row. No warmup.",
        "",
        "| tool | median s | IQR s | q1 s | q3 s | min s | max s |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for tool, stats in published.items():
        lines.append(
            f"| {tool} | {fmt(stats['median_s'])} | {fmt(stats['iqr_s'])} | "
            f"{fmt(stats['q1_s'])} | {fmt(stats['q3_s'])} | {fmt(stats['min_s'])} | {fmt(stats['max_s'])} |"
        )
    lines += [
        "",
        "## This machine, published harness",
        "",
        "| tool | median s | IQR s | q1 s | q3 s | min s | max s |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for tool, stats in paper_local.items():
        lines.append(
            f"| {tool} | {fmt(stats['median_s'])} | {fmt(stats['iqr_s'])} | "
            f"{fmt(stats['q1_s'])} | {fmt(stats['q3_s'])} | {fmt(stats['min_s'])} | {fmt(stats['max_s'])} |"
        )
    lines += [
        "",
        "## This machine, fair harness",
        "",
        "Insert creates the database outside the clock and times the commit.",
        "A full read and the selected-row query (`frame_index` of the middle",
        "frame) copy positions, cell, masses, atomic numbers, constraints,",
        "and velocities into NumPy arrays. ASE does the same via toatoms().",
        "db5 timed only the sqlite fetch, which is why its 1M-atom warm",
        "full read was 0.0166 s against db4's 0.226 s. Open, directory",
        "fsync, fadvise, mincore, and the field check stay outside the",
        "clock. cold-open times the open and the query together, for",
        "both tools, so neither keeps readahead from an open outside",
        "the clock. The cold row still opens outside the clock. Each",
        "row is its own process. A short read repeats to 50 ms and a",
        "short commit to 2 s. Both inserts are one transaction and one",
        "commit fsync. A long readcon-db trajectory puts one 64-frame",
        "group at a time and drops that scratch after the put; the",
        "transaction still commits once. Six samples are kept. A row",
        "whose IQR/median is still above 0.05 takes six more. p90 is",
        "the type-7 90th percentile of those means.",
        "",
        "| workload | op | cache | tool | n | median s | p90 s | IQR s | IQR/median | disk B | peak RSS B | fidelity |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in fair_rows:
        ratio = row["iqr_s"] / row["median_s"] if row["median_s"] else 0.0
        p90 = float(
            np.percentile(
                np.asarray(row["samples_s"], dtype=np.float64), 90, method="linear"
            )
        )
        lines.append(
            f"| {row['workload']} | {row['op']} | {row['cache']} | {row['tool']} | {row['n']} | "
            f"{fmt(row['median_s'])} | {fmt(p90)} | {fmt(row['iqr_s'])} | {ratio:.3f} | "
            f"{row.get('disk_bytes', 0)} | {row.get('peak_rss_bytes', 0)} | "
            f"{row.get('fidelity', 'exact')} |"
        )
    lines.append("")
    path.write_text("\n".join(lines) + "\n")


def workloads_for(mode: str) -> list[dict]:
    paper = {
        "name": "paper40x8",
        "n_frames": 40,
        "n_atoms": 8,
        "paper_text": True,
        "insert_only": True,
    }
    atoms = [
        {"name": f"atoms-{n}", "n_frames": 1, "n_atoms": n}
        for n in (1_000, 10_000, 100_000, 1_000_000)
    ]
    frames = [
        {"name": f"frames-{n}", "n_frames": n, "n_atoms": 8} for n in (1, 100, 10_000)
    ]
    if mode == "paper":
        return [paper]
    if mode == "ladder":
        return atoms + frames
    if mode == "inserts":
        rows = [paper, *atoms, *frames]
        for row in rows:
            row["insert_only"] = True
        return rows
    if mode == "db8":
        return [
            {"name": "atoms-100000", "n_frames": 1, "n_atoms": 100_000},
            {"name": "atoms-1000000", "n_frames": 1, "n_atoms": 1_000_000},
            {"name": "frames-100", "n_frames": 100, "n_atoms": 8},
            {"name": "frames-10000", "n_frames": 10_000, "n_atoms": 8},
        ]
    if mode == "db9":
        return [
            {"name": "atoms-1000000", "n_frames": 1, "n_atoms": 1_000_000},
            {"name": "frames-10000", "n_frames": 10_000, "n_atoms": 8},
            {"name": "frames-100", "n_frames": 100, "n_atoms": 8},
        ]
    if mode == "db10":
        return [{"name": "frames-100", "n_frames": 100, "n_atoms": 8}]
    if mode == "smoke":
        return [
            paper,
            {"name": "atoms-1000", "n_frames": 1, "n_atoms": 1000},
            {"name": "frames-1", "n_frames": 1, "n_atoms": 8},
        ]
    return [paper] + atoms + frames


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--out", type=Path, default=Path(__file__).resolve().parent / "results" / "fair"
    )
    ap.add_argument(
        "--mode",
        choices=("all", "paper", "ladder", "smoke", "inserts", "db8", "db9", "db10"),
        default="all",
    )
    ap.add_argument("--self-check", action="store_true")
    ap.add_argument("--row", action="store_true")
    args = ap.parse_args()
    if args.row:
        request = json.loads(sys.stdin.read())
        json.dump(execute_row(request), sys.stdout)
        sys.stdout.write("\n")
        return
    published = {
        "readcon-db": summarize(PUBLISHED_READCON),
        "ase.db": summarize(PUBLISHED_ASE),
    }
    if args.self_check:
        json.dump(published, sys.stdout, indent=2)
        sys.stdout.write("\n")
        return
    args.out.mkdir(parents=True, exist_ok=True)
    work = args.out / "work"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    log = Log(args.out / "campaign.log")
    for line in header_lines():
        log.line(line)
    _db, settings = connect_fair(work / "settings-probe.db")
    log.line("ase_settings " + json.dumps(settings, sort_keys=True))
    paper_local = {}
    fair_rows: list[dict] = []
    specs = workloads_for(args.mode)
    if args.mode in ("all", "paper", "smoke"):
        paper_local = paper_protocol(log, work)
    fair_rows = fair_campaign(log, work, specs)
    log.line("exit_code 0")
    log.close()
    payload = {
        "published_db_v2": published,
        "local_paper_harness": paper_local,
        "fair": fair_rows,
        "settings": settings,
        "head": git_head(),
    }
    (args.out / "campaign.json").write_text(json.dumps(payload, indent=2) + "\n")
    noise = {}
    for row in fair_rows:
        if (
            row["workload"] == "frames-100"
            and row["op"] == "selected"
            and row["cache"]
            in (
                "cold",
                "cold-open",
            )
        ):
            samples = row["samples_s"]
            noise[f"{row['tool']}:{row['cache']}"] = {
                "n": row["n"],
                "median_s": row["median_s"],
                "p90_s": float(
                    np.percentile(
                        np.asarray(samples, dtype=np.float64), 90, method="linear"
                    )
                ),
                "iqr_s": row["iqr_s"],
                "iqr_over_median": (row["iqr_s"] / row["median_s"])
                if row["median_s"]
                else None,
                "samples_s": samples,
            }
    if noise:
        (args.out / "noise.json").write_text(json.dumps(noise, indent=2) + "\n")
    write_markdown(args.out / "RESULTS.md", published, paper_local, fair_rows)
    print(f"wrote {args.out / 'RESULTS.md'}")


if __name__ == "__main__":
    main()
