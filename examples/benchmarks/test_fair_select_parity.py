#!/usr/bin/env python3
"""Assert ASE vs readcon-db hit counts agree on shared CON (symbol Cu, natoms range)."""
from pathlib import Path
import tempfile
import sys

# allow running without pytest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from fair_campaign import (
    DEFAULT_FIXTURE,
    write_ladder_con,
    bench_readcon_db,
    bench_ase_db,
)

def main():
    assert DEFAULT_FIXTURE.is_file(), DEFAULT_FIXTURE
    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        n = 20
        con = td / "n20.con"
        write_ladder_con(DEFAULT_FIXTURE, n, con)
        r = bench_readcon_db(con, n, td / "rdb")
        a = bench_ase_db(con, n, td / "ase.db")
        assert r["hit_symbol_Cu"] == a["hit_symbol_Cu"] == n, (r, a)
        assert r["hit_natoms_1_10000"] == a["hit_natoms_1_10000"] == n, (r, a)
        assert r["hit_mass_window"] == a["hit_mass_window"] == n, (r, a)
        assert r["hit_volume_window"] == a["hit_volume_window"] == n, (r, a)
        assert r.get("energy_select_skipped") is True and a.get("energy_select_skipped") is True
        assert r.get("select_energy_present_mean_s") is None
        assert a.get("select_energy_present_mean_s") is None
        print(
            "ok",
            {
                "n": n,
                "cu": r["hit_symbol_Cu"],
                "natoms": r["hit_natoms_1_10000"],
                "mass": r["hit_mass_window"],
                "volume": r["hit_volume_window"],
            },
        )

if __name__ == "__main__":
    main()
