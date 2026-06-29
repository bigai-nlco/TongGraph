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
from tonggraph.server.client import TongGraphClient
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


def test_sdk_gap_http_endpoints_and_import_export(tmp_path: Path) -> None:
    client = TestClient(create_app(make_config(tmp_path)))
    alice = auth("alice-token")
    bob = auth("bob-token")

    denied = client.post("/graphs/shared/nodes/batch", headers=bob, json={"records": [{"external_id": "blocked"}]})
    assert_error(denied, "permission_denied")

    created = client.post(
        "/graphs/shared/nodes/batch",
        headers=alice,
        json={
            "records": [
                {"external_id": "alice", "labels": ["Person"], "properties": {"name": "Alice", "text": "graph memory"}},
                {"external_id": "bob", "labels": ["Person"], "properties": {"name": "Bob", "text": "agent memory"}},
            ]
        },
    ).json()
    assert created["ids"] == [0, 1]
    assert [node["external_id"] for node in created["nodes"]] == ["alice", "bob"]

    duplicate = client.post(
        "/graphs/shared/nodes/batch",
        headers=alice,
        json={"records": [{"external_id": "dup"}, {"external_id": "dup"}]},
    )
    assert_error(duplicate, "invalid_request")
    assert client.get("/graphs/shared/nodes/count", headers=alice).json()["count"] == 2

    edges = client.post(
        "/graphs/shared/edges/batch",
        headers=alice,
        json={"records": [{"source": 0, "target": 1, "edge_type": "KNOWS", "properties": {"weight": 0.7}}]},
    ).json()
    assert edges["ids"] == [0]
    assert edges["edges"][0]["edge_type"] == "KNOWS"

    bad_edges = client.post(
        "/graphs/shared/edges/batch",
        headers=alice,
        json={
            "records": [
                {"source": 0, "target": 1, "edge_type": "OK"},
                {"source": 999, "target": 1, "edge_type": "BAD"},
            ]
        },
    )
    assert_error(bad_edges, "invalid_request")
    assert client.get("/graphs/shared/edges/count", headers=alice).json()["count"] == 1

    assert client.get("/graphs/shared/nodes/by-external-id/alice", headers=bob).json()["id"] == 0
    assert client.get("/graphs/shared/nodes/by-external-id/missing", headers=bob).json()["id"] is None
    assert client.get("/graphs/shared/query/schema", headers=bob).json()["schema"]["name"] == "tonggraph_query_dsl_v0"

    client.post("/graphs/shared/fulltext/indexes", headers=alice, json={"name": "people", "properties": ["text"], "target": "node"})
    client.post("/graphs/shared/vector/indexes", headers=alice, json={"name": "people", "dimensions": 2, "target": "node"})
    assert client.put(
        "/graphs/shared/vector/people/batch",
        headers=alice,
        json={"vectors": {"0": [1.0, 0.0], "1": [0.8, 0.2]}},
    ).json() == {"upserted": True, "count": 2}

    context = client.post(
        "/graphs/shared/retrieve/context",
        headers=bob,
        json={
            "text_index": "people",
            "text_query": "graph memory",
            "vector_index": "people",
            "vector_query": [1.0, 0.0],
            "labels": ["Person"],
            "radius": 1,
            "limit": 2,
        },
    ).json()["results"]
    assert context[0]["kind"] == "node"
    assert context[0]["record"]["external_id"] == "alice"
    assert "text" in context[0]["source_scores"]

    snapshot = client.post("/graphs/shared/snapshots", headers=bob, json={"ttl_seconds": 60}).json()["snapshot"]
    snapshot_id = snapshot["snapshot_id"]
    later = client.post(
        "/graphs/shared/nodes",
        headers=alice,
        json={"external_id": "later", "labels": ["Person"], "properties": {"text": "later only"}},
    ).json()["id"]
    client.put(f"/graphs/shared/vector/people/{later}", headers=alice, json={"vector": [0.0, 1.0]})

    assert client.get(f"/graphs/shared/snapshots/{snapshot_id}/fulltext/indexes", headers=bob).json()["indexes"][0]["name"] == "people"
    assert client.post(
        f"/graphs/shared/snapshots/{snapshot_id}/fulltext/people/search",
        headers=bob,
        json={"query": "later", "limit": 5},
    ).json()["results"] == []
    assert client.get(f"/graphs/shared/snapshots/{snapshot_id}/vector/indexes", headers=bob).json()["indexes"][0]["name"] == "people"
    assert client.get(f"/graphs/shared/snapshots/{snapshot_id}/vector/people/0", headers=bob).json()["vector"] == [1.0, 0.0]
    assert client.post(
        f"/graphs/shared/snapshots/{snapshot_id}/vector/people/search",
        headers=bob,
        json={"query_vector": [0.0, 1.0], "limit": 3},
    ).json()["results"][-1]["id"] != later

    assert client.post(
        "/graphs/shared/vector/people/delete-batch",
        headers=alice,
        json={"entity_ids": [1]},
    ).json() == {"deleted": True, "count": 1}
    assert [row["id"] for row in client.post(
        "/graphs/shared/vector/people/search",
        headers=bob,
        json={"query_vector": [0.8, 0.2], "limit": 5},
    ).json()["results"]] == [0, later]

    imports = tmp_path / "imports"
    imports.mkdir()
    (imports / "nodes.jsonl").write_text(
        json.dumps({"external_id": "doc", "labels": ["Document"], "properties": {"title": "Doc"}}) + "\n",
        encoding="utf-8",
    )
    (imports / "edges.csv").write_text("source,target,edge_type,weight\ndoc,alice,MENTIONS,1.0\n", encoding="utf-8")
    assert client.post("/graphs/shared/import/nodes/jsonl", headers=alice, json={"path": "nodes.jsonl"}).json()["ids"] == [3]
    assert client.post("/graphs/shared/import/edges/csv", headers=alice, json={"path": "edges.csv"}).json()["ids"] == [1]
    assert_error(client.post("/graphs/shared/import/nodes/jsonl", headers=alice, json={"path": "../escape.jsonl"}), "invalid_request")
    assert_error(client.post("/graphs/shared/import/nodes/jsonl", headers=bob, json={"path": "nodes.jsonl"}), "permission_denied")

    assert client.post("/graphs/shared/export/nodes/jsonl", headers=bob, json={"path": "nodes/out.jsonl"}).json()["exported"] is True
    assert json.loads((tmp_path / "exports" / "nodes" / "out.jsonl").read_text(encoding="utf-8").splitlines()[0])["external_id"] == "alice"
    assert client.post(
        "/graphs/shared/export/query-rows/jsonl",
        headers=bob,
        json={"path": "rows/out.jsonl", "rows": [{"name": "Alice"}]},
    ).json()["exported"] is True
    assert json.loads((tmp_path / "exports" / "rows" / "out.jsonl").read_text(encoding="utf-8")) == {"name": "Alice"}
    assert_error(client.post("/graphs/shared/export/nodes/jsonl", headers=bob, json={"path": "../escape.jsonl"}), "invalid_request")


def test_python_client_sdk_gap_wrappers(tmp_path: Path) -> None:
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
        graph = TongGraphClient(f"http://127.0.0.1:{port}", token="alice-token").graph("shared")
        ids = graph.add_nodes(
            [
                {"external_id": "alice", "labels": ["Person"], "properties": {"text": "graph memory"}},
                {"external_id": "bob", "labels": ["Person"], "properties": {"text": "agent memory"}},
            ]
        )
        assert ids == [0, 1]
        assert graph.add_edges([{"source": 0, "target": 1, "edge_type": "KNOWS"}]) == [0]
        assert graph.get_node_id("alice") == 0
        assert graph.query_schema()["name"] == "tonggraph_query_dsl_v0"

        graph.create_fulltext_index("people", ["text"])
        graph.create_vector_index("people", 2)
        assert graph.upsert_vectors("people", {0: [1.0, 0.0], 1: [0.5, 0.5]}) is True
        assert graph.retrieve_context(text_index="people", text_query="graph", vector_index="people", vector_query=[1.0, 0.0])[0]["id"] == 0
        snapshot = graph.create_snapshot()
        assert snapshot.fulltext_indexes()[0]["name"] == "people"
        assert snapshot.search_text("people", "graph")[0]["id"] == 0
        assert snapshot.vector_indexes()[0]["name"] == "people"
        assert snapshot.get_vector("people", 0) == [1.0, 0.0]
        assert snapshot.search_vector("people", [1.0, 0.0], limit=1)[0]["id"] == 0
        assert graph.delete_vectors("people", [1]) is True

        imports = tmp_path / "imports"
        imports.mkdir(exist_ok=True)
        (imports / "nodes.csv").write_text(
            'external_id,labels,properties\ndoc,Document,"{""title"": ""Doc""}"\n',
            encoding="utf-8",
        )
        assert graph.import_nodes_csv("nodes.csv") == [2]
        assert graph.export_nodes_jsonl("client/nodes.jsonl")["exported"] is True
        assert (tmp_path / "exports" / "client" / "nodes.jsonl").exists()
    finally:
        server.should_exit = True
        thread.join(timeout=5)


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])
