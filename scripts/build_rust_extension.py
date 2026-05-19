#!/usr/bin/env python
from __future__ import annotations

import shutil
import subprocess
import sysconfig
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUST_CRATE = ROOT / "rust" / "Cargo.toml"
TARGET_DIR = ROOT / "rust" / "target" / "release"
PYTHON_PACKAGE = ROOT / "src" / "image_similarity"


def main() -> int:
    subprocess.run(
        ["cargo", "build", "--manifest-path", str(RUST_CRATE), "--release"],
        check=True,
    )

    extension_suffix = sysconfig.get_config_var("EXT_SUFFIX")
    if not extension_suffix:
        raise RuntimeError("Python did not report an extension suffix")

    source = TARGET_DIR / "lib_rust.so"
    destination = PYTHON_PACKAGE / f"_rust{extension_suffix}"
    shutil.copy2(source, destination)
    print(destination.relative_to(ROOT))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
