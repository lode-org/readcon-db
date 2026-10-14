Frame values are zstd level 1 of the CON text, written outside the LMDB transaction, and xxh3-128 is the decompressed text.
A single frame is the bytes 0xFD 0x01 plus one zstd frame, and an aligned run of 64 frames shares one 0xFD 0x03 zstd frame when that group is shorter than the per-frame values.
The other keys in the run store a 0xFD 0x02 redirect.
Untagged UTF-8 still reads as a 0.1.8 value.
There is no dictionary.
Ingest and migrate do not cook.
--no-compress stores the raw UTF-8.
migrate rewrites a legacy trajectory once and a second call writes 0 frames.
heed 0.21 cannot set the LMDB page size.
mdb_stat reports 4096 byte pages on these corpora.
On the 4000 frame sulfolene trajectory (13 atoms, 2,874,383 raw bytes) the per-frame zstd file was 4,284,416 bytes, 1.490 times the text.
Its frames table was 1,839,104 bytes (0.640 times the text), the secondary indexes were 2,002,944 bytes, the two hash tables were 417,792 bytes, and the freelist was 4,096 bytes.
mdb_copy -c of that file wrote 4,276,224 bytes, 8,192 bytes less, so free pages were not the 1.49 times ratio.
The same file with compression off is 6,569,984 bytes, and its frames table is 4,124,672 bytes (1.435 times the text).
After grouping, that compressed file is 2,707,456 bytes, 0.942 times the text.
The frames table is 262,144 bytes, 0.091 times the text.
The indexes stay 2,002,944 bytes, the hash tables stay 417,792 bytes, and the freelist stays 4,096 bytes.
On the 1000 frame 208 atom trajectory (7,637,000 raw bytes) per-frame zstd occupied 2,727,936 bytes, 0.357 times the text, with a frames table of 2,064,384 bytes (0.270 times the text).
The uncompressed file is 8,892,416 bytes and its frames table is 8,228,864 bytes, including 2,000 overflow pages.
After grouping, the compressed file is 962,560 bytes, 0.126 times the text, and the frames table is 299,008 bytes, 0.039 times the text.
Indexes stay 524,288 bytes and the hash tables stay 114,688 bytes.
The 8 shard gate of 8 copies of the sulfolene trajectory (32,000 frames, 22,995,064 raw bytes) is 22,507,520 bytes, 0.979 times the text, down from 35,143,680 bytes.
At 1 rank that ingest took 1.0745 s (29,780.1 frames/s) and 256 cold reads took 80.8 microseconds.
At 8 ranks it took 0.6308 s (50,729.1 frames/s) and the cold reads took 101.5 microseconds.
Single-writer and sharded canonical dumps match at 1 rank and at 8 ranks for batch sizes 100 and 4096.
