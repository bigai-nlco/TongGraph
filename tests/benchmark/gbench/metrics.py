from __future__ import annotations

import math
import platform
import subprocess
import time
from pathlib import Path
from typing import Callable


def time_repeated(repeat: int, fn: Callable[[], int]) -> dict[str, object]:
    if repeat <= 0:
        raise ValueError("--repeat must be greater than 0")

    timings: list[int] = []
    checksum = 0
    for _ in range(repeat):
        start = time.perf_counter_ns()
        checksum = fn()
        timings.append(time.perf_counter_ns() - start)

    total_ns = sum(timings)
    return {
        "repeat": repeat,
        "checksum": checksum,
        "min_ns": min(timings),
        "mean_ns": int(total_ns / repeat),
        "p50_ns": percentile(timings, 0.50),
        "p90_ns": percentile(timings, 0.90),
        "p95_ns": percentile(timings, 0.95),
        "p99_ns": percentile(timings, 0.99),
        "max_ns": max(timings),
        "throughput_qps": repeat / (total_ns / 1_000_000_000) if total_ns else math.inf,
    }


def percentile(values: list[int], quantile: float) -> int:
    if not values:
        raise ValueError("percentile requires at least one value")
    ordered = sorted(values)
    index = math.ceil((len(ordered) - 1) * quantile)
    return ordered[index]


def runtime_metadata(repo_root: Path) -> dict[str, object]:
    return {
        "commit": git_commit(repo_root),
        "python": platform.python_version(),
        "platform": platform.platform(),
        "timestamp_unix_ns": time.time_ns(),
    }


def git_commit(repo_root: Path) -> str | None:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except Exception:
        return None
