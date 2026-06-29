from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import require_graph_access
from ..errors import ServerError
from ..logical import assert_record_in_scope, merge_scope_properties, resolve_scope
from ..schemas import (
    FullTextIndexRequest,
    RetrieveContextRequest,
    TextSearchRequest,
    VectorBatchDeleteRequest,
    VectorBatchSearchRequest,
    VectorBatchUpsertRequest,
    VectorIndexRequest,
    VectorSearchRequest,
    VectorUpsertRequest,
)
from ..serialization import serialize

router = APIRouter(prefix="/graphs/{graph}")


def _vector_target(graph_obj, index: str) -> str:
    for item in graph_obj.vector_indexes():
        if dict(item).get("name") == index:
            return str(dict(item).get("target", "node"))
    return "node"


def _assert_entity_scope(graph_obj, index: str, entity_id: int, scope: str | None, graph: str) -> None:
    if scope is None:
        return
    target = _vector_target(graph_obj, index)
    if target == "edge":
        assert_record_in_scope(graph_obj.get_edge(entity_id), scope, kind="edge", graph=graph)
    else:
        assert_record_in_scope(graph_obj.get_node(entity_id), scope, kind="node", graph=graph)


@router.get("/fulltext/indexes")
async def fulltext_indexes(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"indexes": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.fulltext_indexes())}


@router.post("/fulltext/indexes")
async def create_fulltext_index(request: Request, graph: str, payload: FullTextIndexRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        graph_obj.create_fulltext_index(payload.name, payload.properties, target=payload.target, tokenizer=payload.tokenizer)
        return {"created": True, "name": payload.name}

    return request.app.state.registry.call(graph, op)


@router.delete("/fulltext/indexes/{index}")
async def drop_fulltext_index(request: Request, graph: str, index: str) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.call(graph, lambda graph_obj: (graph_obj.drop_fulltext_index(index), {"dropped": True, "name": index})[1])


@router.post("/fulltext/{index}/rebuild")
async def rebuild_fulltext_index(request: Request, graph: str, index: str) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.call(graph, lambda graph_obj: (graph_obj.rebuild_fulltext_index(index), {"rebuilt": True, "name": index})[1])


@router.post("/fulltext/{index}/search")
async def search_text(request: Request, graph: str, index: str, payload: TextSearchRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, payload.logical_graph_id)

    def op(graph_obj):
        return {
            "results": serialize(
                graph_obj.search_text(
                    index,
                    payload.query,
                    mode=payload.mode,
                    labels=payload.labels,
                    edge_type=payload.edge_type,
                    properties=merge_scope_properties(payload.properties, scope),
                    limit=payload.limit,
                    offset=payload.offset,
                )
            )
        }

    return request.app.state.registry.call(graph, op)


@router.post("/retrieve/context")
async def retrieve_context(request: Request, graph: str, payload: RetrieveContextRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, payload.logical_graph_id)

    def op(graph_obj):
        return {
            "results": serialize(
                graph_obj.retrieve_context(
                    text_query=payload.text_query,
                    text_index=payload.text_index,
                    vector_query=payload.vector_query,
                    vector_index=payload.vector_index,
                    labels=payload.labels,
                    edge_type=payload.edge_type,
                    properties=merge_scope_properties(payload.properties, scope),
                    radius=payload.radius,
                    direction=payload.direction,
                    limit=payload.limit,
                    text_weight=payload.text_weight,
                    vector_weight=payload.vector_weight,
                    graph_weight=payload.graph_weight,
                )
            )
        }

    return request.app.state.registry.call(graph, op)


@router.get("/vector/indexes")
async def vector_indexes(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"indexes": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.vector_indexes())}


@router.post("/vector/indexes")
async def create_vector_index(request: Request, graph: str, payload: VectorIndexRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        graph_obj.create_vector_index(payload.name, payload.dimensions, target=payload.target, metric=payload.metric, model=payload.model, model_version=payload.model_version)
        return {"created": True, "name": payload.name}

    return request.app.state.registry.call(graph, op)


@router.delete("/vector/indexes/{index}")
async def drop_vector_index(request: Request, graph: str, index: str) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.call(graph, lambda graph_obj: (graph_obj.drop_vector_index(index), {"dropped": True, "name": index})[1])


@router.put("/vector/{index}/batch")
async def upsert_vectors(request: Request, graph: str, index: str, payload: VectorBatchUpsertRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    scope = resolve_scope(request, graph, payload.logical_graph_id)
    vectors = {int(entity_id): vector for entity_id, vector in payload.vectors.items()}

    def op(graph_obj):
        for entity_id in vectors:
            _assert_entity_scope(graph_obj, index, entity_id, scope, graph)
        graph_obj.upsert_vectors(index, vectors)
        return {"upserted": True, "count": len(vectors)}

    return request.app.state.registry.call(graph, op)


@router.put("/vector/{index}/{entity_id}")
async def upsert_vector(request: Request, graph: str, index: str, entity_id: int, payload: VectorUpsertRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    scope = resolve_scope(request, graph, payload.logical_graph_id)

    def op(graph_obj):
        _assert_entity_scope(graph_obj, index, entity_id, scope, graph)
        graph_obj.upsert_vector(index, entity_id, payload.vector)
        return {"upserted": True}

    return request.app.state.registry.call(graph, op)


@router.get("/vector/{index}/{entity_id}")
async def get_vector(request: Request, graph: str, index: str, entity_id: int, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        _assert_entity_scope(graph_obj, index, entity_id, scope, graph)
        return {"vector": graph_obj.get_vector(index, entity_id)}

    return request.app.state.registry.call(graph, op)


@router.delete("/vector/{index}/{entity_id}")
async def delete_vector(request: Request, graph: str, index: str, entity_id: int, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    scope = resolve_scope(request, graph, logical_graph_id)

    def op(graph_obj):
        _assert_entity_scope(graph_obj, index, entity_id, scope, graph)
        graph_obj.delete_vector(index, entity_id)
        return {"deleted": True}

    return request.app.state.registry.call(graph, op)


@router.post("/vector/{index}/delete-batch")
async def delete_vectors(request: Request, graph: str, index: str, payload: VectorBatchDeleteRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    scope = resolve_scope(request, graph, payload.logical_graph_id)

    def op(graph_obj):
        for entity_id in payload.entity_ids:
            _assert_entity_scope(graph_obj, index, entity_id, scope, graph)
        graph_obj.delete_vectors(index, payload.entity_ids)
        return {"deleted": True, "count": len(payload.entity_ids)}

    return request.app.state.registry.call(graph, op)


@router.post("/vector/{index}/search")
async def search_vector(request: Request, graph: str, index: str, payload: VectorSearchRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, payload.logical_graph_id)

    def op(graph_obj):
        return {
            "results": serialize(
                graph_obj.search_vector(
                    index,
                    payload.query_vector,
                    labels=payload.labels,
                    edge_type=payload.edge_type,
                    properties=merge_scope_properties(payload.properties, scope),
                    min_score=payload.min_score,
                    limit=payload.limit,
                    offset=payload.offset,
                )
            )
        }

    return request.app.state.registry.call(graph, op)


@router.post("/vector/{index}/search-batch")
async def search_vectors(request: Request, graph: str, index: str, payload: VectorBatchSearchRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, payload.logical_graph_id)

    def op(graph_obj):
        return {
            "results": serialize(
                graph_obj.search_vectors(
                    index,
                    payload.query_vectors,
                    labels=payload.labels,
                    edge_type=payload.edge_type,
                    properties=merge_scope_properties(payload.properties, scope),
                    min_score=payload.min_score,
                    limit=payload.limit,
                    offset=payload.offset,
                )
            )
        }

    return request.app.state.registry.call(graph, op)
