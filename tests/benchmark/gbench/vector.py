from __future__ import annotations

import argparse
import json
import math
import socket
import tempfile
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Sequence

import uvicorn

from tonggraph import Graph
from tonggraph.server.app import create_app
from tonggraph.server.client import TongGraphClient
from tonggraph.server.config import parse_config

from .datasets import BENCH_ROOT
from .metrics import percentile, runtime_metadata
from .runner import REPO_ROOT


INDEX_NAME = "embeddings"


@dataclass(frozen=True)
class VectorBenchmarkConfig:
    vectors: int = 10_000
    dimensions: int = 128
    queries: int = 20
    batch_size: int = 8
    repeat: int = 3
    seed: int = 7
    metric: str = "cosine"
    limit: int = 10
    cache_dir: Path | None = None
    include_server: bool = True

    @property
    def temp_dir(self) -> Path:
        if self.cache_dir is None:
            return BENCH_ROOT / "temp"
        return self.cache_dir.parent / "temp"


def run_vector_benchmark(config: VectorBenchmarkConfig) -> dict[str, Any]:
    _validate_config(config)
    config.temp_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="vector-exact-", dir=config.temp_dir) as temp_dir:
        temp_path = Path(temp_dir)
        db_path = temp_path / "vectors.db"
        graph = build_vector_graph(db_path, config)
        query_vectors = _query_vectors(config)
        query_batches = _batches(query_vectors, config.batch_size)

        workloads = [
            _measure_operations(
                group="vector",
                name="embedded_search_vector",
                operation="Graph.search_vector",
                repeat=config.repeat,
                operation_count=len(query_vectors),
                query_vectors_per_operation=1,
                fn=lambda i: _checksum_search(graph.search_vector(INDEX_NAME, query_vectors[i], limit=config.limit)),
            ),
            _measure_operations(
                group="vector",
                name="embedded_search_vectors",
                operation="Graph.search_vectors",
                repeat=config.repeat,
                operation_count=len(query_batches),
                query_vectors_per_operation=config.batch_size,
                fn=lambda i: _checksum_batches(graph.search_vectors(INDEX_NAME, query_batches[i], limit=config.limit)),
            ),
        ]

        if config.include_server:
            workloads.extend(_run_server_workloads(temp_path, query_vectors, query_batches, config))

        return {
            "metadata": runtime_metadata(REPO_ROOT),
            "config": {
                "vectors": config.vectors,
                "dimensions": config.dimensions,
                "queries": config.queries,
                "batch_size": config.batch_size,
                "repeat": config.repeat,
                "seed": config.seed,
                "metric": config.metric,
                "limit": config.limit,
                "include_server": config.include_server,
            },
            "dataset": {
                "vectors": config.vectors,
                "dimensions": config.dimensions,
                "nodes": graph.node_count(),
                "edges": graph.edge_count(),
                "index": INDEX_NAME,
                "metric": config.metric,
                "storage": "sqlite",
            },
            "workloads": workloads,
        }


def build_vector_graph(path: Path, config: VectorBenchmarkConfig) -> Graph:
    graph = Graph(str(path))
    graph.add_nodes(
        [
            {
                "external_id": f"vector:{node_id}",
                "labels": ["VectorItem"],
                "properties": {"rank": node_id},
            }
            for node_id in range(config.vectors)
        ]
    )
    graph.create_vector_index(INDEX_NAME, config.dimensions, target="node", metric=config.metric)
    for start in range(0, config.vectors, 1000):
        stop = min(start + 1000, config.vectors)
        graph.upsert_vectors(
            INDEX_NAME,
            {entity_id: deterministic_vector(entity_id, config.dimensions, config.seed) for entity_id in range(start, stop)},
        )
    return graph


def deterministic_vector(entity_id: int, dimensions: int, seed: int) -> list[float]:
    state = ((entity_id + 1) * 0x9E3779B1) ^ seed
    values: list[float] = []
    for dim in range(dimensions):
        state = (state * 1664525 + 1013904223 + dim) & 0xFFFFFFFF
        values.append(((state % 2001) - 1000) / 1000.0)
    return values


def main(argv: Sequence[str] | None = None) -> dict[str, Any]:
    args = _build_parser().parse_args(argv)
    artifact = run_vector_benchmark(
        VectorBenchmarkConfig(
            vectors=args.vectors,
            dimensions=args.dimensions,
            queries=args.queries,
            batch_size=args.batch_size,
            repeat=args.repeat,
            seed=args.seed,
            metric=args.metric,
            limit=args.limit,
            cache_dir=args.cache_dir,
            include_server=not args.skip_server,
        )
    )
    payload = json.dumps(artifact, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    else:
        print(payload)
    return artifact


def _run_server_workloads(
    data_dir: Path,
    query_vectors: list[list[float]],
    query_batches: list[list[list[float]]],
    config: VectorBenchmarkConfig,
) -> list[dict[str, Any]]:
    app = create_app(
        parse_config(
            {
                "host": "127.0.0.1",
                "port": 0,
                "data_dir": str(data_dir),
                "graphs": {"bench": "vectors.db"},
                "auth": {"mode": "none"},
                "operations": {"request_logging": False, "metrics": False},
            }
        )
    )
    port = _free_port()
    server = uvicorn.Server(uvicorn.Config(app, host="127.0.0.1", port=port, log_level="critical", lifespan="on"))
    thread = threading.Thread(target=server.run, daemon=True)
    thread.start()
    _wait_for_server(server, thread)
    try:
        graph = TongGraphClient(f"http://127.0.0.1:{port}").graph("bench")
        graph.open()
        return [
            _measure_operations(
                group="vector",
                name="server_search_vector",
                operation="HTTP POST /vector/{index}/search",
                repeat=config.repeat,
                operation_count=len(query_vectors),
                query_vectors_per_operation=1,
                fn=lambda i: _checksum_search(graph.search_vector(INDEX_NAME, query_vectors[i], limit=config.limit)),
            ),
            _measure_operations(
                group="vector",
                name="server_search_vectors",
                operation="HTTP POST /vector/{index}/search-batch",
                repeat=config.repeat,
                operation_count=len(query_batches),
                query_vectors_per_operation=config.batch_size,
                fn=lambda i: _checksum_batches(graph.search_vectors(INDEX_NAME, query_batches[i], limit=config.limit)),
            ),
        ]
    finally:
        server.should_exit = True
        thread.join(timeout=10)


def _measure_operations(
    *,
    group: str,
    name: str,
    operation: str,
    repeat: int,
    operation_count: int,
    query_vectors_per_operation: int,
    fn: Callable[[int], int],
) -> dict[str, Any]:
    timings: list[int] = []
    checksum = 0
    for _ in range(repeat):
        for index in range(operation_count):
            start = time.perf_counter_ns()
            checksum += fn(index)
            timings.append(time.perf_counter_ns() - start)

    total_ns = sum(timings)
    total_operations = len(timings)
    return {
        "group": group,
        "name": name,
        "operation": operation,
        "repeat": repeat,
        "operation_count": operation_count,
        "total_operations": total_operations,
        "query_vectors_per_operation": query_vectors_per_operation,
        "checksum": checksum,
        "min_ns": min(timings),
        "mean_ns": int(total_ns / total_operations),
        "p50_ns": percentile(timings, 0.50),
        "p90_ns": percentile(timings, 0.90),
        "p95_ns": percentile(timings, 0.95),
        "p99_ns": percentile(timings, 0.99),
        "max_ns": max(timings),
        "throughput_ops_per_sec": total_operations / (total_ns / 1_000_000_000) if total_ns else math.inf,
    }


def _query_vectors(config: VectorBenchmarkConfig) -> list[list[float]]:
    return [
        deterministic_vector((config.seed + index * 7919) % config.vectors, config.dimensions, config.seed)
        for index in range(config.queries)
    ]


def _batches(values: list[list[float]], batch_size: int) -> list[list[list[float]]]:
    return [values[start : start + batch_size] for start in range(0, len(values), batch_size)]


def _checksum_search(rows: list[dict[str, Any]]) -> int:
    return sum(int(row["id"]) for row in rows) + len(rows)


def _checksum_batches(batches: list[list[dict[str, Any]]]) -> int:
    return sum(_checksum_search(rows) for rows in batches)


def _validate_config(config: VectorBenchmarkConfig) -> None:
    if config.vectors <= 0:
        raise ValueError("--vectors must be greater than 0")
    if config.dimensions <= 0:
        raise ValueError("--dimensions must be greater than 0")
    if config.queries <= 0:
        raise ValueError("--queries must be greater than 0")
    if config.batch_size <= 0:
        raise ValueError("--batch-size must be greater than 0")
    if config.repeat <= 0:
        raise ValueError("--repeat must be greater than 0")
    if config.limit <= 0:
        raise ValueError("--limit must be greater than 0")
    if config.metric not in {"cosine", "dot", "euclidean"}:
        raise ValueError('--metric must be "cosine", "dot", or "euclidean"')


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run exact vector search benchmarks for embedded and server APIs.")
    parser.add_argument("--vectors", type=int, default=10_000, help="Number of node vectors to index.")
    parser.add_argument("--dimensions", type=int, default=128, help="Vector dimensions.")
    parser.add_argument("--queries", type=int, default=20, help="Query vectors to measure per repeat.")
    parser.add_argument("--batch-size", type=int, default=8, help="Query vectors per search-batch request.")
    parser.add_argument("--repeat", type=int, default=3, help="Measured repeats.")
    parser.add_argument("--seed", type=int, default=7, help="Deterministic data seed.")
    parser.add_argument("--metric", choices=["cosine", "dot", "euclidean"], default="cosine", help="Vector metric.")
    parser.add_argument("--limit", type=int, default=10, help="Search result limit.")
    parser.add_argument("--cache-dir", type=Path, help="Benchmark cache directory; temp files are placed next to it.")
    parser.add_argument("--skip-server", action="store_true", help="Only run embedded Graph vector workloads.")
    parser.add_argument("--output", type=Path, help="Write JSON results to this path instead of stdout.")
    return parser


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _wait_for_server(server: uvicorn.Server, thread: threading.Thread) -> None:
    deadline = time.time() + 10
    while not server.started:
        if not thread.is_alive() or time.time() > deadline:
            server.should_exit = True
            thread.join(timeout=5)
            raise RuntimeError("uvicorn benchmark server did not start")
        time.sleep(0.01)


if __name__ == "__main__":
    main()
