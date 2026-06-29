from __future__ import annotations

import json
from pathlib import Path

from tests.benchmark.gbench.vector import VectorBenchmarkConfig, main, run_vector_benchmark


def test_vector_benchmark_smoke_shape(tmp_path: Path) -> None:
    artifact = run_vector_benchmark(
        VectorBenchmarkConfig(
            vectors=16,
            dimensions=4,
            queries=3,
            batch_size=2,
            repeat=1,
            cache_dir=tmp_path / "cache",
        )
    )

    assert artifact["config"]["vectors"] == 16
    assert artifact["config"]["dimensions"] == 4
    assert artifact["dataset"] == {
        "vectors": 16,
        "dimensions": 4,
        "nodes": 16,
        "edges": 0,
        "index": "embeddings",
        "metric": "cosine",
        "storage": "sqlite",
    }
    assert {workload["name"] for workload in artifact["workloads"]} == {
        "embedded_search_vector",
        "embedded_search_vectors",
        "server_search_vector",
        "server_search_vectors",
    }
    for workload in artifact["workloads"]:
        assert workload["checksum"] > 0
        assert workload["mean_ns"] > 0
        assert workload["p95_ns"] >= workload["min_ns"]
        assert workload["throughput_ops_per_sec"] > 0


def test_vector_benchmark_cli_writes_json(tmp_path: Path) -> None:
    output = tmp_path / "vector.json"
    main(
        [
            "--vectors",
            "12",
            "--dimensions",
            "4",
            "--queries",
            "2",
            "--batch-size",
            "2",
            "--repeat",
            "1",
            "--cache-dir",
            str(tmp_path / "cache"),
            "--output",
            str(output),
        ]
    )

    artifact = json.loads(output.read_text(encoding="utf-8"))
    assert artifact["config"]["vectors"] == 12
    assert artifact["workloads"][0]["name"] == "embedded_search_vector"
