# readcon-core 0.16.2: streaming reader drops an empty user line

CON header line 1 is free text and may be empty. The Python streaming
reader treats a leading newline as spacing to skip, so the JSON line is
parsed as the user line and the rest of the header shifts.

## Where

- Crate: `readcon-core` 0.16.2 (Python package `readcon` 0.16.2).
- Entry point: `readcon.iter_con`.
- Function: `PyConFrameIterator::__next__` in `src/python.rs`.
- The loop before `ConFrameIterator::next_with_raw_span` advances `pos`
  while the byte is `\n`, `\r`, or space. The comment says it skips
  leading blank lines between frames.

`readcon.read_con` and `readcon.read_con_string` call
`frames_from_text`, which uses `ConFrameIterator::next`. That path does
not skip the first line. A one-frame file with an empty user line parses,
and the user line is the empty string.

## Minimal frame

The bytes are `eval/bugs/empty-user-line.con`. The file begins with a
newline. The remaining lines are `resources/test/tiny_cuh2.con` without its
user line.

## What happens

`readcon.iter_con` on that one frame raises:

```
OSError: parse error: expected 3 values on line, found 2
```

The shift is:

1. The leading newline is discarded.
2. The JSON object becomes the user line.
3. The box line does not start with `{`, so the parser takes the legacy
   path and does not read metadata.
4. The angles line is accepted as the three box lengths.
5. The postbox line `0 0` is parsed as three angles and has two values.

`readcon.read_con` on the same file returns one frame whose user line
is empty.

`readcon.count_frames` on that one frame returns 1. It uses
`ConFrameIterator::forward_fast`, which counts the empty line as line 1
of the header.

## Second frame

Concatenating the same text twice makes the empty user line of frame 2
a blank line after frame 1.

- `readcon.count_frames` (`ConFrameIterator::forward_fast`) raises
  `invalid number format: invalid digit found in string`. After the
  coordinate block, a blank line is consumed as another section.
- `readcon.read_con` (`ConFrameIterator::next`) raises
  `file ended unexpectedly while reading velocity section`. The same
  blank line starts velocity parsing.

A conformance test should accept one frame and two frames when line 1
is empty, and it should keep that empty line as the user line.
