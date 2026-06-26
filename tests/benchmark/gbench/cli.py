from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Sequence

from .runner import BenchmarkConfig, list_workloads, run_benchmark


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run pure-Python TongGraph GBench benchmarks.",
    )
    parser.add_argument(
        "--dataset",
        choices=["synthetic-smoke", "pokec-small"],
        default="synthetic-smoke",
        help="Dataset to load. pokec-small downloads and caches the upstream file on first use.",
    )
    parser.add_argument("--repeat", type=int, default=3, help="Measured repeats per workload.")
    parser.add_argument("--seed", type=int, default=7, help="Deterministic seed for workload choices.")
    parser.add_argument(
        "--workload",
        action="append",
        default=[],
        metavar="GROUP/NAME",
        help="Workload glob to run. Can be repeated. Defaults to all workloads for the dataset.",
    )
    parser.add_argument(
        "--warm-up",
        choices=["cold", "hot", "vulcanic"],
        default="cold",
        help="Warm-up condition before measuring each workload.",
    )
    parser.add_argument("--nodes", type=int, default=100, help="Synthetic smoke node count.")
    parser.add_argument("--degree", type=int, default=3, help="Synthetic smoke outgoing degree.")
    parser.add_argument("--max-nodes", type=int, help="Maximum Pokec nodes to import.")
    parser.add_argument("--max-edges", type=int, help="Maximum Pokec edges to import after node filtering.")
    parser.add_argument(
        "--cache-dir",
        type=Path,
        help="Benchmark cache directory. Defaults to tests/benchmark/.gbench/cache.",
    )
    parser.add_argument("--output", type=Path, help="Write JSON results to this path instead of stdout.")
    parser.add_argument("--list-workloads", action="store_true", help="Print available workloads and exit.")
    return parser


def main(argv: Sequence[str] | None = None) -> dict[str, object] | None:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.list_workloads:
        for workload in list_workloads(dataset=args.dataset):
            print(f"{workload['group']}/{workload['name']}")
        return None

    config = BenchmarkConfig(
        dataset=args.dataset,
        nodes=args.nodes,
        degree=args.degree,
        repeat=args.repeat,
        seed=args.seed,
        warm_up=args.warm_up,
        workloads=tuple(args.workload),
        cache_dir=args.cache_dir,
        max_nodes=args.max_nodes,
        max_edges=args.max_edges,
    )
    artifact = run_benchmark(config)
    payload = json.dumps(artifact, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    else:
        print(payload)
    return artifact
