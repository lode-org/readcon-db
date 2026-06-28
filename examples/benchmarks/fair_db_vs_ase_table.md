# Fair ASE.db vs readcon-db (same CON ladder)

Fixture: `/home/rgoswami/Git/Github/LODE/readcon-core/resources/test/tiny_cuh2.con`  Ladder: [10, 50, 100, 200, 500]

| N | rdb ins/s | ase ins/s | rdb ext/s | ase ext/s | rdb sel Cu (s) | ase sel Cu (s) | rdb8 (s) | ase8 (s) | Cu hits agree |
|---|-----------|-----------|-----------|-----------|----------------|----------------|----------|----------|---------------|
| 10 | 2.21e+04 | 7.21e+02 | 5.72e+05 | 9.37e+02 | 8.53e-06 | 4.36e-03 | 0.018 | 0.065 | yes |
| 50 | 6.88e+04 | 1.71e+03 | 9.91e+05 | 1.60e+03 | 1.47e-05 | 2.86e-02 | 0.013 | 0.282 | yes |
| 100 | 9.32e+04 | 1.77e+03 | 1.22e+06 | 1.28e+03 | 2.50e-05 | 5.12e-02 | 0.013 | 0.572 | yes |
| 200 | 9.70e+04 | 3.34e+03 | 1.19e+06 | 1.61e+03 | 5.43e-05 | 1.04e-01 | 0.015 | 1.401 | yes |
| 500 | 5.37e+04 | 2.74e+03 | 6.33e+05 | 1.77e+03 | 2.89e-04 | 1.98e-01 | 0.015 | 1.925 | yes |

Interchange (parse multi-frame CON):
```json
{
  "n_frames": 100,
  "repeats": 5,
  "readcon_mean_s": 0.0012939456006279214,
  "ase_io_mean_s": 0.04245459820085671,
  "readcon_frames_per_s": 77283.00165901282,
  "ase_io_frames_per_s": 2355.4574589751282
}
```

Legacy `bench_ase_db.py` Cu2 stand-in timings remain **unequal-workload** artifacts; this file is the fair campaign.
