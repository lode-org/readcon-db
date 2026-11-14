#!/usr/bin/env python3
"""Build the readcon-db C ABI via cargo rustc and copy artifacts.

Used by meson.build. Headers are shipped; this script never invokes cbindgen.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    if len(argv) not in (9, 10):
        print(
            "usage: meson_cargo_build.py CARGO SRC_ROOT TARGET_DIR PROFILE "
            "SHARED_NAME STATIC_NAME OUT_SHARED OUT_STATIC [FEATURES]",
            file=sys.stderr,
        )
        return 2
    (
        cargo,
        src_root,
        target_dir,
        profile,
        shared_name,
        static_name,
        out_shared,
        out_static,
    ) = argv[1:9]
    features = argv[9] if len(argv) == 10 else ""
    src_root_p = Path(src_root)
    cmd = [
        cargo,
        "rustc",
        "--lib",
        "--manifest-path",
        str(src_root_p / "Cargo.toml"),
        "--target-dir",
        target_dir,
    ]
    if (src_root_p / "Cargo.lock").is_file():
        cmd.append("--locked")
    if (src_root_p / "vendor").is_dir():
        cmd.append("--offline")
    if profile == "release":
        cmd.append("--release")
    env = os.environ.copy()
    link_args: list[str] = []
    if sys.platform.startswith("linux"):
        link_args.append(f"-Clink-arg=-Wl,-soname,{shared_name}")
    elif sys.platform == "darwin":
        link_args.append(f"-Clink-arg=-Wl,-install_name,@rpath/{shared_name}")
    # The cdylib uses mimalloc local-dynamic TLS so a later dlopen does not
    # need static TLS surplus. The staticlib keeps initial-exec.
    for crate_type in ("cdylib", "staticlib"):
        crate_features = features.strip()
        if crate_type == "cdylib":
            crate_features = ",".join(
                part for part in (crate_features, "local_dynamic_tls") if part
            )
        crate_cmd = list(cmd)
        if crate_features:
            crate_cmd.extend(["--features", crate_features])
        crate_cmd.extend(["--", f"--crate-type={crate_type}", *link_args])
        subprocess.check_call(crate_cmd, cwd=src_root, env=env)
        built = Path(target_dir) / profile
        if crate_type == "cdylib":
            shutil.copy2(built / shared_name, out_shared)
        else:
            shutil.copy2(built / static_name, out_static)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
