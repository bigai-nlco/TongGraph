from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence

from .datasets import DEFAULT_CACHE_DIR, DEFAULT_TEMP_DIR, DatasetArtifact, load_dataset
from .metrics import runtime_metadata, time_repeated
from .workloads import Workload, select_workloads, workloads_for_dataset


REPO_ROOT = Path(__file__).resolve().parents[3]


@dataclass(frozen=True)
class BenchmarkConfig:
    dataset: str = "synthetic-smoke"
    nodes: int = 100
    degree: int = 3
    repeat: int = 3
    seed: int = 7
    warm_up: str = "cold"
    workloads: tuple[str, ...] = ()
    cache_dir: Path | None = None
    max_nodes: int | None = None
    max_edges: int | None = None

    @property
    def resolved_cache_dir(self) -> Path:
        return self.cache_dir or DEFAULT_CACHE_DIR

    @property
    def temp_dir(self) -> Path:
        if self.cache_dir is None:
            return DEFAULT_TEMP_DIR
        return self.cache_dir.parent / "temp"


def run_benchmark(config: BenchmarkConfig) -> dict[str, Any]:
    _validate_config(config)
    config.resolved_cache_dir.mkdir(parents=True, exist_ok=True)
    config.temp_dir.mkdir(parents=True, exist_ok=True)

    dataset = load_dataset(
        config.dataset,
        nodes=config.nodes,
        degree=config.degree,
        seed=config.seed,
        cache_dir=config.resolved_cache_dir,
        max_nodes=config.max_nodes,
        max_edges=config.max_edges,
    )
    selected = select_workloads(dataset.name, config.workloads)

    workload_results = []
    for workload in selected:
        _warm_up(config.warm_up, config.repeat, workload, dataset, config)
        metrics = time_repeated(config.repeat, lambda workload=workload: workload.run(dataset, config))
        workload_results.append(
            {
                "group": workload.group,
                "name": workload.name,
                **metrics,
            }
        )

    return {
        "metadata": {
            **runtime_metadata(REPO_ROOT),
            "dataset_cache_status": dataset.metadata.get("cache_status"),
            "dataset_source": dataset.metadata.get("source"),
            "dataset_source_url": dataset.metadata.get("source_url"),
        },
        "config": {
            "dataset": config.dataset,
            "nodes": config.nodes,
            "degree": config.degree,
            "repeat": config.repeat,
            "seed": config.seed,
            "warm_up": config.warm_up,
            "workloads": list(config.workloads),
            "max_nodes": config.max_nodes,
            "max_edges": config.max_edges,
            "cache_dir": str(config.resolved_cache_dir),
        },
        "dataset": dataset.stats,
        "workloads": workload_results,
    }


def list_workloads(dataset: str | None = None) -> list[dict[str, str]]:
    names = [dataset] if dataset else ["synthetic-smoke", "pokec-small"]
    listed: list[dict[str, str]] = []
    for dataset_name in names:
        for workload in workloads_for_dataset(dataset_name):
            listed.append(
                {
                    "dataset": dataset_name,
                    "group": workload.group,
                    "name": workload.name,
                }
            )
    return listed


def _validate_config(config: BenchmarkConfig) -> None:
    if config.repeat <= 0:
        raise ValueError("--repeat must be greater than 0")
    if config.warm_up not in {"cold", "hot", "vulcanic"}:
        raise ValueError('--warm-up must be "cold", "hot", or "vulcanic"')


def _warm_up(
    warm_up: str,
    repeat: int,
    workload: Workload,
    dataset: DatasetArtifact,
    config: BenchmarkConfig,
) -> None:
    if warm_up == "cold":
        return
    if warm_up == "hot":
        workload.run(dataset, config)
        return
    if warm_up == "vulcanic":
        for _ in range(repeat):
            workload.run(dataset, config)
        return
    raise ValueError(f'unknown warm-up condition "{warm_up}"')
