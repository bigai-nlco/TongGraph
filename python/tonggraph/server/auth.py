"""Authentication helpers for TongGraph server."""

from __future__ import annotations

from dataclasses import dataclass

from fastapi import Request

from .config import ServerConfig
from .errors import ServerError


@dataclass(frozen=True)
class User:
    user_id: str
    admin: bool = False


def authenticate(request: Request) -> User:
    config: ServerConfig = request.app.state.config
    if config.auth_mode == "none":
        return User("anonymous", admin=True)

    header = request.headers.get("authorization", "")
    prefix = "Bearer "
    if not header.startswith(prefix):
        raise ServerError("unauthenticated", "missing bearer token", status_code=401)
    token = header[len(prefix):].strip()
    for user in config.users.values():
        if user.token and user.token == token:
            return User(user.user_id, admin=user.admin)
    raise ServerError("unauthenticated", "invalid bearer token", status_code=401)
