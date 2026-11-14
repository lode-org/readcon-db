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
Reads are a separate cold series and a warm series. Cold closes the
store, advises ``DONTNEED``, and checks residency with ``mincore``.
The advice is retried until no page is resident. A sample that stays
resident is labeled and left out of the cold median. n = 6, serial,
type-7 median and IQR. Every stored field is checked; a mismatch aborts.
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.util
import gc
import json
import os
import platform
import shutil
import socket
import sqlite3
import subprocess
import sys
import threading
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
N_SAMPLES = 6
# A single sub-millisecond fsync or query is mostly timer noise. Repeat it
# until the timed spans cover this long, and publish the mean as one sample.
# Reads repeat until the timed spans cover this. A 50 ms mean is enough
# once open, advice, and the field check are outside the clock.
SAMPLE_S = 0.050
# Commit fsyncs are correlated (lag-1 about 0.37 on a one-frame insert).
# A 50 ms or 250 ms mean of those commits still has IQR/median above 0.15.
# Means of about 2000 commits, roughly two seconds, fall under that line.
INSERT_SPAN_S = 2.0
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
        f"samples {N_SAMPLES}",
        "quantile type-7 (numpy percentile method=linear)",
        "order serial, tools alternate each sample index, no parallel samples",
        "timer: open, directory fsync, fadvise, mincore, the element-wise check, and the collector stay outside; the clock covers the commit or the query",
        "timer_short: a read shorter than 50 ms is repeated until the timed spans add to 50 ms; the sample is that mean",
        "timer_insert: a commit shorter than 2 s is repeated until the timed spans add to 2 s, because commit fsyncs stay correlated past 250 ms",
        "page_cache insert: unlink and create the database file outside the timer, one commit fsync inside",
        "page_cache read_cold: posix_fadvise DONTNEED, then mincore, outside the timer; retry until resident fraction is 0",
        "page_cache read_cold_label: a sample that stays resident is labeled and omitted from the cold median",
        "page_cache drop_caches: /proc/sys/vm/drop_caches is not writable here, so it is not used",
        "page_cache read_warm: one warmup read on the open store, then time the read",
        "lmdb MDB_NOSYNC unset, one write transaction, fsync on commit",
        "ase backend sqlite journal_mode=DELETE synchronous=FULL, one commit fsync",
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


def vm_rss() -> int:
    """Current resident set, in bytes."""
    try:
        for line in Path("/proc/self/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) * 1024
    except OSError:
        return 0
    return 0


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
    """
    peak = vm_rss()
    stop = threading.Event()

    def poll() -> None:
        nonlocal peak
        while not stop.wait(0.002):
            peak = max(peak, vm_rss())

    thread = threading.Thread(target=poll, daemon=True)
    thread.start()
    total = 0.0
    count = 0
    collect = gc.isenabled()
    limit = SAMPLE_S if span is None else span
    try:
        while True:
            if setup is not None:
                setup()
            try:
                peak = max(peak, vm_rss())
                if collect:
                    gc.disable()
                t0 = time.perf_counter()
                run()
                dt = time.perf_counter() - t0
                peak = max(peak, vm_rss())
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
        stop.set()
        thread.join()
    return {
        "seconds": total / count,
        "inner_n": count,
        "timed_span_s": total,
        "peak_rss_bytes": peak,
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
            frames = read_con_frames(db, payload["n_frames"])
            for frame, rec in zip(frames, payload["records"]):
                check_stored(kind, frame, rec, geometry_only)
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
        # Insert: unlink+create each sample, one warmup, six serial samples.
        insert_samples = {"readcon-db": [], "ase.db": []}
        insert_meta = {"readcon-db": [], "ase.db": []}

        for kind in ("readcon-db", "ase.db"):
            timed_insert(kind, name, payload, work)
            log.line(f"warmup protocol=fair workload={name} op=insert tool={kind}")
        for i in range(N_SAMPLES):
            order = ("readcon-db", "ase.db") if i % 2 == 0 else ("ase.db", "readcon-db")
            for kind in order:
                stat = timed_insert(kind, name, payload, work)
                insert_samples[kind].append(stat["seconds"])
                insert_meta[kind].append(stat)
                log.line(
                    f"sample protocol=fair workload={name} op=insert tool={kind} "
                    f"cache=cold-file i={i} seconds={stat['seconds']} "
                    f"inner_n={stat['inner_n']} timed_span_s={stat['timed_span_s']:.6f} "
                    f"disk_bytes={stat['disk_bytes']} peak_rss_bytes={stat['peak_rss_bytes']}"
                )
        for kind, values in insert_samples.items():
            meta = insert_meta[kind]
            rows.append(
                {
                    "protocol": "fair",
                    "workload": name,
                    "n_frames": spec["n_frames"],
                    "n_atoms": spec["n_atoms"],
                    "atoms_total": spec["n_frames"] * spec["n_atoms"],
                    "op": "insert",
                    "cache": "cold-file",
                    "tool": kind,
                    "fidelity": "exact",
                    "disk_bytes": meta[0]["disk_bytes"],
                    "peak_rss_bytes": max(item["peak_rss_bytes"] for item in meta),
                    "inner_n": [item["inner_n"] for item in meta],
                    "span_s": INSERT_SPAN_S,
                    **summarize(values),
                }
            )
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
        # Full read and the selected frame share one sample index. The column
        # scan is the same once the readonly open is outside the clock.
        ops = ("full_read", "selected")
        for cache in ("cold", "warm"):
            samples = {op: {"readcon-db": [], "ase.db": []} for op in ops}
            cold_meta = {op: {"readcon-db": [], "ase.db": []} for op in ops}
            holders = {}

            def prepare_cold(kind: str, stored=stored, holders=holders):
                dest = stored[kind]
                if kind in holders:
                    close_reader(kind, holders.pop(kind))
                return prepare_cold_files(kind, dest)

            def open_kind(kind: str, stored=stored):
                dest = stored[kind]
                if kind == "readcon-db":
                    return open_readcon(dest / "corpus")
                return open_ase_reader(dest / "atoms.db")

            def run_op(
                kind: str,
                db,
                op: str,
                n_frames=spec["n_frames"],
                records=payload["records"],
                mid=mid,
            ):
                if op == "full_read":
                    if kind == "readcon-db":
                        return full_read_readcon(db, n_frames, records)
                    return full_read_ase(db, n_frames, records)
                rec = records[mid]
                if kind == "readcon-db":
                    return selected_readcon(db, mid, rec)
                return selected_ase(db, mid, rec)

            def fetch(kind: str, db, op: str, mid=mid):
                """The query only. The element-wise check stays outside the clock."""
                if kind == "readcon-db":
                    if op == "full_read":
                        return db.read_columns(1)
                    return db.read_columns(1, mid)
                if op == "full_read":
                    return list(db.select())
                return list(db.select(f"frame_index={int(mid)}"))

            def check_fetched(
                kind: str,
                got,
                op: str,
                n_frames=spec["n_frames"],
                records=payload["records"],
                mid=mid,
            ) -> None:
                if kind == "readcon-db":
                    if op == "full_read":
                        if int(got["n_frames"]) != n_frames:
                            raise AssertionError(got["n_frames"])
                        geometry_only = (
                            records[0].get("geometry_only", False)
                            or records[0].get("forces") is None
                        )
                        _check_columns(got, records, geometry_only)
                        return
                    if int(got["n_frames"]) != 1:
                        raise AssertionError(got["n_frames"])
                    rec = records[mid]
                    geometry_only = (
                        rec.get("geometry_only", False) or rec.get("forces") is None
                    )
                    _check_columns(got, [rec], geometry_only)
                    return
                if op == "full_read":
                    rows = sorted(
                        got,
                        key=lambda row: int(row.key_value_pairs["frame_index"]),
                    )
                    if len(rows) != n_frames:
                        raise AssertionError(len(rows))
                    for row, rec in zip(rows, records):
                        assert_ase_matches(row, rec)
                    return
                if len(got) != 1:
                    raise AssertionError(len(got))
                assert_ase_matches(got[0], records[mid])

            if cache == "warm":
                for kind in ("readcon-db", "ase.db"):
                    holders[kind] = open_kind(kind)
                    for op in ops:
                        run_op(kind, holders[kind], op)
                        log.line(
                            f"warmup protocol=fair workload={name} op={op} tool={kind} cache={cache}"
                        )
            else:
                for op in ops:
                    for kind in ("readcon-db", "ase.db"):
                        prepare_cold(kind)
                        db = open_kind(kind)
                        try:
                            run_op(kind, db, op)
                        finally:
                            close_reader(kind, db)
                        log.line(
                            f"warmup protocol=fair workload={name} op={op} tool={kind} cache={cache}"
                        )
            for i in range(N_SAMPLES):
                tools = (
                    ("readcon-db", "ase.db") if i % 2 == 0 else ("ase.db", "readcon-db")
                )
                op_order = ops if i % 2 == 0 else ("selected", "full_read")
                for kind in tools:
                    for op in op_order:
                        resident = None
                        cold_label = None
                        state: dict = {}
                        labels: list[tuple[float, str]] = []

                        def setup(kind=kind, labels=labels, state=state) -> None:
                            frac, label = prepare_cold(kind)
                            labels.append((frac, label))
                            state["db"] = open_kind(kind)

                        def teardown(kind=kind, op=op, state=state) -> None:
                            check_fetched(kind, state["got"], op)
                            close_reader(kind, state["db"])
                            state["db"] = None
                            state["got"] = None
                            if kind == "readcon-db":
                                gc.collect()

                        def run_timed(kind=kind, op=op, state=state) -> None:
                            state["got"] = fetch(kind, state["db"], op)

                        if cache == "cold":
                            stat = sample_operation(run_timed, setup, teardown)
                            resident = max(frac for frac, _label in labels)
                            cold_label = (
                                "verified"
                                if all(label == "verified" for _frac, label in labels)
                                else "resident"
                            )
                            cold_meta[op][kind].append(
                                {
                                    "resident_fraction": resident,
                                    "cold_label": cold_label,
                                    "seconds": stat["seconds"],
                                    "inner_n": stat["inner_n"],
                                }
                            )
                            if cold_label == "verified":
                                samples[op][kind].append(stat["seconds"])
                        else:
                            db = holders[kind]

                            def run_warm(kind=kind, db=db, op=op, state=state) -> None:
                                state["got"] = fetch(kind, db, op)

                            def check_warm(kind=kind, op=op, state=state) -> None:
                                check_fetched(kind, state["got"], op)
                                state["got"] = None

                            stat = sample_operation(run_warm, teardown=check_warm)
                            resident = None
                            cold_label = None
                            samples[op][kind].append(stat["seconds"])
                        extra = (
                            f" inner_n={stat['inner_n']} timed_span_s={stat['timed_span_s']:.6f} "
                            f"peak_rss_bytes={stat['peak_rss_bytes']}"
                        )
                        if resident is not None:
                            extra += f" resident_fraction={resident:.6f} cold_label={cold_label}"
                        log.line(
                            f"sample protocol=fair workload={name} op={op} tool={kind} "
                            f"cache={cache} i={i} seconds={stat['seconds']}{extra}"
                        )
                        samples[op][kind + ":peak"] = max(
                            samples[op].get(kind + ":peak", 0), stat["peak_rss_bytes"]
                        )
            for kind in list(holders):
                close_reader(kind, holders.pop(kind))
            gc.collect()
            for op in ops:
                for kind in ("readcon-db", "ase.db"):
                    used = samples[op][kind]
                    if cache == "cold" and not used:
                        used = [item["seconds"] for item in cold_meta[op][kind]]
                    row = {
                        "protocol": "fair",
                        "workload": name,
                        "n_frames": spec["n_frames"],
                        "n_atoms": spec["n_atoms"],
                        "atoms_total": spec["n_frames"] * spec["n_atoms"],
                        "op": op,
                        "cache": cache,
                        "tool": kind,
                        "fidelity": "exact",
                        "disk_bytes": disk_bytes(kind, stored[kind]),
                        "peak_rss_bytes": samples[op].get(kind + ":peak", 0),
                        "span_s": SAMPLE_S,
                        **summarize(used),
                    }
                    if cache == "cold":
                        row["cold_verified_n"] = len(samples[op][kind])
                        row["cold_samples"] = cold_meta[op][kind]
                        row["inner_n"] = [
                            item["inner_n"] for item in cold_meta[op][kind]
                        ]
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
        "frame) time the query only: open, directory fsync, fadvise,",
        "mincore, the field check, and the collector stay outside. A read",
        "shorter than 50 ms repeats until the timed spans add to 50 ms. A",
        "commit shorter than 2 s repeats until they add to 2 s, because",
        "those fsyncs stay correlated well past 250 ms. The sample is that",
        "mean. A row that stayed above an IQR/median of 0.15 was measured",
        "again at a longer span; that budget is `span_s` on the row.",
        "",
        "| workload | op | cache | tool | n | median s | IQR s | IQR/median | disk B | peak RSS B | fidelity |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in fair_rows:
        ratio = row["iqr_s"] / row["median_s"] if row["median_s"] else 0.0
        lines.append(
            f"| {row['workload']} | {row['op']} | {row['cache']} | {row['tool']} | {row['n']} | "
            f"{fmt(row['median_s'])} | {fmt(row['iqr_s'])} | {ratio:.3f} | "
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
        choices=("all", "paper", "ladder", "smoke", "inserts"),
        default="all",
    )
    ap.add_argument("--self-check", action="store_true")
    args = ap.parse_args()
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
    write_markdown(args.out / "RESULTS.md", published, paper_local, fair_rows)
    print(f"wrote {args.out / 'RESULTS.md'}")


if __name__ == "__main__":
    main()
