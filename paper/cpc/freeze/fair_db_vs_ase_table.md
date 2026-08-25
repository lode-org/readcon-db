# Fair ASE.db vs readcon-db (same CON ladder)

Fixture: `/tmp/readcon-db-fair/resources/test/tiny_cuh2.con`  Ladder: [10, 50, 100, 200, 500]

| N | rdb ins/s | ase ins/s | rdb ext/s | ase ext/s | rdb sel Cu (s) | ase sel Cu (s) | rdb8 (s) | ase8 (s) | Cu hits agree |
|---|-----------|-----------|-----------|-----------|----------------|----------------|----------|----------|---------------|
| 10 | 7.25e+03 | 5.64e+02 | 3.63e+06 | 9.00e+03 | 1.75e-06 | 9.02e-04 | 0.001 | 0.018 | yes |
| 50 | 3.66e+04 | 2.79e+03 | 4.96e+06 | 9.73e+03 | 4.92e-06 | 3.80e-03 | 0.001 | 0.048 | yes |
| 100 | 5.65e+04 | 3.54e+03 | 4.01e+06 | 1.05e+04 | 1.23e-05 | 7.68e-03 | 0.001 | 0.102 | yes |
| 200 | 9.40e+04 | 5.89e+03 | 5.49e+06 | 1.17e+04 | 1.75e-05 | 1.55e-02 | 0.001 | 0.335 | yes |
| 500 | 1.24e+05 | 8.53e+03 | 5.13e+06 | 1.09e+04 | 5.05e-05 | 3.91e-02 | 0.002 | 0.784 | yes |

Interchange (parse multi-frame CON):
```json
{
  "n_frames": 100,
  "repeats": 5,
  "readcon_mean_s": 0.00039528340566903354,
  "ase_io_mean_s": 0.011485619598533958,
  "readcon_frames_per_s": 252983.0460015033,
  "ase_io_frames_per_s": 8706.539437608064
}
```

Legacy `bench_ase_db.py` Cu2 stand-in timings remain **unequal-workload** artifacts; this file is the fair campaign.
