"""Optional TongGraph local server package."""

from __future__ import annotations

from typing import Any

__all__ = [
    "create_app",
    "TongGraphClient",
    "TongGraphServerError",
    "RemoteGraph",
    "RemoteLogicalGraph",
    "RemoteSnapshot",
]


def __getattr__(name: str) -> Any:
    if name == "create_app":
        from .app import create_app

        return create_app
    if name in {"TongGraphClient", "TongGraphServerError", "RemoteGraph", "RemoteLogicalGraph", "RemoteSnapshot"}:
        from .client import RemoteGraph, RemoteLogicalGraph, RemoteSnapshot, TongGraphClient, TongGraphServerError

        return {
            "TongGraphClient": TongGraphClient,
            "TongGraphServerError": TongGraphServerError,
            "RemoteGraph": RemoteGraph,
            "RemoteLogicalGraph": RemoteLogicalGraph,
            "RemoteSnapshot": RemoteSnapshot,
        }[name]
    raise AttributeError(name)
