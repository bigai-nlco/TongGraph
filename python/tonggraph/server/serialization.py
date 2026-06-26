"""Convert TongGraph SDK records into JSON-compatible objects."""

from __future__ import annotations

from typing import Any


def serialize(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, list):
        return [serialize(item) for item in value]
    if isinstance(value, tuple):
        return [serialize(item) for item in value]
    if isinstance(value, dict):
        return {str(key): serialize(item) for key, item in value.items()}
    if _has_attrs(value, ["id", "external_id", "labels", "properties"]):
        return {
            "id": value.id,
            "external_id": value.external_id or None,
            "labels": list(value.labels),
            "properties": serialize(dict(value.properties)),
        }
    if _has_attrs(value, ["id", "source", "target", "edge_type", "properties"]):
        return {
            "id": value.id,
            "source": value.source,
            "target": value.target,
            "edge_type": value.edge_type,
            "properties": serialize(dict(value.properties)),
        }
    if _has_attrs(value, ["node_count", "edge_count", "node_ids", "edge_ids", "nodes", "edges"]):
        return {
            "node_count": value.node_count(),
            "edge_count": value.edge_count(),
            "node_ids": serialize(value.node_ids()),
            "edge_ids": serialize(value.edge_ids()),
            "nodes": serialize(value.nodes()),
            "edges": serialize(value.edges()),
        }
    if _has_attrs(value, ["keys", "records", "summary"]):
        return {
            "keys": serialize(value.keys),
            "records": serialize(value.records),
            "summary": serialize(value.summary),
            "profile": serialize(getattr(value, "profile", None)),
        }
    return value


def _has_attrs(value: Any, attrs: list[str]) -> bool:
    return all(hasattr(value, attr) for attr in attrs)
