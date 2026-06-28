# Fair ASE.db vs readcon-db (same CON ladder)

Fixture: `/home/rgoswami/Git/Github/LODE/readcon-core/resources/test/tiny_cuh2.con`  Ladder: [10, 50, 100, 200, 500]

| N | rdb ins/s | ase ins/s | rdb ext/s | ase ext/s | rdb sel Cu (s) | ase sel Cu (s) | rdb8 (s) | ase8 (s) | Cu hits agree |
|---|-----------|-----------|-----------|-----------|----------------|----------------|----------|----------|---------------|
| 10 | 2.27e+04 | 1.21e+03 | 5.63e+05 | 1.19e+03 | 7.78e-06 | 1.02e-02 | 0.002 | 0.111 | yes |
| 50 | 4.14e+04 | 8.57e+02 | 5.88e+05 | 1.08e+03 | 2.60e-05 | 4.97e-02 | 0.007 | 0.459 | yes |
| 100 | 3.80e+04 | 9.67e+02 | 4.74e+05 | 6.66e+02 | 6.35e-05 | 9.44e-02 | 0.019 | 0.927 | yes |
| 200 | 3.76e+04 | 8.22e+02 | 4.69e+05 | 1.04e+03 | 1.42e-04 | 2.09e-01 | 0.015 | 2.057 | yes |
| 500 | 2.86e+04 | 5.35e+02 | 3.79e+05 | 7.74e+02 | 4.16e-04 | 4.86e-01 | 0.021 | 4.750 | yes |

Interchange (parse multi-frame CON):
```json
{
  "n_frames": 100,
  "repeats": 5,
  "readcon_mean_s": 0.0042307758005335925,
  "ase_io_mean_s": 0.13373256099876016,
  "readcon_frames_per_s": 23636.326932613123,
  "ase_io_frames_per_s": 747.7610482680213
}
```

Legacy `bench_ase_db.py` Cu2 stand-in timings remain **unequal-workload** artifacts; this file is the fair campaign.
