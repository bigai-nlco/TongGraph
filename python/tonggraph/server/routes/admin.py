from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import require_admin
from ..schemas import CreateGraphRequest, GrantRequest

router = APIRouter(prefix="/admin")


@router.get("/graphs")
async def admin_graphs(request: Request) -> dict[str, object]:
    require_admin(request)
    return {"graphs": request.app.state.registry.list_graphs()}


@router.post("/graphs")
async def create_graph(request: Request, payload: CreateGraphRequest) -> dict[str, object]:
    user = require_admin(request)
    entry = request.app.state.registry.create_graph(payload.name, created_by=user.user_id, grants=payload.grants)
    return {"graph": {"name": entry.name, "path": str(entry.path), "open": entry.worker is not None, "created_by": entry.created_by}}


@router.post("/graphs/{graph}/grants")
async def grant_graph(request: Request, graph: str, payload: GrantRequest) -> dict[str, object]:
    require_admin(request)
    request.app.state.registry.grant(payload.user, graph, payload.access)
    return {"graph": graph, "user": payload.user, "access": payload.access}


@router.delete("/graphs/{graph}/grants/{user_id}")
async def revoke_graph(request: Request, graph: str, user_id: str) -> dict[str, object]:
    require_admin(request)
    request.app.state.registry.revoke(user_id, graph)
    return {"graph": graph, "user": user_id, "revoked": True}
