# CPC companion positioning

The Computer Physics Communications manuscript is the
[readcon-core](https://github.com/lode-org/readcon-core) paper (CON
specification and the hourglass reader/writer). **readcon-db** is the
companion campaign store in that article, not a second CPC claim and
not a second paper DOI.

Program-summary wording in the core manuscript already names this
crate as the companion store: LMDB indexes (energy, formula, section
presence) and a derived RCSO cache over the same CON text. Deposit
core and db freeze tags together.

## Appendix timings (if used)

Main-claim performance tables in the CPC paper are core parse /
Cachegrind / equal-geometry wall numbers. A store-comparison
appendix, if the manuscript includes one, uses the **frozen fair
campaign** in `paper/cpc/freeze/`:

| Item | Value |
|------|--------|
| Script | `examples/benchmarks/fair_campaign.py` |
| Fixture | `resources/test/tiny_cuh2.con` |
| Ladder | 10, 50, 100, 200, 500 |
| JSON | `paper/cpc/freeze/ase_fair_campaign_1.json` |
| Table | `paper/cpc/src/figures/generated/fair_campaign_table.tex` |
| Refresh tree | `4bef664` |

Both stores see the same CON frames (readcon geometry → ASE `Atoms`).
Select hit counts agree on symbol, natoms, mass, and volume. The
committed JSON does not record host or clock.

Legacy `examples/benchmarks/bench_ase_db.py` Cu2 stand-ins are
**unequal-workload** artifacts. They are not the appendix freeze.

```bash
python paper/cpc/scripts/gen_fair_table.py --check
```

Re-running the campaign writes a new JSON under `--out`. That does
not move the freeze. See `paper/cpc/readme.org`.
