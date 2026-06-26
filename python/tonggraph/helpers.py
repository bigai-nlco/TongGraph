"""Python-level convenience helpers for the native TongGraph classes."""

from __future__ import annotations

import csv
import json
from pathlib import Path
from typing import Any, Iterable, Mapping


_NODE_RESERVED = {"id", "external_id", "labels", "properties"}
_EDGE_RESERVED = {"id", "source", "target", "edge_type", "type", "properties"}


def install_graph_helpers(graph_cls: type, snapshot_cls: type) -> None:
    graph_cls.retrieve_context = _retrieve_context
    graph_cls.import_nodes_csv = _import_nodes_csv
    graph_cls.import_edges_csv = _import_edges_csv
    graph_cls.import_nodes_jsonl = _import_nodes_jsonl
    graph_cls.import_edges_jsonl = _import_edges_jsonl
    graph_cls.export_nodes_jsonl = _export_nodes_jsonl
    graph_cls.export_edges_jsonl = _export_edges_jsonl
    graph_cls.export_query_rows_jsonl = _export_query_rows_jsonl
    snapshot_cls.export_nodes_jsonl = _export_nodes_jsonl
    snapshot_cls.export_edges_jsonl = _export_edges_jsonl
    snapshot_cls.export_query_rows_jsonl = _export_query_rows_jsonl


def _retrieve_context(
    self: Any,
    *,
    text_query: str | None = None,
    text_index: str | None = None,
    vector_query: Iterable[float] | None = None,
    vector_index: str | None = None,
    labels: list[str] | None = None,
    edge_type: str | None = None,
    properties: Mapping[str, Any] | None = None,
    radius: int = 1,
    direction: str = "both",
    limit: int = 20,
    text_weight: float = 1.0,
    vector_weight: float = 1.0,
    graph_weight: float = 0.1,
) -> list[dict[str, Any]]:
    """Compose text/vector candidate search with local graph expansion."""

    if radius < 0:
        raise ValueError("radius must be non-negative")
    if limit <= 0:
        raise ValueError("limit must be greater than zero")
    if bool(text_query) != bool(text_index):
        raise ValueError("text_query and text_index must be provided together")
    if (vector_query is None) != (vector_index is None):
        raise ValueError("vector_query and vector_index must be provided together")

    props = dict(properties or {})
    candidate_limit = max(limit * 4, 20)
    candidates: list[tuple[str, int, str, float]] = []
    if text_query and text_index:
        for row in self.search_text(
            text_index,
            text_query,
            labels=labels,
            edge_type=edge_type,
            properties=props,
            limit=candidate_limit,
        ):
            candidates.append((row["kind"], row["id"], "text", float(row["score"])))
    if vector_query is not None and vector_index:
        for row in self.search_vector(
            vector_index,
            list(vector_query),
            labels=labels,
            edge_type=edge_type,
            properties=props,
            limit=candidate_limit,
        ):
            candidates.append((row["kind"], row["id"], "vector", float(row["score"])))

    ranked: dict[tuple[str, int], dict[str, Any]] = {}
    for kind, entity_id, source, score in candidates:
        _accumulate_context(
            self,
            ranked,
            kind,
            entity_id,
            source,
            score,
            0,
            text_weight,
            vector_weight,
            graph_weight,
        )
        seed_nodes = _seed_nodes_for_result(self, kind, entity_id)
        if radius > 0:
            for seed in seed_nodes:
                for expanded_id in self.k_hop(seed, radius, direction=direction):
                    _accumulate_context(
                        self,
                        ranked,
                        "node",
                        expanded_id,
                        source,
                        score,
                        1,
                        text_weight,
                        vector_weight,
                        graph_weight,
                    )

    rows = sorted(
        ranked.values(),
        key=lambda item: (-item["score"], item["distance"], item["kind"], item["id"]),
    )
    return rows[:limit]


def _accumulate_context(
    graph: Any,
    ranked: dict[tuple[str, int], dict[str, Any]],
    kind: str,
    entity_id: int,
    source: str,
    source_score: float,
    distance: int,
    text_weight: float,
    vector_weight: float,
    graph_weight: float,
) -> None:
    key = (kind, entity_id)
    weighted = source_score * (text_weight if source == "text" else vector_weight)
    weighted += graph_weight / (distance + 1)
    row = ranked.get(key)
    if row is None:
        record = graph.get_node(entity_id) if kind == "node" else graph.get_edge(entity_id)
        row = {
            "kind": kind,
            "id": entity_id,
            "score": weighted,
            "distance": distance,
            "source_scores": {},
            "record": record,
        }
        ranked[key] = row
    else:
        row["score"] += weighted
        row["distance"] = min(row["distance"], distance)
    row["source_scores"][source] = max(
        row["source_scores"].get(source, float("-inf")),
        source_score,
    )


def _seed_nodes_for_result(graph: Any, kind: str, entity_id: int) -> list[int]:
    if kind == "node":
        return [entity_id]
    edge = graph.get_edge(entity_id)
    return [edge.source, edge.target]


def _import_nodes_csv(self: Any, path: str | Path) -> list[int]:
    with Path(path).open(newline="", encoding="utf-8") as handle:
        return [
            self.add_node(
                row.get("external_id") or None,
                labels=_parse_labels(row.get("labels")),
                properties=_row_properties(row, _NODE_RESERVED),
            )
            for row in csv.DictReader(handle)
        ]


def _import_edges_csv(self: Any, path: str | Path) -> list[int]:
    with Path(path).open(newline="", encoding="utf-8") as handle:
        ids = []
        for row in csv.DictReader(handle):
            edge_type = row.get("edge_type") or row.get("type")
            if not edge_type:
                raise ValueError("edge CSV rows require edge_type or type")
            ids.append(
                self.add_edge(
                    _resolve_node_ref(self, row.get("source")),
                    _resolve_node_ref(self, row.get("target")),
                    edge_type,
                    properties=_row_properties(row, _EDGE_RESERVED),
                )
            )
        return ids


def _import_nodes_jsonl(self: Any, path: str | Path) -> list[int]:
    ids = []
    for row in _read_jsonl(path):
        ids.append(
            self.add_node(
                row.get("external_id"),
                labels=list(row.get("labels") or []),
                properties=dict(row.get("properties") or {}),
            )
        )
    return ids


def _import_edges_jsonl(self: Any, path: str | Path) -> list[int]:
    ids = []
    for row in _read_jsonl(path):
        edge_type = row.get("edge_type") or row.get("type")
        if not edge_type:
            raise ValueError("edge JSONL rows require edge_type or type")
        ids.append(
            self.add_edge(
                _resolve_node_ref(self, row.get("source")),
                _resolve_node_ref(self, row.get("target")),
                edge_type,
                properties=dict(row.get("properties") or {}),
            )
        )
    return ids


def _export_nodes_jsonl(self: Any, path: str | Path, nodes: Iterable[int] | None = None) -> None:
    node_ids = self.node_ids() if nodes is None else list(nodes)
    _write_jsonl(path, [_record_to_json(self.get_node(node_id)) for node_id in node_ids])


def _export_edges_jsonl(self: Any, path: str | Path, edges: Iterable[int] | None = None) -> None:
    edge_ids = self.edge_ids() if edges is None else list(edges)
    _write_jsonl(path, [_record_to_json(self.get_edge(edge_id)) for edge_id in edge_ids])


def _export_query_rows_jsonl(self: Any, path: str | Path, rows: Iterable[Mapping[str, Any]]) -> None:
    _write_jsonl(path, [_jsonable(dict(row)) for row in rows])


def _row_properties(row: Mapping[str, str], reserved: set[str]) -> dict[str, Any]:
    properties = {}
    if row.get("properties"):
        parsed = json.loads(row["properties"])
        if not isinstance(parsed, dict):
            raise ValueError("properties column must contain a JSON object")
        properties.update(parsed)
    for key, value in row.items():
        if key not in reserved and value not in (None, ""):
            properties[key] = _parse_scalar(value)
    return properties


def _parse_labels(value: str | None) -> list[str]:
    if not value:
        return []
    return [label for label in value.split("|") if label]


def _parse_scalar(value: str) -> Any:
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError:
        return value
    if isinstance(parsed, (str, int, float, bool)):
        return parsed
    return value


def _resolve_node_ref(graph: Any, value: Any) -> int:
    if value is None or value == "":
        raise ValueError("edge rows require source and target")
    try:
        return int(value)
    except (TypeError, ValueError):
        node_id = graph.get_node_id(str(value))
        if node_id is None:
            raise ValueError(f"unknown node reference {value!r}") from None
        return node_id


def _read_jsonl(path: str | Path) -> Iterable[dict[str, Any]]:
    with Path(path).open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            if not isinstance(row, dict):
                raise ValueError(f"JSONL line {line_number} must be an object")
            yield row


def _write_jsonl(path: str | Path, rows: Iterable[Mapping[str, Any]]) -> None:
    with Path(path).open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(_jsonable(row), sort_keys=True) + "\n")


def _record_to_json(record: Any) -> dict[str, Any]:
    if hasattr(record, "edge_type"):
        return {
            "id": record.id,
            "source": record.source,
            "target": record.target,
            "edge_type": record.edge_type,
            "properties": dict(record.properties),
        }
    return {
        "id": record.id,
        "external_id": record.external_id,
        "labels": list(record.labels),
        "properties": dict(record.properties),
    }


def _jsonable(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {key: _jsonable(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_jsonable(item) for item in value]
    if hasattr(value, "id") and (hasattr(value, "labels") or hasattr(value, "edge_type")):
        return _record_to_json(value)
    return value
