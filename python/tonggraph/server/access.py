"""Graph ACL checks."""

from __future__ import annotations

from fastapi import Request

from .auth import User, authenticate
from .errors import ServerError


_LEVELS = {"read": 1, "write": 2}


def current_user(request: Request) -> User:
    user = getattr(request.state, "user", None)
    if user is None:
        user = authenticate(request)
        request.state.user = user
    return user


def require_admin(request: Request) -> User:
    user = current_user(request)
    if not user.admin:
        raise ServerError("admin_required", "administrator privileges are required", status_code=403)
    return user


def require_graph_access(request: Request, graph: str, access: str) -> User:
    user = current_user(request)
    if user.admin:
        return user
    if request.app.state.config.auth_mode == "none":
        return user
    required = _LEVELS[access]
    granted = request.app.state.registry.access_for(user.user_id, graph)
    if granted is None or _LEVELS[granted] < required:
        raise ServerError("permission_denied", f"user {user.user_id!r} cannot {access} graph {graph!r}", status_code=403, graph=graph)
    return user
