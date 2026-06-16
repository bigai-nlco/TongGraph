#!/usr/bin/env python3
"""Practical wall-clock benchmark for TongGraph v0.3 algorithms."""

from __future__ import annotations

import argparse
import time
from collections.abc import Callable
from dataclasses import dataclass

from tonggraph import Graph


@dataclass(frozen=True)
class Timing:
    name: str
    total_seconds: float
    repeat: int

    @property
    def average_ms(self) -> float:
        return self.total_seconds * 1000.0 / self.repeat


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--nodes", type=int, default=1000)
    parser.add_argument("--degree", type=int, default=4)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--seed", type=int, default=7)
    args = parser.parse_args()
    if args.nodes <= 0:
        raise SystemExit("--nodes must be positive")
    if args.degree < 0:
        raise SystemExit("--degree must be non-negative")
    if args.repeat <= 0:
        raise SystemExit("--repeat must be positive")

    build_start = time.perf_counter()
    graph = build_sample_graph(args.nodes, args.degree)
    build_seconds = time.perf_counter() - build_start

    start = 0
    target = args.nodes // 2
    subgraph_nodes = list(range(min(args.nodes, 100)))
    jobs = [
        {"op": "bfs", "start": start, "max_depth": 3},
        {
            "op": "shortest_path",
            "start": start,
            "target": target,
            "weight_property": "weight",
        },
        {"op": "pagerank", "iterations": 10, "tolerance": 1e-9},
    ]

    timings = [
        benchmark("bfs", args.repeat, lambda: graph.bfs(start, max_depth=3)),
        benchmark(
            "shortest_path",
            args.repeat,
            lambda: graph.shortest_path(start, target, weight_property="weight"),
        ),
        benchmark("connected_components", args.repeat, graph.connected_components),
        benchmark(
            "pagerank",
            args.repeat,
            lambda: graph.pagerank(iterations=10, tolerance=1e-9),
        ),
        benchmark(
            "random_walk",
            args.repeat,
            lambda: graph.random_walk(start, min(args.nodes, 1000), seed=args.seed),
        ),
        benchmark("subgraph", args.repeat, lambda: graph.subgraph(subgraph_nodes)),
        benchmark("compute_batch", args.repeat, lambda: graph.compute_batch(jobs)),
    ]

    print(
        f"sample_graph nodes={args.nodes} degree={args.degree} "
        f"edges={graph.edge_count()} build_ms={build_seconds * 1000.0:.3f}"
    )
    for timing in timings:
        print(
            f"{timing.name}: total_ms={timing.total_seconds * 1000.0:.3f} "
            f"avg_ms={timing.average_ms:.3f} repeat={timing.repeat}"
        )


def build_sample_graph(node_count: int, degree: int) -> Graph:
    graph = Graph()
    nodes = [graph.add_node(f"node:{index}") for index in range(node_count)]
    if node_count == 1:
        return graph

    max_degree = min(degree, node_count - 1)
    for source in range(node_count):
        for offset in range(1, max_degree + 1):
            target = (source + offset) % node_count
            weight = deterministic_weight(source, offset)
            graph.add_edge(nodes[source], nodes[target], "LINK", properties={"weight": weight})
    return graph


def deterministic_weight(source: int, offset: int) -> float:
    return ((source * 31 + offset * 17) % 100 + 1) / 100.0


def benchmark(name: str, repeat: int, fn: Callable[[], object]) -> Timing:
    start = time.perf_counter()
    for _ in range(repeat):
        fn()
    return Timing(name=name, total_seconds=time.perf_counter() - start, repeat=repeat)


if __name__ == "__main__":
    main()
