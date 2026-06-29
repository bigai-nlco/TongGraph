from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Request

from ..access import require_graph_access
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


@router.get("/nodes/count")
async def node_count(request: Request, graph: str) -> dict[str, int]:
    require_graph_access(request, graph, "read")
    return {"count": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.node_count())}


@router.get("/edges/count")
async def edge_count(request: Request, graph: str) -> dict[str, int]:
    require_graph_access(request, graph, "read")
    return {"count": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.edge_count())}


@router.get("/nodes")
async def node_ids(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.node_ids())}


@router.get("/edges")
async def edge_ids(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.edge_ids())}


@router.get("/nodes/by-label/{label}")
async def nodes_by_label(request: Request, graph: str, label: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.nodes_with_label(label))}


@router.get("/nodes/by-property")
async def nodes_by_property(request: Request, graph: str, key: str, value: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    parsed = _value(value)
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.nodes_with_property(key, parsed))}


@router.get("/nodes/by-external-id/{external_id}")
async def node_by_external_id(request: Request, graph: str, external_id: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"id": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.get_node_id(external_id))}


@router.get("/edges/by-type/{edge_type}")
async def edges_by_type(request: Request, graph: str, edge_type: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.edges_by_type(edge_type))}


@router.get("/edges/by-property")
async def edges_by_property(request: Request, graph: str, key: str, value: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    parsed = _value(value)
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.edges_with_property(key, parsed))}


@router.post("/nodes")
async def add_node(request: Request, graph: str, payload: NodeCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        node_id = graph_obj.add_node(payload.external_id, labels=payload.labels, properties=payload.properties)
        return {"id": node_id, "node": serialize(graph_obj.get_node(node_id))}

    return request.app.state.registry.call(graph, op)


@router.post("/nodes/batch")
async def add_nodes(request: Request, graph: str, payload: NodeBatchCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        records = [record.model_dump(exclude_none=True) for record in payload.records]
        ids = graph_obj.add_nodes(records)
        return {"ids": ids, "nodes": serialize([graph_obj.get_node(node_id) for node_id in ids])}

    return request.app.state.registry.call(graph, op)


@router.get("/nodes/{node_id}")
async def get_node(request: Request, graph: str, node_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"node": serialize(graph_obj.get_node(node_id))})


@router.patch("/nodes/{node_id}")
async def update_node(request: Request, graph: str, node_id: int, payload: NodeUpdateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
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
async def delete_node(request: Request, graph: str, node_id: int, detach: bool = False) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.call(graph, lambda graph_obj: (graph_obj.delete_node(node_id, detach=detach), {"deleted": True})[1])


@router.post("/edges")
async def add_edge(request: Request, graph: str, payload: EdgeCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        edge_id = graph_obj.add_edge(payload.source, payload.target, payload.edge_type, properties=payload.properties)
        return {"id": edge_id, "edge": serialize(graph_obj.get_edge(edge_id))}

    return request.app.state.registry.call(graph, op)


@router.post("/edges/batch")
async def add_edges(request: Request, graph: str, payload: EdgeBatchCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        records = [record.model_dump(exclude_none=True) for record in payload.records]
        ids = graph_obj.add_edges(records)
        return {"ids": ids, "edges": serialize([graph_obj.get_edge(edge_id) for edge_id in ids])}

    return request.app.state.registry.call(graph, op)


@router.get("/edges/{edge_id}")
async def get_edge(request: Request, graph: str, edge_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"edge": serialize(graph_obj.get_edge(edge_id))})


@router.patch("/edges/{edge_id}")
async def update_edge(request: Request, graph: str, edge_id: int, payload: EdgeUpdateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {"edge": serialize(graph_obj.update_edge(edge_id, set_properties=payload.set_properties, remove_properties=payload.remove_properties))},
    )


@router.delete("/edges/{edge_id}")
async def delete_edge(request: Request, graph: str, edge_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.call(graph, lambda graph_obj: (graph_obj.delete_edge(edge_id), {"deleted": True})[1])
