from __future__ import annotations

import json
import socket
import threading
import time
from pathlib import Path

import pytest
import uvicorn
from fastapi.testclient import TestClient

from tonggraph.server.app import create_app
from tonggraph.server.client import TongGraphClient, TongGraphServerError
from tonggraph.server.config import parse_config


def make_config(tmp_path: Path):  # type: ignore[no-untyped-def]
    return parse_config(
        {
            "host": "127.0.0.1",
            "port": 8719,
            "data_dir": str(tmp_path),
            "graphs": {"shared": "shared.db"},
            "auth": {
                "mode": "token",
                "users": {
                    "admin": {"admin": True, "token": "admin-token", "graphs": {"*": "write"}},
                    "alice": {"token": "alice-token", "graphs": {"shared": "write"}},
                    "bob": {"token": "bob-token", "graphs": {"shared": "read"}},
                },
            },
            "operations": {"request_logging": False},
        }
    )


def auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def assert_error(response, code: str) -> None:  # type: ignore[no-untyped-def]
    assert response.status_code >= 400
    payload = response.json()["error"]
    assert payload["code"] == code
    assert "request_id" in payload


def test_admin_user_management_token_rotation_and_acl(tmp_path: Path) -> None:
    client = TestClient(create_app(make_config(tmp_path)))

    assert client.get("/graphs", headers=auth("alice-token")).status_code == 200
    users = client.get("/admin/users", headers=auth("admin-token")).json()["users"]
    alice = next(user for user in users if user["user_id"] == "alice")
    assert alice["source"] == "config"
    assert alice["has_token"] is True
    assert "token" not in alice

    created = client.post(
        "/admin/users",
        headers=auth("admin-token"),
        json={"user_id": "dave", "token": "dave-token", "graphs": {"shared": "read"}},
    ).json()["user"]
    assert created["user_id"] == "dave"
    assert created["source"] == "dynamic"
    assert created["graphs"] == {"shared": "read"}
    assert "token" not in created

    assert client.get("/graphs", headers=auth("dave-token")).json()["graphs"][0]["name"] == "shared"
    denied_write = client.post("/graphs/shared/nodes", headers=auth("dave-token"), json={"external_id": "blocked"})
    assert_error(denied_write, "permission_denied")

    updated = client.patch(
        "/admin/users/dave",
        headers=auth("admin-token"),
        json={"graphs": {"shared": "write"}},
    ).json()["user"]
    assert updated["graphs"] == {"shared": "write"}
    assert client.post("/graphs/shared/nodes", headers=auth("dave-token"), json={"external_id": "ok"}).status_code == 200

    rotated = client.post(
        "/admin/users/dave/token",
        headers=auth("admin-token"),
        json={"token": "dave-token-2"},
    ).json()
    assert rotated["token"] == "dave-token-2"
    assert rotated["user"]["has_token"] is True
    assert "token" not in rotated["user"]
    assert_error(client.get("/graphs", headers=auth("dave-token")), "unauthenticated")
    assert client.get("/graphs", headers=auth("dave-token-2")).status_code == 200

    generated = client.post("/admin/users/dave/token", headers=auth("admin-token"), json={}).json()["token"]
    assert isinstance(generated, str) and len(generated) >= 32
    assert_error(client.get("/graphs", headers=auth("dave-token-2")), "unauthenticated")
    assert client.get("/graphs", headers=auth(generated)).status_code == 200
    assert "token" not in client.get("/admin/users/dave", headers=auth("admin-token")).json()["user"]

    client.patch("/admin/users/dave", headers=auth("admin-token"), json={"disabled": True})
    assert_error(client.get("/graphs", headers=auth(generated)), "unauthenticated")
    client.patch("/admin/users/dave", headers=auth("admin-token"), json={"disabled": False})
    assert client.get("/graphs", headers=auth(generated)).status_code == 200

    assert_error(client.get("/admin/users", headers=auth("bob-token")), "admin_required")
    assert_error(client.delete("/admin/users/alice", headers=auth("admin-token")), "conflict")
    assert client.delete("/admin/users/dave", headers=auth("admin-token")).json() == {"user": "dave", "deleted": True}
    assert_error(client.get("/graphs", headers=auth(generated)), "unauthenticated")


def test_dynamic_users_persist_and_config_users_can_be_overridden(tmp_path: Path) -> None:
    client = TestClient(create_app(make_config(tmp_path)))
    created = client.post(
        "/admin/users",
        headers=auth("admin-token"),
        json={"user_id": "erin", "token": "erin-token", "graphs": {"shared": "read"}},
    )
    assert created.status_code == 200
    client.patch("/admin/users/alice", headers=auth("admin-token"), json={"disabled": True})

    state = json.loads((tmp_path / "server-state.json").read_text(encoding="utf-8"))
    assert state["users"]["erin"]["token"] == "erin-token"
    assert state["users"]["alice"]["disabled"] is True
    assert state["grants"]["erin"] == {"shared": "read"}

    restarted = TestClient(create_app(make_config(tmp_path)))
    assert restarted.get("/graphs", headers=auth("erin-token")).status_code == 200
    assert_error(restarted.get("/graphs", headers=auth("alice-token")), "unauthenticated")
    restarted.patch("/admin/users/alice", headers=auth("admin-token"), json={"disabled": False})
    assert restarted.get("/graphs", headers=auth("alice-token")).status_code == 200


def test_python_client_auth_management_wrappers(tmp_path: Path) -> None:
    port = _free_port()
    app = create_app(make_config(tmp_path))
    server = uvicorn.Server(uvicorn.Config(app, host="127.0.0.1", port=port, log_level="critical", lifespan="on"))
    thread = threading.Thread(target=server.run, daemon=True)
    thread.start()
    deadline = time.time() + 10
    while not server.started:
        if not thread.is_alive() or time.time() > deadline:
            server.should_exit = True
            thread.join(timeout=5)
            raise RuntimeError("uvicorn test server did not start")
        time.sleep(0.01)
    try:
        admin = TongGraphClient(f"http://127.0.0.1:{port}", token="admin-token")
        created = admin.create_user("frank", token="frank-token", graphs={"shared": "read"})
        assert created["user_id"] == "frank"
        assert admin.admin_user("frank")["has_token"] is True
        assert any(user["user_id"] == "frank" for user in admin.admin_users())

        frank = TongGraphClient(f"http://127.0.0.1:{port}", token="frank-token")
        assert frank.graphs()[0]["name"] == "shared"
        with pytest.raises(TongGraphServerError) as denied:
            frank.graph("shared").add_node("blocked")
        assert denied.value.code == "permission_denied"

        admin.update_user("frank", graphs={"shared": "write"})
        assert frank.graph("shared").add_node("allowed") == 0
        rotated = admin.rotate_user_token("frank")
        assert rotated["token"]
        with pytest.raises(TongGraphServerError) as old_token:
            frank.graphs()
        assert old_token.value.code == "unauthenticated"
        assert TongGraphClient(f"http://127.0.0.1:{port}", token=rotated["token"]).graphs()[0]["name"] == "shared"
        assert admin.delete_user("frank") == {"user": "frank", "deleted": True}
    finally:
        server.should_exit = True
        thread.join(timeout=5)


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])
