#!/usr/bin/env python3
"""Practical wall-clock benchmark for active-subgraph belief propagation."""

from __future__ import annotations

import argparse
import time
from dataclasses import dataclass

from tonggraph import Graph


@dataclass(frozen=True)
class RunSummary:
    total_seconds: float
    repeat: int
    active_variables: int
    active_factors: int
    active_graph_nodes: int
    iterations: int
    messages_updated: int
    converged: bool
    max_residual: float

    @property
    def average_ms(self) -> float:
        return self.total_seconds * 1000.0 / self.repeat


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--nodes", type=int, default=1000)
    parser.add_argument("--degree", type=int, default=4)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--radius", type=int, default=2)
    parser.add_argument("--max-iters", type=int, default=1000)
    parser.add_argument("--tolerance", type=float, default=1e-6)
    parser.add_argument("--damping", type=float, default=0.2)
    args = parser.parse_args()
    if args.nodes <= 0:
        raise SystemExit("--nodes must be positive")
    if args.degree < 0:
        raise SystemExit("--degree must be non-negative")
    if args.repeat <= 0:
        raise SystemExit("--repeat must be positive")
    if args.radius < 0:
        raise SystemExit("--radius must be non-negative")

    build_start = time.perf_counter()
    graph, variables = build_factor_graph(args.nodes, args.degree, args.seed)
    build_seconds = time.perf_counter() - build_start

    query = variables[args.seed % args.nodes]
    evidence_variable = variables[(args.seed * 17 + 3) % args.nodes]
    evidence = {evidence_variable: "true"}

    active = graph.compile_active_subgraph([query], evidence=evidence, radius=args.radius)
    summary = benchmark(
        graph,
        query,
        evidence,
        args.radius,
        args.max_iters,
        args.tolerance,
        args.damping,
        args.repeat,
    )

    print(
        f"sample_factor_graph nodes={args.nodes} degree={args.degree} "
        f"edges={graph.edge_count()} factors={graph.factor_count()} "
        f"build_ms={build_seconds * 1000.0:.3f}"
    )
    print(
        f"query={query} evidence={evidence_variable}:true radius={args.radius} "
        f"active_variables={summary.active_variables} "
        f"active_factors={summary.active_factors} "
        f"active_graph_nodes={summary.active_graph_nodes} "
        f"truncated={active['truncated']}"
    )
    print(
        f"belief_propagation: total_ms={summary.total_seconds * 1000.0:.3f} "
        f"avg_ms={summary.average_ms:.3f} repeat={summary.repeat} "
        f"iterations={summary.iterations} "
        f"messages_updated={summary.messages_updated} "
        f"converged={summary.converged} "
        f"max_residual={summary.max_residual:.6g}"
    )


def build_factor_graph(node_count: int, degree: int, seed: int) -> tuple[Graph, list[int]]:
    graph = Graph()
    nodes = [graph.add_node(f"node:{index}") for index in range(node_count)]
    variables = [
        graph.add_variable("binary", owner_id=node_id, prior={"p": prior_probability(index, seed)})
        for index, node_id in enumerate(nodes)
    ]
    if node_count == 1:
        return graph, variables

    max_degree = min(degree, node_count - 1)
    for source in range(node_count):
        for offset in range(1, max_degree + 1):
            target = (source + offset) % node_count
            graph.add_edge(nodes[source], nodes[target], "LINK")
            same = 1.0 + deterministic_strength(source, offset, seed)
            different = 1.0
            graph.add_factor_table(
                [variables[source], variables[target]],
                [same, different, different, same],
            )
    return graph, variables


def prior_probability(index: int, seed: int) -> float:
    return 0.1 + (((index * 31 + seed * 17) % 80) / 100.0)


def deterministic_strength(source: int, offset: int, seed: int) -> float:
    return ((source * 13 + offset * 19 + seed * 23) % 100) / 100.0


def benchmark(
    graph: Graph,
    query: int,
    evidence: dict[int, str],
    radius: int,
    max_iters: int,
    tolerance: float,
    damping: float,
    repeat: int,
) -> RunSummary:
    last_result = None
    start = time.perf_counter()
    for _ in range(repeat):
        last_result = graph.belief_propagation(
            [query],
            evidence=evidence,
            radius=radius,
            max_iters=max_iters,
            tolerance=tolerance,
            damping=damping,
            persist=False,
        )
    elapsed = time.perf_counter() - start
    assert last_result is not None
    active = last_result["active"]
    return RunSummary(
        total_seconds=elapsed,
        repeat=repeat,
        active_variables=len(active["variables"]),
        active_factors=len(active["factors"]),
        active_graph_nodes=len(active["graph_nodes"]),
        iterations=last_result["iterations"],
        messages_updated=last_result["messages_updated"],
        converged=last_result["converged"],
        max_residual=last_result["max_residual"],
    )


if __name__ == "__main__":
    main()
