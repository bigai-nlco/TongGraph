from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import require_admin
from ..schemas import BackupGraphRequest, CreateGraphRequest, GrantRequest, RestoreBackupRequest, UserCreateRequest, UserTokenRotateRequest, UserUpdateRequest

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


@router.get("/users")
async def admin_users(request: Request) -> dict[str, object]:
    require_admin(request)
    return {"users": request.app.state.registry.list_users()}


@router.get("/users/{user_id}")
async def admin_user(request: Request, user_id: str) -> dict[str, object]:
    require_admin(request)
    return {"user": request.app.state.registry.get_user(user_id)}


@router.post("/users")
async def create_user(request: Request, payload: UserCreateRequest) -> dict[str, object]:
    user = require_admin(request)
    created = request.app.state.registry.create_user(
        payload.user_id,
        token=payload.token,
        admin=payload.admin,
        disabled=payload.disabled,
        graphs=payload.graphs,
        created_by=user.user_id,
    )
    return {"user": created}


@router.patch("/users/{user_id}")
async def update_user(request: Request, user_id: str, payload: UserUpdateRequest) -> dict[str, object]:
    user = require_admin(request)
    updated = request.app.state.registry.update_user(
        user_id,
        admin=payload.admin,
        disabled=payload.disabled,
        graphs=payload.graphs,
        updated_by=user.user_id,
    )
    return {"user": updated}


@router.post("/users/{user_id}/token")
async def rotate_user_token(request: Request, user_id: str, payload: UserTokenRotateRequest) -> dict[str, object]:
    user = require_admin(request)
    return request.app.state.registry.rotate_user_token(user_id, token=payload.token, updated_by=user.user_id)


@router.delete("/users/{user_id}")
async def delete_user(request: Request, user_id: str) -> dict[str, object]:
    require_admin(request)
    request.app.state.registry.delete_user(user_id)
    return {"user": user_id, "deleted": True}


@router.post("/graphs/{graph}/backup")
async def backup_graph(request: Request, graph: str, payload: BackupGraphRequest) -> dict[str, object]:
    require_admin(request)
    return {"backup": request.app.state.registry.backup_graph(graph, note=payload.note)}


@router.get("/backups")
async def list_backups(request: Request) -> dict[str, object]:
    require_admin(request)
    return {"backups": request.app.state.registry.list_backups()}


@router.delete("/backups/{backup_id}")
async def delete_backup(request: Request, backup_id: str) -> dict[str, object]:
    require_admin(request)
    request.app.state.registry.delete_backup(backup_id)
    return {"backup_id": backup_id, "deleted": True}


@router.post("/backups/{backup_id}/restore")
async def restore_backup(request: Request, backup_id: str, payload: RestoreBackupRequest) -> dict[str, object]:
    require_admin(request)
    restored = request.app.state.registry.restore_backup(
        backup_id,
        graph=payload.graph,
        overwrite=payload.overwrite,
        grants=payload.grants,
    )
    return {"graph": restored}
