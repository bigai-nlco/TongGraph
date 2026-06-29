from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import current_user, require_graph_access
from ..schemas import ComputeBatchRequest, CypherRequest, QueryRequest, SnapshotCreateRequest, TextSearchRequest, VectorBatchSearchRequest, VectorSearchRequest
from ..serialization import serialize

router = APIRouter(prefix="/graphs/{graph}/snapshots")


def _user_for_snapshot(request: Request, graph: str):  # type: ignore[no-untyped-def]
    require_graph_access(request, graph, "read")
    return current_user(request)


@router.post("")
async def create_snapshot(request: Request, graph: str, payload: SnapshotCreateRequest = SnapshotCreateRequest()) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    snapshot = request.app.state.registry.create_snapshot(graph, user.user_id, ttl_seconds=payload.ttl_seconds)
    return {"snapshot": snapshot}


@router.get("")
async def list_snapshots(request: Request, graph: str) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {"snapshots": request.app.state.registry.list_snapshots(graph, user.user_id, admin=user.admin)}


@router.delete("/{snapshot_id}")
async def delete_snapshot(request: Request, graph: str, snapshot_id: str) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    request.app.state.registry.delete_snapshot(graph, snapshot_id, user.user_id, admin=user.admin)
    return {"deleted": True, "snapshot_id": snapshot_id}


@router.get("/{snapshot_id}/stats")
async def snapshot_stats(request: Request, graph: str, snapshot_id: str) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "stats": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.stats())
        )
    }


@router.get("/{snapshot_id}/schema")
async def snapshot_schema(request: Request, graph: str, snapshot_id: str) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "schema": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.schema())
        )
    }


@router.get("/{snapshot_id}/nodes/count")
async def snapshot_node_count(request: Request, graph: str, snapshot_id: str) -> dict[str, int]:
    user = _user_for_snapshot(request, graph)
    return {
        "count": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: snapshot.node_count()
        )
    }


@router.get("/{snapshot_id}/edges/count")
async def snapshot_edge_count(request: Request, graph: str, snapshot_id: str) -> dict[str, int]:
    user = _user_for_snapshot(request, graph)
    return {
        "count": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: snapshot.edge_count()
        )
    }


@router.get("/{snapshot_id}/nodes")
async def snapshot_node_ids(request: Request, graph: str, snapshot_id: str) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "ids": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.node_ids())
        )
    }


@router.get("/{snapshot_id}/edges")
async def snapshot_edge_ids(request: Request, graph: str, snapshot_id: str) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "ids": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.edge_ids())
        )
    }


@router.get("/{snapshot_id}/nodes/{node_id}")
async def snapshot_get_node(request: Request, graph: str, snapshot_id: str, node_id: int) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "node": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.get_node(node_id))
        )
    }


@router.get("/{snapshot_id}/edges/{edge_id}")
async def snapshot_get_edge(request: Request, graph: str, snapshot_id: str, edge_id: int) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "edge": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.get_edge(edge_id))
        )
    }


@router.post("/{snapshot_id}/query")
async def snapshot_query(request: Request, graph: str, snapshot_id: str, payload: QueryRequest) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "result": request.app.state.registry.call_snapshot(
            graph,
            snapshot_id,
            user.user_id,
            admin=user.admin,
            func=lambda snapshot: serialize(snapshot.query(payload.spec, profile=payload.profile)),
        )
    }


@router.post("/{snapshot_id}/cypher")
async def snapshot_cypher(request: Request, graph: str, snapshot_id: str, payload: CypherRequest) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "result": request.app.state.registry.call_snapshot(
            graph,
            snapshot_id,
            user.user_id,
            admin=user.admin,
            func=lambda snapshot: serialize(snapshot.cypher(payload.query, payload.parameters, profile=payload.profile)),
        )
    }


@router.post("/{snapshot_id}/compute/batch")
async def snapshot_compute_batch(request: Request, graph: str, snapshot_id: str, payload: ComputeBatchRequest) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "results": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.compute_batch(payload.jobs))
        )
    }


@router.get("/{snapshot_id}/fulltext/indexes")
async def snapshot_fulltext_indexes(request: Request, graph: str, snapshot_id: str) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "indexes": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.fulltext_indexes())
        )
    }


@router.post("/{snapshot_id}/fulltext/{index}/search")
async def snapshot_search_text(request: Request, graph: str, snapshot_id: str, index: str, payload: TextSearchRequest) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)

    def search(snapshot):
        return serialize(
            snapshot.search_text(
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

    return {
        "results": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=search
        )
    }


@router.get("/{snapshot_id}/vector/indexes")
async def snapshot_vector_indexes(request: Request, graph: str, snapshot_id: str) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "indexes": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.vector_indexes())
        )
    }


@router.get("/{snapshot_id}/vector/{index}/{entity_id}")
async def snapshot_get_vector(request: Request, graph: str, snapshot_id: str, index: str, entity_id: int) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)
    return {
        "vector": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=lambda snapshot: serialize(snapshot.get_vector(index, entity_id))
        )
    }


@router.post("/{snapshot_id}/vector/{index}/search")
async def snapshot_search_vector(request: Request, graph: str, snapshot_id: str, index: str, payload: VectorSearchRequest) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)

    def search(snapshot):
        return serialize(
            snapshot.search_vector(
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

    return {
        "results": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=search
        )
    }


@router.post("/{snapshot_id}/vector/{index}/search-batch")
async def snapshot_search_vectors(request: Request, graph: str, snapshot_id: str, index: str, payload: VectorBatchSearchRequest) -> dict[str, object]:
    user = _user_for_snapshot(request, graph)

    def search(snapshot):
        return serialize(
            snapshot.search_vectors(
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

    return {
        "results": request.app.state.registry.call_snapshot(
            graph, snapshot_id, user.user_id, admin=user.admin, func=search
        )
    }
