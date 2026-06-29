from __future__ import annotations

import asyncio
import logging
from pathlib import Path

from fastapi.testclient import TestClient

from tonggraph.server.app import create_app
from tonggraph.server.config import parse_config


def make_config(tmp_path: Path, *, auth_mode: str = "token", timeout: float | None = None, request_logging: bool = True):
    auth = {"mode": "none", "users": {}}
    if auth_mode == "token":
        auth = {
            "mode": "token",
            "users": {
                "admin": {"admin": True, "token": "admin-token", "graphs": {"*": "write"}},
                "alice": {"token": "alice-token", "graphs": {"shared": "write"}},
            },
        }
    return parse_config(
        {
            "host": "127.0.0.1",
            "port": 8719,
            "data_dir": str(tmp_path),
            "graphs": {"shared": "shared.db"},
            "auth": auth,
            "operations": {
                "request_logging": request_logging,
                "request_timeout_seconds": timeout,
                "metrics": True,
            },
        }
    )


def auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def test_request_headers_logging_and_metrics_auth(tmp_path: Path, caplog) -> None:  # type: ignore[no-untyped-def]
    app = create_app(make_config(tmp_path))
    client = TestClient(app)

    caplog.set_level(logging.INFO, logger="tonggraph.server")
    response = client.get("/graphs/shared/nodes/count", headers={**auth("alice-token"), "x-request-id": "req-fixed"})

    assert response.status_code == 200
    assert response.headers["x-request-id"] == "req-fixed"
    assert float(response.headers["x-tonggraph-elapsed-ms"]) >= 0.0
    assert "method=GET" in caplog.text
    assert "path=/graphs/shared/nodes/count" in caplog.text
    assert "status=200" in caplog.text
    assert "request_id=req-fixed" in caplog.text
    assert "user=alice" in caplog.text
    assert "graph=shared" in caplog.text

    denied = client.get("/metrics", headers=auth("alice-token"))
    assert denied.status_code == 403
    assert denied.json()["error"]["code"] == "admin_required"

    metrics = client.get("/metrics", headers=auth("admin-token"))
    assert metrics.status_code == 200
    payload = metrics.json()
    assert payload["requests"]["total_requests"] >= 2
    assert payload["requests"]["error_requests"] >= 1
    assert payload["requests"]["status_counts"]["200"] >= 1
    assert payload["requests"]["status_counts"]["403"] >= 1
    assert payload["requests"]["route_counts"]["GET /graphs/{graph}/nodes/count"] >= 1
    assert payload["graphs"]["configured_graphs"] == 1
    assert payload["graphs"]["open_graphs"] == 1
    assert payload["graphs"]["graphs"][0]["node_count"] == 0
    assert payload["graphs"]["graphs"][0]["edge_count"] == 0


def test_metrics_allowed_without_auth_when_auth_none(tmp_path: Path) -> None:
    client = TestClient(create_app(make_config(tmp_path, auth_mode="none", request_logging=False)))

    response = client.get("/metrics")

    assert response.status_code == 200
    assert response.json()["requests"]["total_requests"] == 0


def test_request_timeout_returns_stable_error(tmp_path: Path) -> None:
    app = create_app(make_config(tmp_path, timeout=0.01, request_logging=False))

    @app.get("/slow")
    async def slow() -> dict[str, bool]:
        await asyncio.sleep(0.05)
        return {"ok": True}

    client = TestClient(app)
    response = client.get("/slow", headers={**auth("admin-token"), "x-request-id": "req-timeout"})

    assert response.status_code == 504
    assert response.headers["x-request-id"] == "req-timeout"
    assert response.json()["error"] == {
        "code": "timeout",
        "message": "request timed out",
        "request_id": "req-timeout",
    }
    metrics = client.get("/metrics", headers=auth("admin-token")).json()["requests"]
    assert metrics["status_counts"]["504"] == 1


def test_lifespan_shutdown_closes_open_workers(tmp_path: Path) -> None:
    app = create_app(make_config(tmp_path, request_logging=False))
    with TestClient(app) as client:
        assert client.post("/graphs/shared/open", headers=auth("alice-token")).status_code == 200
        assert app.state.registry.list_graphs()[0]["open"] is True

    assert app.state.registry.list_graphs()[0]["open"] is False
