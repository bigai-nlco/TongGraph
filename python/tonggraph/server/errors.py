"""Server error model and FastAPI exception handlers."""

from __future__ import annotations

from typing import Any

from fastapi import Request
from fastapi.responses import JSONResponse


class ServerError(Exception):
    def __init__(self, code: str, message: str, *, status_code: int = 400, graph: str | None = None) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.status_code = status_code
        self.graph = graph


def error_payload(error: ServerError, request_id: str | None = None) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "code": error.code,
        "message": error.message,
    }
    if error.graph is not None:
        payload["graph"] = error.graph
    if request_id is not None:
        payload["request_id"] = request_id
    return {"error": payload}


async def server_error_handler(request: Request, exc: ServerError) -> JSONResponse:
    return JSONResponse(
        status_code=exc.status_code,
        content=error_payload(exc, getattr(request.state, "request_id", None)),
    )


async def value_error_handler(request: Request, exc: ValueError) -> JSONResponse:
    error = ServerError("invalid_request", str(exc), status_code=400)
    return JSONResponse(
        status_code=400,
        content=error_payload(error, getattr(request.state, "request_id", None)),
    )


async def key_error_handler(request: Request, exc: KeyError) -> JSONResponse:
    message = str(exc).strip("'")
    error = ServerError("not_found", message, status_code=404)
    return JSONResponse(
        status_code=404,
        content=error_payload(error, getattr(request.state, "request_id", None)),
    )


async def validation_error_handler(request: Request, exc: Exception) -> JSONResponse:
    error = ServerError("invalid_request", str(exc), status_code=422)
    return JSONResponse(
        status_code=422,
        content=error_payload(error, getattr(request.state, "request_id", None)),
    )
