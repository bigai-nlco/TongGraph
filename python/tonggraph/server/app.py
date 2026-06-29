"""ASGI app factory for TongGraph server."""

from __future__ import annotations

import asyncio
import logging
import time
import uuid

from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from fastapi.exceptions import RequestValidationError

from .config import ServerConfig, load_config
from .errors import ServerError, error_payload, key_error_handler, server_error_handler, validation_error_handler, value_error_handler
from .metrics import RequestMetrics
from .registry import GraphRegistry
from .routes import admin, compute, graphs, health, operations, query, records, retrieval, snapshots

logger = logging.getLogger("tonggraph.server")


def create_app(config: ServerConfig | str | None = None) -> FastAPI:
    server_config = load_config(config) if not isinstance(config, ServerConfig) else config
    @asynccontextmanager
    async def lifespan(app: FastAPI):
        yield
        app.state.registry.close_all()

    app = FastAPI(title="TongGraph Server", lifespan=lifespan)
    app.state.config = server_config
    app.state.registry = GraphRegistry(server_config)
    app.state.metrics = RequestMetrics()

    @app.middleware("http")
    async def operations_middleware(request: Request, call_next):  # type: ignore[no-untyped-def]
        request.state.request_id = request.headers.get("x-request-id", str(uuid.uuid4()))
        started = time.perf_counter()
        route_for_metrics = request.url.path
        status_code = 500
        response = None
        if server_config.operations.metrics:
            app.state.metrics.begin()
        try:
            if server_config.operations.request_timeout_seconds is None:
                response = await call_next(request)
            else:
                response = await asyncio.wait_for(
                    call_next(request),
                    timeout=server_config.operations.request_timeout_seconds,
                )
            status_code = response.status_code
            route = request.scope.get("route")
            if route is not None and getattr(route, "path", None):
                route_for_metrics = route.path
            return response
        except asyncio.TimeoutError:
            error = ServerError("timeout", "request timed out", status_code=504, graph=_graph_from_path(request.url.path))
            status_code = 504
            response = JSONResponse(
                status_code=504,
                content=error_payload(error, request.state.request_id),
            )
            return response
        except BaseException:
            status_code = 500
            raise
        finally:
            elapsed_ms = (time.perf_counter() - started) * 1000.0
            if response is not None:
                response.headers["x-request-id"] = request.state.request_id
                response.headers["x-tonggraph-elapsed-ms"] = f"{elapsed_ms:.3f}"
            if server_config.operations.metrics:
                app.state.metrics.record(request.method, route_for_metrics, status_code, elapsed_ms)
            if server_config.operations.request_logging:
                user = getattr(request.state, "user", None)
                logger.info(
                    "request method=%s path=%s status=%s duration_ms=%.3f request_id=%s user=%s graph=%s",
                    request.method,
                    request.url.path,
                    status_code,
                    elapsed_ms,
                    request.state.request_id,
                    getattr(user, "user_id", None),
                    _graph_from_path(request.url.path),
                )

    app.add_exception_handler(ServerError, server_error_handler)
    app.add_exception_handler(ValueError, value_error_handler)
    app.add_exception_handler(RequestValidationError, validation_error_handler)
    app.add_exception_handler(KeyError, key_error_handler)

    app.include_router(health.router)
    app.include_router(operations.router)
    app.include_router(admin.router)
    app.include_router(graphs.router)
    app.include_router(records.router)
    app.include_router(retrieval.router)
    app.include_router(query.router)
    app.include_router(compute.router)
    app.include_router(snapshots.router)

    return app


def _graph_from_path(path: str) -> str | None:
    parts = [part for part in path.split("/") if part]
    if len(parts) >= 2 and parts[0] == "graphs":
        return parts[1]
    if len(parts) >= 3 and parts[0] == "admin" and parts[1] == "graphs":
        return parts[2]
    return None
