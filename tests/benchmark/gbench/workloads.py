from __future__ import annotations

import fnmatch
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Callable

from tonggraph import Graph

from .datasets import DatasetArtifact

if TYPE_CHECKING:
    from .runner import BenchmarkConfig


WorkloadFn = Callable[[DatasetArtifact, "BenchmarkConfig"], int]


@dataclass(frozen=True)
class Workload:
    group: str
    name: str
    datasets: tuple[str, ...]
    run: WorkloadFn

    @property
    def key(self) -> str:
        return f"{self.group}/{self.name}"


def workloads_for_dataset(dataset: str) -> list[Workload]:
    return [workload for workload in _REGISTRY if dataset in workload.datasets]


def select_workloads(dataset: str, patterns: tuple[str, ...]) -> list[Workload]:
    available = workloads_for_dataset(dataset)
    if not patterns:
        return available

    selected = [
        workload
        for workload in available
        if any(_matches_workload(workload, pattern) for pattern in patterns)
    ]
    if not selected:
        known = ", ".join(workload.key for workload in available)
        raise ValueError(f"no workloads matched {patterns!r}; available workloads: {known}")
    return selected


def _matches_workload(workload: Workload, pattern: str) -> bool:
    return fnmatch.fnmatchcase(workload.key, pattern) or fnmatch.fnmatchcase(workload.name, pattern)


def _synthetic_traversal(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    graph = dataset.graph
    start = int(dataset.metadata["start_node"])
    target = int(dataset.metadata["target_node"])
    bfs = graph.bfs(start, max_depth=3)
    path = graph.shortest_path(start, target)
    ranks = graph.pagerank(iterations=5)
    return len(bfs) + len(path["nodes"] if path else []) + int(sum(ranks.values()) * 1000)


def _synthetic_query(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    graph = dataset.graph
    spec = {
        "match": [
            {"node": "a", "labels": ["Document"]},
            {"edge": "r", "type": "LINKS"},
            {"node": "b", "properties": {"published": True}},
        ],
        "return": ["a", "b"],
        "limit": 20,
    }
    structured = graph.query(spec, profile=True)
    cypher = graph.cypher(
        "MATCH (a:Document)-[:LINKS]->(b) WHERE b.published = true RETURN a, b LIMIT 20",
        profile=True,
    )
    return len(structured["rows"]) + len(cypher.records)


def _synthetic_graphrag(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    rows = dataset.graph.retrieve_context(
        text_index="docs",
        text_query="graph memory",
        vector_index="docs",
        vector_query=[1.0, 0.2, 0.5],
        labels=["Document"],
        radius=1,
        limit=10,
    )
    return sum(int(row["id"]) for row in rows)


def _synthetic_persistence(dataset: DatasetArtifact, config: "BenchmarkConfig") -> int:
    with tempfile.TemporaryDirectory(prefix="gbench-", dir=config.temp_dir) as temp_dir:
        path = Path(temp_dir) / "bench.db"
        persisted = Graph(str(path))
        persisted.add_nodes(
            [
                {
                    "external_id": node.external_id,
                    "labels": node.labels,
                    "properties": node.properties,
                }
                for node in dataset.graph.nodes()
            ]
        )
        persisted.add_edges(
            [
                {
                    "source": edge.source,
                    "target": edge.target,
                    "edge_type": edge.edge_type,
                    "properties": edge.properties,
                }
                for edge in dataset.graph.edges()
            ]
        )
        persisted.compact()
        reopened = Graph(str(path))
        return reopened.node_count() + reopened.edge_count()


def _synthetic_inference(_dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    graph = Graph()
    parent = graph.add_variable("binary", prior={"false": 0.4, "true": 0.6})
    child = graph.add_variable("binary")
    graph.add_cpd(child, [parent], [0.8, 0.2, 0.1, 0.9])
    result = graph.belief_propagation([child], evidence={parent: "true"}, max_iters=50)
    return int(result["beliefs"][child]["true"] * 1000)


def _pokec_single_vertex(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    start = _start_node(dataset)
    if start is None:
        return 0
    return int(dataset.graph.get_node(start).properties.get("id", 0))


def _pokec_expansion_1(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    start = _start_node(dataset)
    if start is None:
        return 0
    return sum(dataset.graph.neighbors(start, edge_type="Friend"))


def _pokec_expansion_2(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    start = _start_node(dataset)
    if start is None:
        return 0
    first_hop = dataset.graph.neighbors(start, edge_type="Friend")
    second_hop: set[int] = set()
    for node_id in first_hop:
        second_hop.update(dataset.graph.neighbors(node_id, edge_type="Friend"))
    return sum(second_hop)


def _pokec_age_distribution(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    histogram: dict[int, int] = {}
    for node_id in dataset.graph.nodes_with_property("age"):
        age = int(dataset.graph.get_node(node_id).properties["age"])
        histogram[age] = histogram.get(age, 0) + 1
    return sum(age * count for age, count in histogram.items())


def _pokec_dsl_expansion(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    start = _start_node(dataset)
    if start is None:
        return 0
    rows = dataset.graph.query(
        {
            "match": [
                {"node": "s", "id": start},
                {"edge": "r", "type": "Friend"},
                {"node": "n"},
            ],
            "return": ["n"],
            "limit": 50,
        }
    )
    return sum(int(row["n"]) for row in rows)


def _pokec_cypher_expansion(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    external_id = dataset.metadata.get("start_external_id")
    if external_id is None:
        return 0
    rows = dataset.graph.cypher(
        "MATCH (s:User {id: $id})-[:Friend]->(n:User) RETURN n.id AS id LIMIT 50",
        {"id": external_id},
    )
    return sum(int(row["id"]) for row in rows.records)


def _pokec_shortest_path(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    start = _start_node(dataset)
    target = _target_node(dataset)
    if start is None or target is None:
        return 0
    path = dataset.graph.shortest_path(start, target, edge_type="Friend")
    if path is None:
        return 0
    return len(path["nodes"]) + int(path["distance"])


def _pokec_pagerank(dataset: DatasetArtifact, _config: "BenchmarkConfig") -> int:
    ranks = dataset.graph.pagerank(iterations=5, edge_type="Friend")
    return int(sum(ranks.values()) * 1000)


def _start_node(dataset: DatasetArtifact) -> int | None:
    value = dataset.metadata.get("start_node")
    return int(value) if value is not None else None


def _target_node(dataset: DatasetArtifact) -> int | None:
    value = dataset.metadata.get("target_node")
    return int(value) if value is not None else None


_REGISTRY = [
    Workload("synthetic", "traversal", ("synthetic-smoke",), _synthetic_traversal),
    Workload("synthetic", "query", ("synthetic-smoke",), _synthetic_query),
    Workload("synthetic", "graphrag", ("synthetic-smoke",), _synthetic_graphrag),
    Workload("synthetic", "persistence", ("synthetic-smoke",), _synthetic_persistence),
    Workload("synthetic", "inference", ("synthetic-smoke",), _synthetic_inference),
    Workload("read", "single_vertex", ("pokec-small",), _pokec_single_vertex),
    Workload("read", "expansion_1", ("pokec-small",), _pokec_expansion_1),
    Workload("read", "expansion_2", ("pokec-small",), _pokec_expansion_2),
    Workload("aggregate", "age_distribution", ("pokec-small",), _pokec_age_distribution),
    Workload("query", "dsl_expansion", ("pokec-small",), _pokec_dsl_expansion),
    Workload("query", "cypher_expansion", ("pokec-small",), _pokec_cypher_expansion),
    Workload("analytical", "shortest_path", ("pokec-small",), _pokec_shortest_path),
    Workload("analytical", "pagerank", ("pokec-small",), _pokec_pagerank),
]
