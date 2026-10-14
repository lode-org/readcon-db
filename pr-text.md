Per-frame zstd level 1 stores each CON span in the existing sharded LMDB as the bytes 0xFD 0x01 plus one stock zstd frame.
Untagged UTF-8 still reads as a 0.1.8 value, and a tagged frame that is not shorter than the text is stored as that UTF-8.
Compression runs in the prepare step, outside the write transaction.
Both MPI ingest routes send the stored bytes, and xxh3-128 is computed on the decompressed text.
There is no dictionary, and ingest and migrate do not cook frames_soa.
readcon-db migrate rewrites a legacy value once and leaves an already tagged value unchanged, and --no-compress stores the raw UTF-8.
A round trip of resources/test/sulfolene.con is byte exact, a corpus written with compression off reads as 0.1.8 and migrates once, and a second migrate rewrites 0 frames.
Single-writer canonical dumps of 6 files and 4006 frames at 1 rank and at 8 ranks match, sha256 373dc9e68b3726061f7bc4ab1532b1fcb39feb492d37096e9842b6845d8620c1.
Sharded canonical dumps of those same files at 1 rank and at 8 ranks match, sha256 12724e9a70df3b8c84c475d2cfa077d66608229274b667f2e4fa8b4a15918429.
The container gate ingested 8 copies of a 4000-frame varying sulfolene trajectory (2,874,383 bytes each, 32,000 frames, 8 shards) and then cold-read 256 frames after posix_fadvise DONTNEED.
At 1 rank ingest took 0.7477 s (42,799.8 frames/s) and those cold reads took 113.0 microseconds each, returning 183,960 decompressed bytes.
At 8 ranks ingest took 0.3151 s (101,550.0 frames/s) and the same 256 cold reads took 118.9 microseconds each.
The eight shard data.mdb files totaled 35,143,680 bytes against 22,995,064 bytes of raw CON.
Sharded ingest rose from 42,799.8 to 101,550.0 frames/s (2.37 times), above the published segment rate of 19.9k frames/s at 8 ranks, so the segment follow-on stays closed.
