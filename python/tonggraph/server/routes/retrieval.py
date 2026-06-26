from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import require_graph_access
from ..schemas import FullTextIndexRequest, TextSearchRequest, VectorBatchSearchRequest, VectorIndexRequest, VectorSearchRequest, VectorUpsertRequest
from ..serialization import serialize

router = APIRouter(prefix="/graphs/{graph}")


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

    def op(graph_obj):
        return {
            "results": serialize(
                graph_obj.search_text(
                    index,
                    payload.query,
                    mode=payload.mode,
                    labels=payload.labels,
                    edge_type=payload.edge_type,
                    properties=payload.properties,
                    limit=payload.limit,
                    offset=payload.offset,
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


@router.put("/vector/{index}/{entity_id}")
async def upsert_vector(request: Request, graph: str, index: str, entity_id: int, payload: VectorUpsertRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.call(graph, lambda graph_obj: (graph_obj.upsert_vector(index, entity_id, payload.vector), {"upserted": True})[1])


@router.get("/vector/{index}/{entity_id}")
async def get_vector(request: Request, graph: str, index: str, entity_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"vector": graph_obj.get_vector(index, entity_id)})


@router.delete("/vector/{index}/{entity_id}")
async def delete_vector(request: Request, graph: str, index: str, entity_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.call(graph, lambda graph_obj: (graph_obj.delete_vector(index, entity_id), {"deleted": True})[1])


@router.post("/vector/{index}/search")
async def search_vector(request: Request, graph: str, index: str, payload: VectorSearchRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")

    def op(graph_obj):
        return {
            "results": serialize(
                graph_obj.search_vector(
                    index,
                    payload.query_vector,
                    labels=payload.labels,
                    edge_type=payload.edge_type,
                    properties=payload.properties,
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

    def op(graph_obj):
        return {
            "results": serialize(
                graph_obj.search_vectors(
                    index,
                    payload.query_vectors,
                    labels=payload.labels,
                    edge_type=payload.edge_type,
                    properties=payload.properties,
                    min_score=payload.min_score,
                    limit=payload.limit,
                    offset=payload.offset,
                )
            )
        }

    return request.app.state.registry.call(graph, op)
