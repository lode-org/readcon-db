# CON major-4 fixtures

Written by readcon-core at commit
`8bba6f59ced996784ba713250603dbbdd8220a75` (develop, crate version
0.24.0, unreleased) with the default `ConFrameBuilder`, which stamps
`con_spec_version` 4, `con_spec_minor` 0 and `con_spec_patch` 4.
The files are writer output and are not edited by hand.

| File                  | Content                                |
|-----------------------|----------------------------------------|
| `tiny_cuh2_v4.con`    | the `tiny_cuh2` frame, no velocities   |
| `tiny_cuh2_v4.convel` | the same frame with a velocity section |

readcon-core 0.23.2 and older reject a major above 3 with
`UnsupportedVersion`. `tests/con_v4_fixtures.rs` records that.

These files live outside `resources/test` itself because tests
ingest every `.con` and `.convel` there.

## Regenerating

Build a scratch crate outside this repository that depends on that
readcon-core commit by path, and run:

```rust
use readcon_core::types::ConFrameBuilder;
use readcon_core::writer::write_frames_to_string;

const ATOMS: [(&str, [f64; 3], bool, u64, f64, [f64; 3]); 4] = [
    ("Cu", [0.6394, 0.9045, 6.9753], true, 0, 63.546, [0.001234, 0.002345, -0.003456]),
    ("Cu", [3.1969, 0.9045, 6.9753], true, 1, 63.546, [0.004567, -0.005678, 0.006789]),
    ("H", [8.6823, 9.947, 11.733], false, 2, 1.00793, [-0.012345, 0.023456, 0.034567]),
    ("H", [7.9421, 9.947, 11.733], false, 3, 1.00793, [0.045678, -0.056789, -0.06789]),
];

fn build(with_vel: bool) -> String {
    let mut b = ConFrameBuilder::new([15.3456, 21.702, 100.0], [90.0; 3]);
    b.prebox_header("Random Number Seed");
    b.postbox_header(["0 0".into(), "218 0 1".into()]);
    for (sym, p, fixed, id, m, v) in ATOMS {
        b.add_atom(sym, p[0], p[1], p[2], [fixed; 3], id, m);
        if with_vel {
            b.with_velocity(v);
        }
    }
    write_frames_to_string(&[b.build().unwrap()]).unwrap()
}
```

`build(false)` is `tiny_cuh2_v4.con` and `build(true)` is
`tiny_cuh2_v4.convel`. SHA-256:

```
b74407265bf288f15493d6f92485662c83f032a283164d867ba6d35ebeddc861  tiny_cuh2_v4.con
195660843d3f2fc0e35d1063dda8d2a286c50c91e33d84cf8f63f5592068a6c4  tiny_cuh2_v4.convel
```
