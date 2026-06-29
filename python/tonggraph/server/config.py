"""Configuration loading for the optional TongGraph server."""

from __future__ import annotations

import json
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping

import yaml

from .errors import ServerError


@dataclass(frozen=True)
class UserConfig:
    user_id: str
    admin: bool = False
    token: str | None = None
    graphs: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class OperationsConfig:
    request_logging: bool = True
    request_timeout_seconds: float | None = None
    metrics: bool = True


@dataclass(frozen=True)
class ServerConfig:
    host: str = "127.0.0.1"
    port: int = 8719
    data_dir: Path = Path(".tonggraph")
    graphs: dict[str, Path] = field(default_factory=dict)
    auth_mode: str = "none"
    users: dict[str, UserConfig] = field(default_factory=dict)
    operations: OperationsConfig = field(default_factory=OperationsConfig)


def load_config(path: str | Path | None = None) -> ServerConfig:
    if path is None:
        return parse_config({})
    config_path = Path(path)
    with config_path.open("r", encoding="utf-8") as handle:
        if config_path.suffix.lower() == ".json":
            raw = json.load(handle)
        else:
            raw = yaml.safe_load(handle) or {}
    return parse_config(raw, base_dir=config_path.parent)


def parse_config(raw: Mapping[str, Any], *, base_dir: Path | None = None) -> ServerConfig:
    base_dir = base_dir or Path.cwd()
    host = str(raw.get("host", "127.0.0.1"))
    port = int(raw.get("port", 8719))
    data_dir = _resolve_data_dir(raw.get("data_dir", ".tonggraph"), base_dir)
    data_dir.mkdir(parents=True, exist_ok=True)

    graphs = {}
    for name, path in dict(raw.get("graphs") or {}).items():
        graph_name = validate_graph_name(str(name))
        graphs[graph_name] = resolve_graph_path(data_dir, str(path))

    auth = dict(raw.get("auth") or {})
    auth_mode = str(auth.get("mode", "none"))
    if auth_mode not in {"none", "token"}:
        raise ServerError("invalid_request", "auth.mode must be 'none' or 'token'")
    users = _parse_users(auth.get("users") or {})
    operations = _parse_operations(raw.get("operations") or {})
    return ServerConfig(host=host, port=port, data_dir=data_dir, graphs=graphs, auth_mode=auth_mode, users=users, operations=operations)


def validate_graph_name(name: str) -> str:
    if not name:
        raise ServerError("invalid_request", "graph name cannot be empty")
    if not all(ch.isalnum() or ch in {"_", "-"} for ch in name):
        raise ServerError("invalid_request", "graph name may only contain letters, digits, '_' or '-'")
    return name


def resolve_graph_path(data_dir: Path, value: str) -> Path:
    path = Path(value)
    if path.is_absolute():
        raise ServerError("invalid_request", "graph paths must be relative to data_dir")
    resolved = (data_dir / path).resolve()
    root = data_dir.resolve()
    if root != resolved and root not in resolved.parents:
        raise ServerError("invalid_request", "graph path escapes data_dir")
    resolved.parent.mkdir(parents=True, exist_ok=True)
    return resolved


def default_graph_path(data_dir: Path, name: str) -> Path:
    return resolve_graph_path(data_dir, f"{validate_graph_name(name)}.db")


def _resolve_data_dir(value: Any, base_dir: Path) -> Path:
    path = Path(str(value))
    if not path.is_absolute():
        path = base_dir / path
    return path.resolve()


def _parse_users(raw_users: Mapping[str, Any]) -> dict[str, UserConfig]:
    users = {}
    for user_id, value in raw_users.items():
        payload = dict(value or {})
        token = payload.get("token")
        token_env = payload.get("token_env")
        if token is None and token_env:
            token = os.environ.get(str(token_env))
        graphs = {str(name): _normalize_access(access) for name, access in dict(payload.get("graphs") or {}).items()}
        users[str(user_id)] = UserConfig(
            user_id=str(user_id),
            admin=bool(payload.get("admin", False)),
            token=str(token) if token is not None else None,
            graphs=graphs,
        )
    return users


def _normalize_access(value: Any) -> str:
    access = str(value)
    if access not in {"read", "write"}:
        raise ServerError("invalid_request", "graph access must be 'read' or 'write'")
    return access


def _parse_operations(raw_operations: Mapping[str, Any]) -> OperationsConfig:
    timeout = raw_operations.get("request_timeout_seconds")
    if timeout is not None:
        timeout = float(timeout)
        if timeout <= 0:
            raise ServerError("invalid_request", "operations.request_timeout_seconds must be positive")
    return OperationsConfig(
        request_logging=bool(raw_operations.get("request_logging", True)),
        request_timeout_seconds=timeout,
        metrics=bool(raw_operations.get("metrics", True)),
    )
