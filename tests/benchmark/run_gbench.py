from __future__ import annotations

import json
from pathlib import Path

from tests.benchmark.gbench import run_suite
from tests.benchmark.gbench.cli import main
from tests.benchmark.gbench.datasets import build_pokec_graph, parse_pokec_lines


def test_gbench_smoke_writes_json_shape() -> None:
    artifact = run_suite(nodes=12, degree=2, repeat=1)
    encoded = json.dumps(artifact)
    assert "traversal" in encoded
    assert artifact["config"]["dataset"] == "synthetic-smoke"
    assert artifact["config"]["nodes"] == 12
    assert artifact["config"]["degree"] == 2
    assert artifact["config"]["repeat"] == 1
    assert artifact["dataset"] == {"nodes": 12, "edges": 24}
    assert {workload["name"] for workload in artifact["workloads"]} == {
        "traversal",
        "query",
        "graphrag",
        "persistence",
        "inference",
    }


def test_pokec_parser_filters_edges_to_selected_nodes() -> None:
    rows = parse_pokec_lines(
        [
            'CREATE (:User {id: 10, completion_percentage: 50, gender: "man", age: 20});',
            'CREATE (:User {id: 11, completion_percentage: 60, gender: "woman", age: 21});',
            'CREATE (:User {id: 12, completion_percentage: 70, gender: "woman", age: 22});',
            ";",
            "MATCH (n:User {id: 10}), (m:User {id: 11}) CREATE (n)-[e: Friend]->(m);",
            "MATCH (n:User {id: 11}), (m:User {id: 12}) CREATE (n)-[e: Friend]->(m);",
        ],
        max_nodes=2,
        max_edges=10,
    )

    assert len(rows.nodes) == 2
    assert rows.edges == ((10, 11),)
    artifact = build_pokec_graph(rows, source_path=Path("inline.cypher"))
    assert artifact.graph.node_count() == 2
    assert artifact.graph.edge_count() == 1
    edge = artifact.graph.get_edge(0)
    assert edge.source == 0
    assert edge.target == 1
    assert artifact.metadata["start_external_id"] == 10


def test_workload_filtering_by_group_name_glob(tmp_path: Path) -> None:
    artifact = run_suite(
        nodes=12,
        degree=2,
        repeat=1,
        workloads=["synthetic/query"],
        cache_dir=tmp_path / "cache",
    )

    assert [(workload["group"], workload["name"]) for workload in artifact["workloads"]] == [
        ("synthetic", "query")
    ]


def test_cli_writes_json_output(tmp_path: Path) -> None:
    output = tmp_path / "gbench.json"
    main(
        [
            "--dataset",
            "synthetic-smoke",
            "--repeat",
            "1",
            "--workload",
            "synthetic/traversal",
            "--cache-dir",
            str(tmp_path / "cache"),
            "--output",
            str(output),
        ]
    )

    artifact = json.loads(output.read_text(encoding="utf-8"))
    assert artifact["config"]["dataset"] == "synthetic-smoke"
    assert artifact["workloads"][0]["name"] == "traversal"
