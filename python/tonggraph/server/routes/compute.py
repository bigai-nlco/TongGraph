from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import require_graph_access
from ..logical import resolve_scope, scoped_graph_view
from ..schemas import ComputeBatchRequest, FrontierRequest, SubgraphRequest
from ..serialization import serialize

router = APIRouter(prefix="/graphs/{graph}")


def _view(graph_obj, scope):
    return scoped_graph_view(graph_obj, scope)


@router.get("/traversal/neighbors/{node_id}")
async def neighbors(request: Request, graph: str, node_id: int, direction: str = "out", edge_type: str | None = None, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {"ids": serialize(_view(graph_obj, scope).neighbors(node_id, direction=direction, edge_type=edge_type))},
    )


@router.get("/traversal/k-hop")
async def k_hop(request: Request, graph: str, start: int, hops: int, direction: str = "out", edge_type: str | None = None, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {"ids": serialize(_view(graph_obj, scope).k_hop(start, hops, direction=direction, edge_type=edge_type))},
    )


@router.post("/traversal/frontier")
async def frontier(request: Request, graph: str, payload: FrontierRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, payload.logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "ids": serialize(
                _view(graph_obj, scope).frontier(
                    payload.starts,
                    payload.steps,
                    direction=payload.direction,
                    edge_type=payload.edge_type,
                )
            )
        },
    )


@router.get("/algorithms/bfs")
async def bfs(
    request: Request,
    graph: str,
    start: int,
    direction: str = "out",
    edge_type: str | None = None,
    max_depth: int | None = None,
    logical_graph_id: str | None = None,
) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "ids": serialize(_view(graph_obj, scope).bfs(start, direction=direction, edge_type=edge_type, max_depth=max_depth))
        },
    )


@router.get("/algorithms/shortest-path")
async def shortest_path(
    request: Request,
    graph: str,
    start: int,
    target: int,
    direction: str = "out",
    edge_type: str | None = None,
    weight_property: str | None = None,
    logical_graph_id: str | None = None,
) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "path": serialize(
                _view(graph_obj, scope).shortest_path(
                    start,
                    target,
                    direction=direction,
                    edge_type=edge_type,
                    weight_property=weight_property,
                )
            )
        },
    )


@router.get("/algorithms/connected-components")
async def connected_components(request: Request, graph: str, edge_type: str | None = None, logical_graph_id: str | None = None) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {"components": serialize(_view(graph_obj, scope).connected_components(edge_type=edge_type))},
    )


@router.get("/algorithms/pagerank")
async def pagerank(
    request: Request,
    graph: str,
    iterations: int = 20,
    damping: float = 0.85,
    tolerance: float | None = None,
    edge_type: str | None = None,
    logical_graph_id: str | None = None,
) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "scores": serialize(
                _view(graph_obj, scope).pagerank(
                    iterations=iterations,
                    damping=damping,
                    tolerance=tolerance,
                    edge_type=edge_type,
                )
            )
        },
    )


@router.get("/algorithms/random-walk")
async def random_walk(
    request: Request,
    graph: str,
    start: int,
    steps: int,
    direction: str = "out",
    edge_type: str | None = None,
    seed: int | None = None,
    logical_graph_id: str | None = None,
) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "ids": serialize(
                _view(graph_obj, scope).random_walk(start, steps, direction=direction, edge_type=edge_type, seed=seed)
            )
        },
    )


@router.post("/subgraph")
async def subgraph(request: Request, graph: str, payload: SubgraphRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, payload.logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {"snapshot": serialize(_view(graph_obj, scope).subgraph(payload.nodes, edge_type=payload.edge_type))},
    )


@router.post("/compute/batch")
async def compute_batch(request: Request, graph: str, payload: ComputeBatchRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    scope = resolve_scope(request, graph, payload.logical_graph_id)
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {"results": serialize(_view(graph_obj, scope).compute_batch(payload.jobs))},
    )
