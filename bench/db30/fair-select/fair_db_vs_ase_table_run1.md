# Fair ASE.db vs readcon-db (same CON ladder)

Fixture: `resources/test/tiny_cuh2.con`  Ladder: [10, 50, 100, 200, 500]

| N | rdb ins/s | ase ins/s | rdb ext/s | ase ext/s | rdb sel Cu (s) | ase sel Cu (s) | rdb8 (s) | ase8 (s) | Cu hits agree |
|---|-----------|-----------|-----------|-----------|----------------|----------------|----------|----------|---------------|
| 10 | 1.64e+03 | 4.68e+02 | 1.76e+05 | 4.76e+03 | 8.34e-06 | 1.46e-03 | 0.005 | 0.029 | yes |
| 50 | 1.44e+04 | 2.64e+03 | 1.17e+05 | 6.71e+03 | 7.48e-06 | 6.21e-03 | 0.006 | 0.125 | yes |
| 100 | 1.09e+04 | 3.68e+03 | 1.13e+05 | 6.89e+03 | 1.32e-05 | 1.21e-02 | 0.010 | 0.223 | yes |
| 200 | 2.57e+04 | 4.66e+03 | 1.06e+05 | 6.83e+03 | 2.40e-05 | 2.45e-02 | 0.019 | 0.450 | yes |
| 500 | 3.03e+04 | 6.18e+03 | 1.05e+05 | 6.96e+03 | 5.95e-05 | 6.11e-02 | 0.042 | 1.275 | yes |

Interchange (parse multi-frame CON):
```json
{
  "n_frames": 100,
  "repeats": 5,
  "readcon_mean_s": 0.0001688446000116528,
  "ase_io_mean_s": 0.014897510199989483,
  "readcon_frames_per_s": 592260.5756600952,
  "ase_io_frames_per_s": 6712.5310644238125
}
```

Legacy `bench_ase_db.py` Cu2 stand-in timings remain **unequal-workload** artifacts; this file is the fair campaign.
