from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import current_user, require_graph_access
from ..schemas import LogicalGraphCreateRequest

router = APIRouter(prefix="/graphs/{graph}/logical-graphs")


@router.get("")
async def list_logical_graphs(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"logical_graphs": request.app.state.registry.list_logical_graphs(graph)}


@router.post("")
async def create_logical_graph(request: Request, graph: str, payload: LogicalGraphCreateRequest) -> dict[str, object]:
    user = require_graph_access(request, graph, "write")
    logical_graph = request.app.state.registry.ensure_logical_graph(
        graph, payload.logical_graph_id, created_by=user.user_id
    )
    return {"logical_graph": logical_graph}


@router.get("/{logical_graph_id}")
async def get_logical_graph(request: Request, graph: str, logical_graph_id: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"logical_graph": request.app.state.registry.get_logical_graph(graph, logical_graph_id)}


@router.delete("/{logical_graph_id}")
async def delete_logical_graph(request: Request, graph: str, logical_graph_id: str) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    return request.app.state.registry.delete_logical_graph(graph, logical_graph_id)
