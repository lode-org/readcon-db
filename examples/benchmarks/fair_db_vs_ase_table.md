# Fair ASE.db vs readcon-db (same CON ladder)

Fixture: `resources/test/tiny_cuh2.con`  Ladder: [10, 50, 100, 200, 500]

| N | rdb ins/s | ase ins/s | rdb ext/s | ase ext/s | rdb sel Cu (s) | ase sel Cu (s) | rdb8 (s) | ase8 (s) | Cu hits agree |
|---|-----------|-----------|-----------|-----------|----------------|----------------|----------|----------|---------------|
| 10 | 3.34e+04 | 8.76e+02 | 1.48e+06 | 1.64e+03 | 4.44e-06 | 4.22e-03 | 0.002 | 0.061 | yes |
| 50 | 7.93e+04 | 2.81e+03 | 1.31e+06 | 2.55e+03 | 2.07e-05 | 1.39e-02 | 0.002 | 0.283 | yes |
| 100 | 8.45e+04 | 2.00e+03 | 1.57e+06 | 2.25e+03 | 2.75e-05 | 3.70e-02 | 0.003 | 0.558 | yes |
| 200 | 8.87e+04 | 3.40e+03 | 1.73e+06 | 1.88e+03 | 5.41e-05 | 1.20e-01 | 0.004 | 1.204 | yes |
| 500 | 5.02e+04 | 1.47e+03 | 1.01e+06 | 1.32e+03 | 2.99e-04 | 1.76e-01 | 0.010 | 1.945 | yes |

Interchange (parse multi-frame CON):
```json
{
  "n_frames": 100,
  "repeats": 5,
  "readcon_mean_s": 0.0006950007984414697,
  "ase_io_mean_s": 0.03377580000087619,
  "readcon_frames_per_s": 143884.72678628386,
  "ase_io_frames_per_s": 2960.6996724698115
}
```

Legacy `bench_ase_db.py` Cu2 stand-in timings remain **unequal-workload** artifacts; this file is the fair campaign.
