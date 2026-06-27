"""Build a multi-frame CON corpus on disk for benchmarks (concat tiny frames)."""
import pathlib, sys
root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "../readcon-core/resources/test")
out = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else "/tmp/bench_frames")
out.mkdir(parents=True, exist_ok=True)
single = (root / "tiny_cuh2.con").read_text()
for n in [10, 50, 100, 200, 500]:
    (out / f"n{n}.con").write_text(single * n)
print("wrote", out)
