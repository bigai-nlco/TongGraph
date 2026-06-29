from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Request

from ..access import require_graph_access
from ..errors import ServerError
from ..logical import (
    assert_record_in_scope,
    merge_scope_properties,
    record_in_scope,
    reject_reserved_property_mutation,
    resolve_scope,
    scoped_edge_ids,
    scoped_node_ids,
)
from ..schemas import EdgeBatchCreateRequest, EdgeCreateRequest, EdgeUpdateRequest, NodeBatchCreateRequest, NodeCreateRequest, NodeUpdateRequest
from ..serialization import serialize

router = APIRouter(prefix="/graphs/{graph}")


def _value(value: str | None) -> Any:
    if value is None:
        return None
    if value.lower() == "true":
        return True
    if value.lower() == "false":
        return False
    try:
        return int(value)
    except ValueError:
        try:
            return float(value)
        except ValueError:
            return value


def _batch_scope(records: list[Any], payload_scope: str | None) -> str | None:
    scopes = {record.logical_graph_id for record in records if getattr(record, "logical_graph_id", None) is not None}
    if payload_scope is not None:
        scopes.add(payload_scope)
    if len(scopes) > 1:
        raise ServerError("invalid_request", "batch records must use one logical_graph_id")
    return next(iter(scopes)) if scopes else None


@router.get("/nodes/count")
async def node_count(request: Request, graph: str, logical_graph_id: str | None = None) -> dict[str, int]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return {"count": request.app.state.registry.call(graph, lambda graph_obj: len(scoped_node_ids(graph_obj, scope)))}


@router.get("/edges/count")
async def edge_count(request: Request, graph: str, logical_graph_id: str | None = None) -> dict[str, int]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return {"count": request.app.state.registry.call(graph, lambda graph_obj: len(scoped_edge_ids(graph_obj, scope)))}


@router.get("/nodes")
async def node_ids(request: Request, graph: str, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: scoped_node_ids(graph_obj, scope))}


@router.get("/edges")
async def edge_ids(request: Request, graph: str, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: scoped_edge_ids(graph_obj, scope))}


@router.get("/nodes/by-label/{label}")
async def nodes_by_label(request: Request, graph: str, label: str, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        ids = list(graph_obj.nodes_with_label(label))
        if scope is None:
            return ids
        allowed = set(scoped_node_ids(graph_obj, scope))
        return [node_id for node_id in ids if node_id in allowed]

    return {"ids": request.app.state.registry.call(graph, op)}


@router.get("/nodes/by-property")
async def nodes_by_property(request: Request, graph: str, key: str, value: str | None = None, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    parsed = _value(value)

    def op(graph_obj):
        ids = list(graph_obj.nodes_with_property(key, parsed))
        if scope is None:
            return ids
        allowed = set(scoped_node_ids(graph_obj, scope))
        return [node_id for node_id in ids if node_id in allowed]

    return {"ids": request.app.state.registry.call(graph, op)}


@router.get("/nodes/by-external-id/{external_id}")
async def node_by_external_id(request: Request, graph: str, external_id: str, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        node_id = graph_obj.get_node_id(external_id)
        if node_id is None or scope is None:
            return node_id
        return node_id if record_in_scope(graph_obj.get_node(node_id), scope) else None

    return {"id": request.app.state.registry.call(graph, op)}


@router.get("/edges/by-type/{edge_type}")
async def edges_by_type(request: Request, graph: str, edge_type: str, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        ids = list(graph_obj.edges_by_type(edge_type))
        if scope is None:
            return ids
        allowed = set(scoped_edge_ids(graph_obj, scope))
        return [edge_id for edge_id in ids if edge_id in allowed]

    return {"ids": request.app.state.registry.call(graph, op)}


@router.get("/edges/by-property")
async def edges_by_property(request: Request, graph: str, key: str, value: str | None = None, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    parsed = _value(value)

    def op(graph_obj):
        ids = list(graph_obj.edges_with_property(key, parsed))
        if scope is None:
            return ids
        allowed = set(scoped_edge_ids(graph_obj, scope))
        return [edge_id for edge_id in ids if edge_id in allowed]

    return {"ids": request.app.state.registry.call(graph, op)}


@router.post("/nodes")
async def add_node(request: Request, graph: str, payload: NodeCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    reject_reserved_property_mutation(payload.properties)
    scope = resolve_scope(request, graph, payload.logical_graph_id, create=True)

    def op(graph_obj):
        node_id = graph_obj.add_node(payload.external_id, labels=payload.labels, properties=merge_scope_properties(payload.properties, scope))
        return {"id": node_id, "node": serialize(graph_obj.get_node(node_id))}

    return request.app.state.registry.call(graph, op)


@router.post("/nodes/batch")
async def add_nodes(request: Request, graph: str, payload: NodeBatchCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    for record in payload.records:
        reject_reserved_property_mutation(record.properties)
    scope = resolve_scope(request, graph, _batch_scope(payload.records, payload.logical_graph_id), create=True)

    def op(graph_obj):
        records = []
        for record in payload.records:
            item = record.model_dump(exclude_none=True, exclude={"logical_graph_id"})
            item["properties"] = merge_scope_properties(record.properties, scope)
            records.append(item)
        ids = graph_obj.add_nodes(records)
        return {"ids": ids, "nodes": serialize([graph_obj.get_node(node_id) for node_id in ids])}

    return request.app.state.registry.call(graph, op)


@router.get("/nodes/{node_id}")
async def get_node(request: Request, graph: str, node_id: int, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        node = graph_obj.get_node(node_id)
        assert_record_in_scope(node, scope, kind="node", graph=graph)
        return {"node": serialize(node)}

    return request.app.state.registry.call(graph, op)


@router.patch("/nodes/{node_id}")
async def update_node(request: Request, graph: str, node_id: int, payload: NodeUpdateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    reject_reserved_property_mutation(payload.set_properties, payload.remove_properties)
    scope = resolve_scope(request, graph, payload.logical_graph_id)

    def op(graph_obj):
        assert_record_in_scope(graph_obj.get_node(node_id), scope, kind="node", graph=graph)
        node = graph_obj.update_node(
            node_id,
            external_id=payload.external_id,
            add_labels=payload.add_labels,
            remove_labels=payload.remove_labels,
            set_properties=payload.set_properties,
            remove_properties=payload.remove_properties,
        )
        return {"node": serialize(node)}

    return request.app.state.registry.call(graph, op)


@router.delete("/nodes/{node_id}")
async def delete_node(request: Request, graph: str, node_id: int, detach: bool = False, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        assert_record_in_scope(graph_obj.get_node(node_id), scope, kind="node", graph=graph)
        graph_obj.delete_node(node_id, detach=detach)
        return {"deleted": True}

    return request.app.state.registry.call(graph, op)


@router.post("/edges")
async def add_edge(request: Request, graph: str, payload: EdgeCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    reject_reserved_property_mutation(payload.properties)
    scope = resolve_scope(request, graph, payload.logical_graph_id, create=True)

    def op(graph_obj):
        if scope is not None:
            assert_record_in_scope(graph_obj.get_node(payload.source), scope, kind="node", graph=graph)
            assert_record_in_scope(graph_obj.get_node(payload.target), scope, kind="node", graph=graph)
        edge_id = graph_obj.add_edge(payload.source, payload.target, payload.edge_type, properties=merge_scope_properties(payload.properties, scope))
        return {"id": edge_id, "edge": serialize(graph_obj.get_edge(edge_id))}

    return request.app.state.registry.call(graph, op)


@router.post("/edges/batch")
async def add_edges(request: Request, graph: str, payload: EdgeBatchCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    for record in payload.records:
        reject_reserved_property_mutation(record.properties)
    scope = resolve_scope(request, graph, _batch_scope(payload.records, payload.logical_graph_id), create=True)

    def op(graph_obj):
        records = []
        for record in payload.records:
            if scope is not None:
                assert_record_in_scope(graph_obj.get_node(record.source), scope, kind="node", graph=graph)
                assert_record_in_scope(graph_obj.get_node(record.target), scope, kind="node", graph=graph)
            item = record.model_dump(exclude_none=True, exclude={"logical_graph_id"})
            item["properties"] = merge_scope_properties(record.properties, scope)
            records.append(item)
        ids = graph_obj.add_edges(records)
        return {"ids": ids, "edges": serialize([graph_obj.get_edge(edge_id) for edge_id in ids])}

    return request.app.state.registry.call(graph, op)


@router.get("/edges/{edge_id}")
async def get_edge(request: Request, graph: str, edge_id: int, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        edge = graph_obj.get_edge(edge_id)
        assert_record_in_scope(edge, scope, kind="edge", graph=graph)
        return {"edge": serialize(edge)}

    return request.app.state.registry.call(graph, op)


@router.patch("/edges/{edge_id}")
async def update_edge(request: Request, graph: str, edge_id: int, payload: EdgeUpdateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    reject_reserved_property_mutation(payload.set_properties, payload.remove_properties)
    scope = resolve_scope(request, graph, payload.logical_graph_id)

    def op(graph_obj):
        assert_record_in_scope(graph_obj.get_edge(edge_id), scope, kind="edge", graph=graph)
        edge = graph_obj.update_edge(edge_id, set_properties=payload.set_properties, remove_properties=payload.remove_properties)
        return {"edge": serialize(edge)}

    return request.app.state.registry.call(graph, op)


@router.delete("/edges/{edge_id}")
async def delete_edge(request: Request, graph: str, edge_id: int, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        assert_record_in_scope(graph_obj.get_edge(edge_id), scope, kind="edge", graph=graph)
        graph_obj.delete_edge(edge_id)
        return {"deleted": True}

    return request.app.state.registry.call(graph, op)
