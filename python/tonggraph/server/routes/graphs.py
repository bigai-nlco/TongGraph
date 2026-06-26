from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import current_user, require_graph_access

router = APIRouter()


@router.get("/graphs")
async def list_graphs(request: Request) -> dict[str, object]:
    user = current_user(request)
    return {"graphs": request.app.state.registry.visible_graphs(user.user_id, admin=user.admin)}


@router.post("/graphs/{graph}/open")
async def open_graph(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    entry = request.app.state.registry.open_graph(graph)
    return {"graph": entry.name, "open": True}


@router.post("/graphs/{graph}/compact")
async def compact_graph(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    request.app.state.registry.compact(graph)
    return {"graph": graph, "compacted": True}


@router.post("/graphs/{graph}/refresh")
async def refresh_graph(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    request.app.state.registry.refresh(graph)
    return {"graph": graph, "refreshed": True}


@router.get("/graphs/{graph}/stats")
async def graph_stats(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"stats": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.stats())}


@router.get("/graphs/{graph}/schema")
async def graph_schema(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"schema": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.schema())}
