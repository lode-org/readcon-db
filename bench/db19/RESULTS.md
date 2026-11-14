# db19 coordinate codec, not landed

traj-100k is 342 MB because the cooked columns are raw f64. Eight
synthetic 100k-atom frames, release build, one core:

| codec | 8 frames, bytes | decode of one frame |
| --- | --- | --- |
| raw RCSO | 38401120 | from_blob 485 µs |
| zstd level 1 | 1297861 |  |
| byte-shuffle then zstd level 1 | 136081 | zstd 1785 µs, unshuffle 1579 µs, both plus from_blob 4071 µs |
| XOR with the previous frame, then zstd 1 | 449296 | needs every earlier frame |
| 64 KiB raw dictionary plus shuffle | 141062 | no better than shuffle alone |

Shuffle plus zstd is bit-exact and about 280 times smaller on these
frames. A dictionary does not beat it. XOR with the previous frame is
larger and a selected read would walk the earlier frames.

The same codec was then put on the real read path and measured
against the db18 binary. Each build read the corpus it wrote. Samples
alternate db18 then db19, n=12, same host, fair cold and warm rows.
Cold drops the file until mincore is 0 and opens outside the clock.

| | db18 bytes | shuffle bytes |
| --- | --- | --- |
| traj-100k file | 341966848 | 35717120 |

| op | cache | db18 median s | shuffle median s | ratio |
| --- | --- | --- | --- | --- |
| full_read | cold | 7.4836e-01 | 6.7707e-01 | 0.905 |
| selected | cold | 6.3222e-03 | 5.8118e-03 | 0.919 |
| range | cold | 1.7918e-01 | 1.7503e-01 | 0.977 |
| full_read | warm | 3.7774e-01 | 5.8490e-01 | 1.548 |
| selected | warm | 1.3520e-03 | 4.5889e-03 | 3.394 |
| range | warm | 9.4428e-02 | 1.5033e-01 | 1.592 |

Cold is a little faster because the file is smaller. Warm selected is
3.4 times slower, warm range 1.6, warm full 1.5. That is not noise.
The codec is not in this tree.

ASE on the db18 campaign, same host, traj-100k file 384503808 bytes:
full cold 2.414 s, selected cold 3.627e-2 s, range cold 6.025e-1 s,
full warm 1.928 s, selected warm 3.013e-2 s, range warm 4.930e-1 s.
The shuffle build would still beat those, and it still loses the warm
rows to db18.
