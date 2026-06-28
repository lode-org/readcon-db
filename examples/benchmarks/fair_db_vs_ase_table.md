# Fair ASE.db vs readcon-db (same CON ladder)

Fixture: `/home/rgoswami/Git/Github/LODE/readcon-core/resources/test/tiny_cuh2.con`  Ladder: [10, 50, 100, 200, 500]

| N | rdb ins/s | ase ins/s | rdb ext/s | ase ext/s | rdb sel Cu (s) | ase sel Cu (s) | rdb8 (s) | ase8 (s) | Cu hits agree |
|---|-----------|-----------|-----------|-----------|----------------|----------------|----------|----------|---------------|
| 10 | 1.46e+04 | 3.60e+02 | 5.28e+04 | 2.75e+02 | 1.18e-05 | 2.28e-02 | 0.015 | 0.154 | yes |
| 50 | 2.83e+04 | 3.43e+02 | 1.39e+05 | 4.32e+02 | 3.58e-05 | 8.80e-02 | 0.015 | 0.457 | yes |
| 100 | 5.50e+04 | 1.10e+03 | 6.61e+05 | 7.85e+02 | 4.62e-05 | 1.52e-01 | 0.020 | 1.315 | yes |
| 200 | 2.26e+04 | 4.31e+02 | 2.14e+05 | 2.89e+02 | 2.15e-04 | 5.54e-01 | 0.048 | 2.526 | yes |
| 500 | 4.79e+04 | 5.67e+02 | 6.41e+05 | 5.08e+02 | 2.80e-04 | 1.03e+00 | 0.041 | 6.428 | yes |

Interchange (parse multi-frame CON):
```json
{
  "n_frames": 100,
  "repeats": 5,
  "readcon_mean_s": 0.001410490000853315,
  "ase_io_mean_s": 0.06772762939799577,
  "readcon_frames_per_s": 70897.3476873301,
  "ase_io_frames_per_s": 1476.5022914408878
}
```

Legacy `bench_ase_db.py` Cu2 stand-in timings remain **unequal-workload** artifacts; this file is the fair campaign.
