from __future__ import annotations

from fastapi import APIRouter, Request

from tonggraph import __version__

router = APIRouter()


@router.get("/health")
async def health(request: Request) -> dict[str, object]:
    registry = request.app.state.registry
    return {"status": "ok", "version": __version__, "graphs": len(registry.graphs)}
