#!/usr/bin/env python3
"""Build the TongGraph PyO3 extension in-place for local testing."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import sysconfig
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true", help="build in release mode")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    build_args = ["cargo", "build", "--features", "extension-module"]
    if args.release:
        build_args.append("--release")
    subprocess.run(build_args, cwd=root, check=True)

    profile = "release" if args.release else "debug"
    target_dir = root / "target" / profile
    source = find_extension_artifact(target_dir)
    suffix = sysconfig.get_config_var("EXT_SUFFIX")
    if not suffix:
        suffix = ".pyd" if sys.platform == "win32" else ".so"
    destination = root / "python" / "tonggraph" / f"_tonggraph{suffix}"
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    sign_extension(destination)
    print(f"copied {source} -> {destination}")

    generic_suffix = ".pyd" if sys.platform == "win32" else ".so"
    generic_destination = root / "python" / "tonggraph" / f"_tonggraph{generic_suffix}"
    if generic_destination != destination:
        shutil.copy2(source, generic_destination)
        sign_extension(generic_destination)
        print(f"copied {source} -> {generic_destination}")


def find_extension_artifact(target_dir: Path) -> Path:
    suffixes = {".so", ".dylib", ".dll", ".pyd"}
    candidates = [
        path
        for path in target_dir.iterdir()
        if "tonggraph" in path.name and path.suffix in suffixes
    ]
    if not candidates:
        raise FileNotFoundError(f"no TongGraph extension artifact found in {target_dir}")
    candidates.sort(key=lambda path: path.stat().st_mtime, reverse=True)
    return candidates[0]


def sign_extension(path: Path) -> None:
    if sys.platform != "darwin":
        return
    subprocess.run(["codesign", "--force", "--sign", "-", str(path)], check=True)


if __name__ == "__main__":
    main()
