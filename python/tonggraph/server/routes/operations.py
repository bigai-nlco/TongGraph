from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import require_admin

router = APIRouter()


@router.get("/metrics")
async def metrics(request: Request) -> dict[str, object]:
    if request.app.state.config.auth_mode != "none":
        require_admin(request)
    return {
        "requests": request.app.state.metrics.snapshot(),
        "graphs": request.app.state.registry.graph_summary(),
    }
