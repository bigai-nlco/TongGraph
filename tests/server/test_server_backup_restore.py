from __future__ import annotations

import socket
import tarfile
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
                    "bob": {"token": "bob-token", "graphs": {}},
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


def seed_graph(client: TestClient) -> tuple[int, int]:
    headers = auth("alice-token")
    alice = client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "alice", "labels": ["Person"], "properties": {"name": "Alice"}},
    ).json()["id"]
    bob = client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "bob", "labels": ["Person"], "properties": {"name": "Bob"}},
    ).json()["id"]
    client.post(
        "/graphs/shared/edges",
        headers=headers,
        json={"source": alice, "target": bob, "edge_type": "KNOWS", "properties": {"weight": 1.0}},
    )
    client.post("/graphs/shared/fulltext/indexes", headers=headers, json={"name": "people", "properties": ["name"], "target": "node"})
    client.post("/graphs/shared/vector/indexes", headers=headers, json={"name": "embeddings", "dimensions": 2, "target": "node"})
    client.put(f"/graphs/shared/vector/embeddings/{alice}", headers=headers, json={"vector": [1.0, 0.0]})
    client.put(f"/graphs/shared/vector/embeddings/{bob}", headers=headers, json={"vector": [0.5, 0.5]})
    return alice, bob


def test_admin_backup_restore_archive_and_grants(tmp_path: Path) -> None:
    client = TestClient(create_app(make_config(tmp_path)))
    alice, bob = seed_graph(client)
    snapshot_id = client.post("/graphs/shared/snapshots", headers=auth("alice-token"), json={"ttl_seconds": 60}).json()["snapshot"]["snapshot_id"]
    assert snapshot_id

    denied = client.post("/admin/graphs/shared/backup", headers=auth("alice-token"), json={"note": "blocked"})
    assert_error(denied, "admin_required")

    backup = client.post("/admin/graphs/shared/backup", headers=auth("admin-token"), json={"note": "daily"}).json()["backup"]
    assert backup["graph"] == "shared"
    assert backup["node_count"] == 2
    assert backup["edge_count"] == 1
    assert backup["size_bytes"] > 0
    assert backup["filename"].endswith(".tar.gz")
    backup_path = tmp_path / "backups" / backup["filename"]
    assert backup_path.exists()

    with tarfile.open(backup_path, "r:gz") as archive:
        names = set(archive.getnames())
        assert "metadata.json" in names
        assert "graph.db" in names
        assert "graph.db.segments/manifest.txt" in names
        assert "graph.db.segments/segment-v1.bin" in names

    backups = client.get("/admin/backups", headers=auth("admin-token")).json()["backups"]
    assert [item["backup_id"] for item in backups] == [backup["backup_id"]]

    restored = client.post(
        f"/admin/backups/{backup['backup_id']}/restore",
        headers=auth("admin-token"),
        json={"graph": "restored", "grants": {"bob": "read"}},
    ).json()["graph"]
    assert restored["graph"] == "restored"
    assert restored["source_graph"] == "shared"
    assert restored["node_count"] == 2
    assert restored["edge_count"] == 1

    assert client.get("/graphs/restored/nodes/count", headers=auth("bob-token")).json()["count"] == 2
    assert client.post(
        "/graphs/restored/fulltext/people/search",
        headers=auth("bob-token"),
        json={"query": "Alice", "limit": 1},
    ).json()["results"][0]["id"] == alice
    assert client.post(
        "/graphs/restored/vector/embeddings/search",
        headers=auth("bob-token"),
        json={"query_vector": [1.0, 0.0], "limit": 1},
    ).json()["results"][0]["id"] == alice
    assert client.post(
        "/graphs/restored/query",
        headers=auth("bob-token"),
        json={"spec": {"match": [{"node": "n", "external_id": "bob"}]}},
    ).json()["result"] == [{"n": bob}]
    assert client.get("/graphs/restored/snapshots", headers=auth("bob-token")).json()["snapshots"] == []

    conflict = client.post(
        f"/admin/backups/{backup['backup_id']}/restore",
        headers=auth("admin-token"),
        json={"graph": "restored"},
    )
    assert_error(conflict, "conflict")

    client.post("/admin/graphs", headers=auth("admin-token"), json={"name": "replace_me", "grants": {"alice": "write"}})
    client.post("/graphs/replace_me/nodes", headers=auth("alice-token"), json={"external_id": "old"})
    overwritten = client.post(
        f"/admin/backups/{backup['backup_id']}/restore",
        headers=auth("admin-token"),
        json={"graph": "replace_me", "overwrite": True, "grants": {"bob": "read"}},
    ).json()["graph"]
    assert overwritten["overwritten"] is True
    assert client.get("/graphs/replace_me/nodes/count", headers=auth("bob-token")).json()["count"] == 2
    assert client.post(
        "/graphs/replace_me/query",
        headers=auth("bob-token"),
        json={"spec": {"match": [{"node": "n", "external_id": "old"}]}},
    ).json()["result"] == []

    restarted = TestClient(create_app(make_config(tmp_path)))
    assert restarted.get("/graphs/restored/nodes/count", headers=auth("bob-token")).json()["count"] == 2

    assert_error(client.delete("/admin/backups/bad.id", headers=auth("admin-token")), "invalid_request")
    assert_error(client.post("/admin/backups/missing/restore", headers=auth("admin-token"), json={"graph": "x"}), "backup_not_found")
    assert client.delete(f"/admin/backups/{backup['backup_id']}", headers=auth("admin-token")).json()["deleted"] is True
    assert client.get("/admin/backups", headers=auth("admin-token")).json()["backups"] == []
    assert_error(client.delete(f"/admin/backups/{backup['backup_id']}", headers=auth("admin-token")), "backup_not_found")


def test_python_client_backup_restore_wrappers(tmp_path: Path) -> None:
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
        alice_client = TongGraphClient(f"http://127.0.0.1:{port}", token="alice-token")
        alice_graph = alice_client.graph("shared")
        node = alice_graph.add_node("client-node", labels=["Person"], properties={"name": "Client"})
        alice_graph.add_node("client-target")
        alice_graph.add_edge(node, node + 1, "LINK")

        backup = admin.backup_graph("shared", note="client")
        assert backup["backup_id"] in [item["backup_id"] for item in admin.backups()]
        restored = admin.restore_backup(backup["backup_id"], "client_restored", grants={"bob": "read"})
        assert restored["graph"] == "client_restored"
        assert TongGraphClient(f"http://127.0.0.1:{port}", token="bob-token").graph("client_restored").node_count() == 2
        assert admin.delete_backup(backup["backup_id"])["deleted"] is True
        with pytest.raises(TongGraphServerError) as missing:
            admin.delete_backup(backup["backup_id"])
        assert missing.value.code == "backup_not_found"
    finally:
        server.should_exit = True
        thread.join(timeout=5)


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])
