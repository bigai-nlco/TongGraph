"""Logical graph namespace helpers for server routes."""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Mapping

from fastapi import Request

from .access import current_user
from .config import validate_graph_name
from .errors import ServerError

LOGICAL_GRAPH_PROPERTY = "_tg_logical_graph_id"


def validate_logical_graph_id(logical_graph_id: str) -> str:
    return validate_graph_name(logical_graph_id)


def resolve_scope(
    request: Request,
    graph: str,
    logical_graph_id: str | None,
    *,
    required_for_non_admin: bool = True,
    create: bool = False,
) -> str | None:
    if logical_graph_id is not None:
        logical_graph_id = validate_logical_graph_id(str(logical_graph_id))
    if not request.app.state.registry.logical_graphs_enabled(graph):
        if logical_graph_id is not None:
            raise ServerError("invalid_request", "logical_graph_id is only valid for logical-graph-enabled graphs", graph=graph)
        return None
    user = current_user(request)
    if logical_graph_id is None:
        if required_for_non_admin and not user.admin:
            raise ServerError(
                "logical_graph_required",
                "logical_graph_id is required for this graph",
                status_code=400,
                graph=graph,
            )
        return None
    if create:
        request.app.state.registry.ensure_logical_graph(graph, logical_graph_id, created_by=user.user_id)
    else:
        request.app.state.registry.get_logical_graph(graph, logical_graph_id)
    return logical_graph_id


def merge_scope_properties(properties: Mapping[str, Any] | None, logical_graph_id: str | None) -> dict[str, Any] | None:
    if logical_graph_id is None:
        return dict(properties or {}) if properties is not None else None
    merged = dict(properties or {})
    existing = merged.get(LOGICAL_GRAPH_PROPERTY)
    if existing is not None and existing != logical_graph_id:
        raise ServerError("invalid_request", "logical graph property conflicts with logical_graph_id")
    merged[LOGICAL_GRAPH_PROPERTY] = logical_graph_id
    return merged


def reject_reserved_property_mutation(
    properties: Mapping[str, Any] | None = None,
    remove_properties: list[str] | None = None,
) -> None:
    if properties and LOGICAL_GRAPH_PROPERTY in properties:
        raise ServerError("invalid_request", f"{LOGICAL_GRAPH_PROPERTY!r} is a reserved server property")
    if remove_properties and LOGICAL_GRAPH_PROPERTY in remove_properties:
        raise ServerError("invalid_request", f"{LOGICAL_GRAPH_PROPERTY!r} is a reserved server property")


def record_in_scope(record: Any, logical_graph_id: str | None) -> bool:
    if logical_graph_id is None:
        return True
    return dict(getattr(record, "properties", {})).get(LOGICAL_GRAPH_PROPERTY) == logical_graph_id


def assert_record_in_scope(record: Any, logical_graph_id: str | None, *, kind: str, graph: str) -> None:
    if not record_in_scope(record, logical_graph_id):
        record_id = getattr(record, "id", "unknown")
        raise ServerError("not_found", f"{kind} {record_id!r} not found", status_code=404, graph=graph)


def scoped_node_ids(graph_obj: Any, logical_graph_id: str | None) -> list[int]:
    if logical_graph_id is None:
        return list(graph_obj.node_ids())
    return list(graph_obj.nodes_with_property(LOGICAL_GRAPH_PROPERTY, logical_graph_id))


def scoped_edge_ids(graph_obj: Any, logical_graph_id: str | None) -> list[int]:
    if logical_graph_id is None:
        return list(graph_obj.edge_ids())
    return list(graph_obj.edges_with_property(LOGICAL_GRAPH_PROPERTY, logical_graph_id))


def scoped_graph_view(graph_obj: Any, logical_graph_id: str | None) -> Any:
    if logical_graph_id is None:
        return graph_obj
    return graph_obj.subgraph(scoped_node_ids(graph_obj, logical_graph_id))


def inject_query_scope(spec: Mapping[str, Any], logical_graph_id: str | None) -> dict[str, Any]:
    scoped = deepcopy(dict(spec))
    if logical_graph_id is None:
        return scoped
    match = scoped.get("match")
    if not isinstance(match, list):
        return scoped
    for pattern in match:
        if not isinstance(pattern, dict):
            continue
        if "node" not in pattern and "edge" not in pattern:
            continue
        _reject_conflicting_where(pattern, logical_graph_id)
        properties = dict(pattern.get("properties") or {})
        existing = properties.get(LOGICAL_GRAPH_PROPERTY)
        if existing is not None and existing != logical_graph_id:
            raise ServerError("invalid_request", "query scope conflicts with logical_graph_id")
        properties[LOGICAL_GRAPH_PROPERTY] = logical_graph_id
        pattern["properties"] = properties
    return scoped


def _reject_conflicting_where(pattern: dict[str, Any], logical_graph_id: str) -> None:
    for clause in pattern.get("where") or []:
        if not isinstance(clause, dict) or clause.get("property") != LOGICAL_GRAPH_PROPERTY:
            continue
        op = clause.get("op", "eq")
        value = clause.get("value")
        if op != "eq" or value != logical_graph_id:
            raise ServerError("invalid_request", "query scope conflicts with logical_graph_id")


def metadata(logical_graph_id: str, *, created_by: str | None, created_at: float, updated_at: float) -> dict[str, Any]:
    return {
        "logical_graph_id": logical_graph_id,
        "created_by": created_by,
        "created_at": created_at,
        "updated_at": updated_at,
    }
