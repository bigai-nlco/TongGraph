from __future__ import annotations

from pathlib import Path
from typing import Any, Sequence

from .runner import BenchmarkConfig, list_workloads, run_benchmark


def run_suite(
    nodes: int = 100,
    degree: int = 3,
    repeat: int = 3,
    *,
    dataset: str = "synthetic-smoke",
    seed: int = 7,
    warm_up: str = "cold",
    workloads: Sequence[str] | None = None,
    cache_dir: str | Path | None = None,
    max_nodes: int | None = None,
    max_edges: int | None = None,
) -> dict[str, Any]:
    """Run GBench and return a JSON-serializable result artifact."""

    config = BenchmarkConfig(
        dataset=dataset,
        nodes=nodes,
        degree=degree,
        repeat=repeat,
        seed=seed,
        warm_up=warm_up,
        workloads=tuple(workloads or ()),
        cache_dir=Path(cache_dir) if cache_dir is not None else None,
        max_nodes=max_nodes,
        max_edges=max_edges,
    )
    return run_benchmark(config)


__all__ = ["BenchmarkConfig", "list_workloads", "run_benchmark", "run_suite"]
