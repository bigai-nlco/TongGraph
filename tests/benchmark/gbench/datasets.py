from __future__ import annotations

import re
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable

from tonggraph import Graph


BENCH_ROOT = Path(__file__).resolve().parents[1] / ".gbench"
DEFAULT_CACHE_DIR = BENCH_ROOT / "cache"
DEFAULT_TEMP_DIR = BENCH_ROOT / "temp"
POKEC_SMALL_URL = (
    "https://s3.eu-west-1.amazonaws.com/deps.memgraph.io/dataset/pokec/benchmark/"
    "pokec_small_import.cypher"
)

_POKEC_NODE_RE = re.compile(
    r'^CREATE \(:User \{id: (?P<id>\d+), completion_percentage: '
    r'(?P<completion>\d+), gender: "(?P<gender>[^"]+)", age: (?P<age>\d+)\}\);$'
)
_POKEC_EDGE_RE = re.compile(
    r"^MATCH \(n:User \{id: (?P<source>\d+)\}\), "
    r"\(m:User \{id: (?P<target>\d+)\}\) CREATE \(n\)-\[e: Friend\]->\(m\);$"
)


@dataclass(frozen=True)
class DatasetArtifact:
    name: str
    graph: Graph
    stats: dict[str, object]
    metadata: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class PokecRows:
    nodes: tuple[dict[str, object], ...]
    edges: tuple[tuple[int, int], ...]
    ignored_lines: int = 0


def load_dataset(
    name: str,
    *,
    nodes: int,
    degree: int,
    seed: int,
    cache_dir: Path | None,
    max_nodes: int | None,
    max_edges: int | None,
) -> DatasetArtifact:
    if name == "synthetic-smoke":
        return build_synthetic_smoke(nodes=nodes, degree=degree, seed=seed)
    if name == "pokec-small":
        return load_pokec_small(
            cache_dir=cache_dir or DEFAULT_CACHE_DIR,
            max_nodes=max_nodes,
            max_edges=max_edges,
        )
    raise ValueError(f'unknown dataset "{name}"')


def build_synthetic_smoke(*, nodes: int, degree: int, seed: int) -> DatasetArtifact:
    if nodes <= 0:
        raise ValueError("--nodes must be greater than 0")
    if degree < 0:
        raise ValueError("--degree must be non-negative")

    graph = Graph()
    records = [
        {
            "external_id": f"node:{i}",
            "labels": ["Document" if i % 3 == 0 else "Entity"],
            "properties": {
                "rank": i,
                "published": i % 2 == 0,
                "text": f"graph retrieval memory node {i} seed {seed}",
            },
        }
        for i in range(nodes)
    ]
    graph.add_nodes(records)

    edges = []
    for source in range(nodes):
        for offset in range(1, degree + 1):
            edges.append(
                {
                    "source": source,
                    "target": (source + offset) % nodes,
                    "edge_type": "LINKS",
                    "properties": {"weight": 1.0 / offset},
                }
            )
    graph.add_edges(edges)

    graph.create_fulltext_index("docs", ["text"])
    graph.create_vector_index("docs", 3)
    for node_id in range(nodes):
        graph.upsert_vector("docs", node_id, [1.0, float((node_id + seed) % 7) / 7.0, 0.5])

    return DatasetArtifact(
        name="synthetic-smoke",
        graph=graph,
        stats={"nodes": graph.node_count(), "edges": graph.edge_count()},
        metadata={
            "cache_status": "not_applicable",
            "source": "generated",
            "start_node": 0,
            "target_node": nodes // 2,
        },
    )


def load_pokec_small(
    *,
    cache_dir: Path,
    max_nodes: int | None = None,
    max_edges: int | None = None,
) -> DatasetArtifact:
    if max_nodes is not None and max_nodes <= 0:
        raise ValueError("--max-nodes must be greater than 0")
    if max_edges is not None and max_edges < 0:
        raise ValueError("--max-edges must be non-negative")

    source_path, cache_status = download_pokec_small(cache_dir)
    with source_path.open(encoding="utf-8") as source:
        rows = parse_pokec_lines(source, max_nodes=max_nodes, max_edges=max_edges)
    return build_pokec_graph(
        rows,
        source_path=source_path,
        source_url=POKEC_SMALL_URL,
        cache_status=cache_status,
    )


def download_pokec_small(cache_dir: Path) -> tuple[Path, str]:
    dataset_dir = cache_dir / "pokec-small"
    dataset_dir.mkdir(parents=True, exist_ok=True)
    target = dataset_dir / "pokec_small_import.cypher"
    if target.exists():
        return target, "cached"

    temp_target = target.with_suffix(".cypher.download")
    with urllib.request.urlopen(POKEC_SMALL_URL) as response, temp_target.open("wb") as output:
        while True:
            chunk = response.read(1024 * 1024)
            if not chunk:
                break
            output.write(chunk)
    temp_target.replace(target)
    return target, "downloaded"


def parse_pokec_lines(
    lines: Iterable[str],
    *,
    max_nodes: int | None = None,
    max_edges: int | None = None,
) -> PokecRows:
    nodes: list[dict[str, object]] = []
    edges: list[tuple[int, int]] = []
    selected_external_ids: set[int] = set()
    ignored_lines = 0

    for raw_line in lines:
        line = raw_line.strip()
        if not line or line == ";":
            continue

        node_match = _POKEC_NODE_RE.match(line)
        if node_match:
            if max_nodes is None or len(nodes) < max_nodes:
                external_id = int(node_match.group("id"))
                selected_external_ids.add(external_id)
                nodes.append(
                    {
                        "external_id": f"pokec:{external_id}",
                        "labels": ["User"],
                        "properties": {
                            "id": external_id,
                            "completion_percentage": int(node_match.group("completion")),
                            "gender": node_match.group("gender"),
                            "age": int(node_match.group("age")),
                        },
                    }
                )
            continue

        edge_match = _POKEC_EDGE_RE.match(line)
        if edge_match:
            source = int(edge_match.group("source"))
            target = int(edge_match.group("target"))
            if source in selected_external_ids and target in selected_external_ids:
                if max_edges is None or len(edges) < max_edges:
                    edges.append((source, target))
                if max_edges is not None and len(edges) >= max_edges:
                    break
            continue

        ignored_lines += 1

    return PokecRows(nodes=tuple(nodes), edges=tuple(edges), ignored_lines=ignored_lines)


def build_pokec_graph(
    rows: PokecRows,
    *,
    source_path: Path | None = None,
    source_url: str | None = None,
    cache_status: str = "not_applicable",
) -> DatasetArtifact:
    graph = Graph()
    node_records = list(rows.nodes)
    internal_ids = graph.add_nodes(node_records)
    internal_by_external = {
        int(record["properties"]["id"]): internal_id
        for record, internal_id in zip(node_records, internal_ids, strict=True)
    }
    edge_records = [
        {
            "source": internal_by_external[source],
            "target": internal_by_external[target],
            "edge_type": "Friend",
            "properties": {"source_id": source, "target_id": target},
        }
        for source, target in rows.edges
        if source in internal_by_external and target in internal_by_external
    ]
    graph.add_edges(edge_records)

    selected_nodes = graph.node_ids()
    selected_edges = graph.edges()
    start_node = selected_edges[0].source if selected_edges else (selected_nodes[0] if selected_nodes else None)
    target_node = selected_edges[0].target if selected_edges else (selected_nodes[-1] if selected_nodes else None)
    start_external_id = None
    if start_node is not None:
        start_external_id = graph.get_node(start_node).properties.get("id")

    return DatasetArtifact(
        name="pokec-small",
        graph=graph,
        stats={
            "nodes": graph.node_count(),
            "edges": graph.edge_count(),
            "ignored_lines": rows.ignored_lines,
        },
        metadata={
            "cache_status": cache_status,
            "source": str(source_path) if source_path is not None else "inline",
            "source_url": source_url,
            "start_node": start_node,
            "target_node": target_node,
            "start_external_id": start_external_id,
        },
    )
