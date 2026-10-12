# Appendix C measurements

Five paired, interleaved repeats of ingest and of three queries. Each series
is the median, the minimum, and the maximum of those five. A megabyte is
1e6 bytes. Threads were pinned to 1. Every timed query checked the hit count
and the sum of frame indexes before the sample was kept. All 20 ingests and
all 180 queries passed that check.

The timed corpora are 1000 and 10000 frames. A repeat at 100000 frames does
not finish in two minutes: `ase.db` ingest is about 200 frames per second, so
that ingest alone is several minutes. One million frames does not fit in the
2 GiB LMDB map (`MDB_MAP_FULL`). Those sizes are not in the tables.

## Machine

Intel Xeon, 8 CPUs, 47.07 GiB RAM. Python 3.12.3, rustc 1.88.0, readcon-db
0.1.8, readcon-core 0.16.2, ASE 3.29.0, numpy 2.5.4. `page_cache_drop` is
false: cold queries are a `posix_fadvise` `DONTNEED` hint, then a fresh
process. Warm is the next query in that process.

## Workloads

ASE EMT searches, same atom order on both ends of HCN/HNC (H, C, N): HCN/HNC
NEB (157 frames), Cu adatom NEB (159), Pt–H NEB (83), Pt adatom dimer (71).
The aKMC trace stores the current state and the saddle at each of 4000 steps
for three systems (24000 frames). Hop counts: HCN/HNC 0 (saddle 197.6 eV;
EMT on H, C, and N is not a quantitative barrier), Cu 0 (saddle 5.23 eV,
barrier about 1.09 eV), Pt–H 1664 (barrier about 0.03 eV). eOn was not used.

The scale files repeat that ensemble. Both timed corpora have as many distinct
xxHash3-128 values as frames (1000 and 10000), so the ingest timing is not a
deduplication win. The composition query is `Cu:9` (292 hits at 1000 frames,
index sum 135816; 2542 hits at 10000 frames, index sum 12511844). Energy and
max-force windows sit in gaps between samples. The oracle rejected a window
on which the per-atom Euclidean maximum and `sqrt` of the maximum sum of
squares would disagree, and both oracles were accepted.

`from_ase` leaves the user line empty. An empty user line is legal CON. The
generator writes the user line `cpc-eval` so `readcon.iter_con` can read the
corpus. That is a workaround for the reader bug in
`eval/bugs/empty-user-line.md`, not a change to the CON rules.

## Correctness

Nine checks, eight pass, one fail. The failure is listed under bugs and was
not fixed.

| check | pass |
| --- | --- |
| round_trip | yes |
| dedup_identical_bytes | yes |
| one_byte_change | yes |
| missing_forces_or_velocities | yes |
| periodicity_not_inferred | yes |
| rebuild_from_text | yes |
| reindex_twice | yes |
| numeric_cache_ignored | yes |
| hash_index_survives_rewrite_of_duplicate | no |

Identical bytes share one hash-index entry and each frame still keeps its own
text. A one-digit coordinate change produces a new digest; both digests remain
findable. `reindex` rebuilds secondary indexes from the stored CON text and
leaves `frames_soa` in place. A cell without metadata `pbc` matches no
periodicity mask.

## Ingest

The clock starts before `append_trajectory_path` (or the ASE write loop) and
stops after a composition check. The directory of CON files has no ingest
step. System order rotates on each repeat.

| frames | system | seconds | frames/s | MB/s |
| --- | --- | --- | --- | --- |
| 1000 | readcon-db | 0.03252 (0.02893–0.03839) | 30748 (26051–34562) | 33.46 (28.35–37.61) |
| 1000 | ase.db | 4.918 (4.702–5.710) | 203.4 (175.1–212.7) | 0.2213 (0.1906–0.2314) |
| 10000 | readcon-db | 0.3017 (0.2818–0.3508) | 33150 (28507–35480) | 36.21 (31.14–38.76) |
| 10000 | ase.db | 51.76 (49.83–55.98) | 193.2 (178.6–200.7) | 0.2110 (0.1951–0.2192) |

Each cell is median (minimum–maximum). The sum of recorded child times in one
repeat is 5.0–6.1 s at 1000 frames and 52.6–58.8 s at 10000 frames.

Figures: `eval/results/figures/ingest_throughput.pdf`,
`eval/results/figures/ingest_bandwidth.pdf`. Error bars run from the minimum
to the maximum.

## Queries

Rust `Select` for readcon-db does not add a natoms predicate. ASE queries use
the bit-exact oracle bounds. Cold is the first query after the fadvise hint.
Warm is the second query in the same process. Cells are seconds, median
(minimum–maximum).

| frames | query | cache | readcon-db | ase.db | CON directory |
| --- | --- | --- | --- | --- | --- |
| 1000 | composition | cold | 0.001307 (0.000948–0.001926) | 0.04175 (0.03874–0.05259) | 0.02120 (0.01703–0.04212) |
| 1000 | composition | warm | 2.317e-5 (2.113e-5–2.391e-5) | 0.03207 (0.03103–0.03417) | 0.01393 (0.01386–0.02067) |
| 1000 | energy | cold | 0.001898 (0.001387–0.002809) | 0.03040 (0.02821–0.03222) | 0.02074 (0.02018–0.02145) |
| 1000 | energy | warm | 2.251e-5 (1.854e-5–2.670e-5) | 0.02215 (0.02157–0.02325) | 0.01395 (0.01365–0.01486) |
| 1000 | fmax | cold | 0.001575 (0.000608–0.002308) | 0.03050 (0.02672–0.03956) | 0.02017 (0.01981–0.02049) |
| 1000 | fmax | warm | 1.970e-5 (1.762e-5–2.408e-5) | 0.02160 (0.02135–0.02335) | 0.01406 (0.01373–0.01511) |
| 10000 | composition | cold | 0.008143 (0.005123–0.01072) | 0.2859 (0.2758–0.2884) | 0.1928 (0.1790–0.2406) |
| 10000 | composition | warm | 0.000176 (0.000175–0.000184) | 0.2580 (0.2576–0.2617) | 0.1398 (0.1368–0.1425) |
| 10000 | energy | cold | 0.01011 (0.008108–0.01444) | 0.2231 (0.2166–0.2367) | 0.1927 (0.1887–0.2074) |
| 10000 | energy | warm | 0.000211 (0.000206–0.000215) | 0.2033 (0.1995–0.2087) | 0.1375 (0.1368–0.1694) |
| 10000 | fmax | cold | 0.009570 (0.007696–0.01160) | 0.2245 (0.2196–0.2339) | 0.1968 (0.1677–0.1986) |
| 10000 | fmax | warm | 0.000208 (0.000205–0.000212) | 0.2062 (0.2033–0.2163) | 0.1379 (0.1371–0.1410) |

Figure: `eval/results/figures/query_latency.pdf`.

Energy windows (eV), from the oracle bit patterns: 4.204617021467001 to
4.521918223411915 at 1000 frames (201 hits); 3.582985061659346 to
4.343762117629726 at 10000 frames (2001 hits). Max-force windows (eV/Å):
0.7997694584862253 to 1.3970191198007356 (201 hits); 0.9288952799164887 to
1.571054798023666 (2001 hits).

## Disk

Allocated bytes. For readcon-db and ase.db the allocated figure is the median
of the five ingests; the minimum and the maximum are in parentheses when they
differ. On this filesystem the readcon-db file size equals the allocated size.
The 2 GiB value is the map ceiling, not the file.

| store | 1000 frames | 10000 frames |
| --- | --- | --- |
| raw CON | 1088101 | 10923745 |
| gzip level 6 | 277258 | 3241302 |
| zstd level 3 | 270863 | 3312800 |
| CON directory | 1134592 | 11501568 |
| readcon-db | 2723840 | 27340800 |
| ase.db | 966656 (962560–970752) | 8921088 (8912896–8929280) |

readcon-db is about 2.5 times the raw CON text (text plus indexes). ase.db is
smaller than the raw text. zstd level 3 is smaller than gzip level 6 at 1000
frames and larger at 10000 frames.

Figure: `eval/results/figures/disk_footprint.pdf`.

## Repeated states

One ingest of `akmc_trace.con` (24000 frames, 27864917 bytes). The hash index
keeps the first key for each xxHash3-128. Stored texts are not shared: stored
blob bytes equal the raw file (27864917). Unique hashes: 7. Unique text: 8319
bytes (0.0002985 of the stored blobs). A corpus of those seven texts allocates
98304 bytes; the full corpus allocates 74911744 bytes.

The seven texts are the unhopped HCN state and its saddle, the unhopped Cu
state and its saddle, and both Pt–H states plus that saddle.

Figure: `eval/results/figures/dedup.pdf`.

## Bugs left as they are

1. Rewriting one copy of a duplicated frame deletes the hash-index entry even
   though another frame still holds those bytes. After `set_trajectory_units`
   on trajectory 1, trajectory 2 is still stored and `find_by_hash` returns
   nothing. `reindex` restores the entry to trajectory 2, frame 0. This is the
   failed correctness check.
2. Python `ConCorpus.select` always applies `natoms_range`, including the
   default `0` to `u32::MAX` (`src/python.rs`). Every Python query therefore
   walks `idx_natoms`. The timed readcon-db queries use the Rust `Select` API
   and do not add that predicate.
3. `readcon.iter_con` (`PyConFrameIterator::__next__` in readcon-core
   `src/python.rs`) skips a leading newline, so an empty user line shifts the
   header and the parse fails. `read_con` on one such frame keeps the empty
   user line. Two concatenated frames also break `count_frames` and velocity
   parsing. Reproducer: `eval/bugs/empty-user-line.con`. The evaluation user
   line `cpc-eval` is only a workaround.

## Limits

Ingest parallelism in a directory load was not used; the thread cap is 1.
Cold times are fadvise hints, not a guaranteed cache drop. The LMDB map stays
at 2 GiB. ASE's database lock waits forever if a dead process leaves
`*.db.lock`; the ASE ingest helper removes that lock before connect.
