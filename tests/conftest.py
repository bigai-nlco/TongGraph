from __future__ import annotations

import os
import subprocess
import sys
import sysconfig
from pathlib import Path


def pytest_configure() -> None:
    if os.environ.get("TONGGRAPH_SKIP_EXTENSION_FRESHNESS") == "1":
        return
    root = Path(__file__).resolve().parents[1]
    extension = root / "python" / "tonggraph" / f"_tonggraph{sysconfig.get_config_var('EXT_SUFFIX')}"
    generic_extension = root / "python" / "tonggraph" / ("_tonggraph.pyd" if sys.platform == "win32" else "_tonggraph.so")
    artifacts = [path for path in [extension, generic_extension] if path.exists()]
    if artifacts and max(path.stat().st_mtime for path in artifacts) >= _latest_source_mtime(root):
        return
    subprocess.run(
        [sys.executable, str(root / "scripts" / "build_python_extension.py")],
        cwd=root,
        check=True,
    )


def _latest_source_mtime(root: Path) -> float:
    paths = [root / "Cargo.toml", root / "pyproject.toml"]
    paths.extend((root / "src").rglob("*.rs"))
    paths.extend((root / "python" / "tonggraph").glob("*.py"))
    return max(path.stat().st_mtime for path in paths if path.exists())
