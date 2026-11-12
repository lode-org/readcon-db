"""Execute the runnable examples in the streaming guide."""

from pathlib import Path


def _runnable_blocks():
    org = (
        Path(__file__).resolve().parents[2]
        / "docs"
        / "orgmode"
        / "howto"
        / "streaming.org"
    )
    text = org.read_text(encoding="utf-8")
    blocks = []
    current = None
    for line in text.splitlines():
        if line.strip() == "#+begin_src python":
            current = []
            continue
        if current is not None and line.strip() == "#+end_src":
            body = "\n".join(current)
            if body.lstrip().startswith("# runnable"):
                blocks.append(body)
            current = None
            continue
        if current is not None:
            current.append(line)
    return blocks


def test_streaming_guide_example(tmp_path, monkeypatch):
    blocks = _runnable_blocks()
    assert blocks, "the streaming guide has no runnable example"
    monkeypatch.chdir(tmp_path)
    for block in blocks:
        exec(compile(block, "docs/orgmode/howto/streaming.org", "exec"), {})
