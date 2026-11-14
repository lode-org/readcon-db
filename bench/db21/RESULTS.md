# db21 shuffle-zstd stays opt-in

The first db21 warm selected row was not a codec measurement. Shuffle
read one frame in a loop and hit a one-frame cache. Raw, db18, and ASE
decoded that frame every time. The cache is gone. It added a mutex and
up to 8 MiB on the connection, and it does not help a read that never
repeats a frame.

Warm selected is now one pass over every frame. The index is
`i * 17 mod 64`, the same sequence on every arm. 17 is coprime to 64,
so each frame is read once and the order is not a contiguous range.
The recorded time is the mean per frame. Pages stay warm from the open.
The clock is that pass once, so a short call is not repeated into a
second cycle. Range and full are one multi-frame call each. Nothing in
those paths can return a cached frame, because there is no frame cache.

A release profile of one 100k-atom frame (positions and forces, 4.80 MB)
still says where the old 2.9 ms went. The db20 interleaved decode was
2246 µs: zstd 1643 µs, unshuffle 287 µs, a new context 1 µs. Allocating
and zeroing the columns before the unshuffle was 465 µs, and a 4.80 MB
copy was 294 µs. Raw `from_blob` was 500 µs. Component-major zstd at
level 3, the layout this tree writes, is 12750 bytes for that frame and
1337 µs for the function that fills the caller's columns. Independent
blocks of 512 to 32768 values were slower. Level -1 and -5 were larger
and no faster.

n=12, serial, same host. At each index the order is the db18 binary,
this tree with `READCON_DB_COOKED_CODEC=raw`, this tree with
`shuffle-zstd`, then ASE. Cold drops the file until mincore is 0 and
opens outside the clock. A slower row is inside the noise when the
relative gap is no larger than the wider IQR/median, and no larger than
0.05 when both spreads are tighter than that.

Warm selected is 1.20 times raw. Its IQR/median is 0.01 on raw and 0.05
on shuffle, so a 20% gap is outside the noise. That row keeps the codec
opt-in. `READCON_DB_COOKED_CODEC=shuffle-zstd` selects it. Unset and
`raw` store RCSO. `FB 03` blobs from db20 still decode. Blobs under
64 KiB, and cooked groups, stay raw, so db15 through db19 files stay
readable.

Warm full is 0.75 times raw and warm range is 0.64 times raw. Both are
faster, and neither one consults a cache. Cold reads and the insert are
faster too. The size win remains.

| | db18 | raw | shuffle-zstd | ASE |
| --- | --- | --- | --- | --- |
| traj-100k file | 341966848 | 341966848 | 35782656 | 384503808 |
| `frames_soa` values | 307208960 | 307208960 | 817047 |  |

| op | cache | db18 s | raw s | shuffle s | ASE s | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| insert | | 1.3247e+00 | 1.2165e+00 | 1.0594e+00 | 1.4084e+00 | 0.871 |
| full_read | cold | 6.4616e-01 | 6.7567e-01 | 2.7248e-01 | 2.1469e+00 | 0.403 |
| selected | cold | 5.3386e-03 | 4.9955e-03 | 2.2844e-03 | 2.8405e-02 | 0.457 |
| range | cold | 2.1145e-01 | 1.9552e-01 | 7.9437e-02 | 4.9744e-01 | 0.406 |
| full_read | warm | 4.2147e-01 | 4.0517e-01 | 3.0473e-01 | 1.8554e+00 | 0.752 |
| selected | warm | 1.6680e-03 | 1.6704e-03 | 2.0071e-03 | 2.3750e-02 | 1.202 |
| range | warm | 1.1044e-01 | 1.0836e-01 | 6.9763e-02 | 4.2553e-01 | 0.644 |

Warm selected is the mean of one pass over 64 distinct frames.
