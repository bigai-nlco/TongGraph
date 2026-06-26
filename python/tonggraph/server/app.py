"""ASGI app factory for TongGraph server."""

from __future__ import annotations

import uuid

from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError

from .config import ServerConfig, load_config
from .errors import ServerError, key_error_handler, server_error_handler, validation_error_handler, value_error_handler
from .registry import GraphRegistry
from .routes import admin, compute, graphs, health, query, records, retrieval, snapshots


def create_app(config: ServerConfig | str | None = None) -> FastAPI:
    server_config = load_config(config) if not isinstance(config, ServerConfig) else config
    @asynccontextmanager
    async def lifespan(app: FastAPI):
        yield
        app.state.registry.close_all()

    app = FastAPI(title="TongGraph Server", lifespan=lifespan)
    app.state.config = server_config
    app.state.registry = GraphRegistry(server_config)

    @app.middleware("http")
    async def request_id_middleware(request: Request, call_next):  # type: ignore[no-untyped-def]
        request.state.request_id = request.headers.get("x-request-id", str(uuid.uuid4()))
        response = await call_next(request)
        response.headers["x-request-id"] = request.state.request_id
        return response

    app.add_exception_handler(ServerError, server_error_handler)
    app.add_exception_handler(ValueError, value_error_handler)
    app.add_exception_handler(RequestValidationError, validation_error_handler)
    app.add_exception_handler(KeyError, key_error_handler)

    app.include_router(health.router)
    app.include_router(admin.router)
    app.include_router(graphs.router)
    app.include_router(records.router)
    app.include_router(retrieval.router)
    app.include_router(query.router)
    app.include_router(compute.router)
    app.include_router(snapshots.router)

    return app
