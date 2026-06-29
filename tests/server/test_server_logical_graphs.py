from __future__ import annotations

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


def auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def assert_error(response, code: str) -> None:  # type: ignore[no-untyped-def]
    assert response.status_code >= 400
    payload = response.json()
    assert payload["error"]["code"] == code
    assert "request_id" in payload["error"]


def logical_config(tmp_path: Path):  # type: ignore[no-untyped-def]
    return parse_config(
        {
            "data_dir": str(tmp_path),
            "graphs": {"shared": {"path": "shared.db", "logical_graphs": True}},
            "auth": {
                "mode": "token",
                "users": {
                    "admin": {"admin": True, "token": "admin-token", "graphs": {"*": "write"}},
                    "alice": {"token": "alice-token", "graphs": {"shared": "write"}},
                    "bob": {"token": "bob-token", "graphs": {"shared": "read"}},
                },
            },
        }
    )


def test_logical_graph_config_admin_create_and_persistence(tmp_path: Path) -> None:
    client = TestClient(create_app(logical_config(tmp_path)))
    admin = auth("admin-token")
    alice = auth("alice-token")
    bob = auth("bob-token")

    assert client.get("/admin/graphs", headers=admin).json()["graphs"][0]["logical_graphs"] is True
    created = client.post(
        "/admin/graphs",
        headers=admin,
        json={"name": "workspace", "logical_graphs": True, "grants": {"alice": "write", "bob": "read"}},
    )
    assert created.status_code == 200
    assert created.json()["graph"]["logical_graphs"] is True

    denied = client.post("/graphs/shared/logical-graphs", headers=bob, json={"logical_graph_id": "q1"})
    assert_error(denied, "permission_denied")

    logical = client.post("/graphs/shared/logical-graphs", headers=alice, json={"logical_graph_id": "q1"})
    assert logical.status_code == 200
    assert logical.json()["logical_graph"]["logical_graph_id"] == "q1"

    reloaded = TestClient(create_app(logical_config(tmp_path)))
    graphs = reloaded.get("/admin/graphs", headers=admin).json()["graphs"]
    assert {item["name"]: item["logical_graphs"] for item in graphs} == {"shared": True, "workspace": True}
    assert reloaded.get("/graphs/shared/logical-graphs", headers=bob).json()["logical_graphs"][0]["logical_graph_id"] == "q1"


def test_logical_graph_scope_isolation_retrieval_query_compute_and_snapshot(tmp_path: Path) -> None:
    client = TestClient(create_app(logical_config(tmp_path)))
    headers = auth("alice-token")

    missing = client.get("/graphs/shared/nodes/count", headers=headers)
    assert_error(missing, "logical_graph_required")

    q1_a = client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "agent:q1:a", "labels": ["Belief"], "properties": {"text": "alpha worship"}, "logical_graph_id": "q1"},
    ).json()["id"]
    q1_b = client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "agent:q1:b", "labels": ["Belief"], "properties": {"text": "beta"}, "logical_graph_id": "q1"},
    ).json()["id"]
    q2_a = client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "agent:q2:a", "labels": ["Belief"], "properties": {"text": "gamma worship"}, "logical_graph_id": "q2"},
    ).json()["id"]
    q1_edge = client.post(
        "/graphs/shared/edges",
        headers=headers,
        json={"source": q1_a, "target": q1_b, "edge_type": "LINK", "properties": {"weight": 1.0}, "logical_graph_id": "q1"},
    ).json()["id"]

    cross = client.post(
        "/graphs/shared/edges",
        headers=headers,
        json={"source": q1_a, "target": q2_a, "edge_type": "LINK", "logical_graph_id": "q1"},
    )
    assert_error(cross, "not_found")

    assert client.get("/graphs/shared/nodes/count", headers=headers, params={"logical_graph_id": "q1"}).json()["count"] == 2
    assert client.get("/graphs/shared/nodes/count", headers=headers, params={"logical_graph_id": "q2"}).json()["count"] == 1
    assert client.get("/graphs/shared/nodes/by-label/Belief", headers=headers, params={"logical_graph_id": "q1"}).json()["ids"] == [q1_a, q1_b]
    assert client.get(f"/graphs/shared/nodes/{q2_a}", headers=headers, params={"logical_graph_id": "q1"}).status_code == 404

    client.post("/graphs/shared/fulltext/indexes", headers=headers, json={"name": "text", "properties": ["text"], "target": "node"})
    text_rows = client.post(
        "/graphs/shared/fulltext/text/search",
        headers=headers,
        json={"query": "worship", "labels": ["Belief"], "logical_graph_id": "q1"},
    ).json()["results"]
    assert [row["id"] for row in text_rows] == [q1_a]

    client.post("/graphs/shared/vector/indexes", headers=headers, json={"name": "emb", "dimensions": 2, "target": "node", "metric": "cosine"})
    client.put(f"/graphs/shared/vector/emb/{q1_a}", headers=headers, json={"vector": [1.0, 0.0], "logical_graph_id": "q1"})
    client.put(f"/graphs/shared/vector/emb/{q2_a}", headers=headers, json={"vector": [0.9, 0.1], "logical_graph_id": "q2"})
    vector_rows = client.post(
        "/graphs/shared/vector/emb/search",
        headers=headers,
        json={"query_vector": [1.0, 0.0], "logical_graph_id": "q1"},
    ).json()["results"]
    assert [row["id"] for row in vector_rows] == [q1_a]

    context = client.post(
        "/graphs/shared/retrieve/context",
        headers=headers,
        json={"text_query": "worship", "text_index": "text", "logical_graph_id": "q1"},
    ).json()["results"]
    assert {row["record"]["id"] for row in context} <= {q1_a, q1_b, q1_edge}

    query_rows = client.post(
        "/graphs/shared/query",
        headers=headers,
        json={"logical_graph_id": "q1", "spec": {"match": [{"node": "n", "labels": ["Belief"]}], "return": ["n"]}},
    ).json()["result"]
    assert query_rows == [{"n": q1_a}, {"n": q1_b}]

    assert client.get("/graphs/shared/traversal/neighbors/" + str(q1_a), headers=headers, params={"logical_graph_id": "q1"}).json()["ids"] == [q1_b]
    assert client.get("/graphs/shared/algorithms/bfs", headers=headers, params={"start": q1_a, "logical_graph_id": "q1"}).json()["ids"] == [q1_a, q1_b]

    snapshot_id = client.post("/graphs/shared/snapshots", headers=headers, json={"logical_graph_id": "q1"}).json()["snapshot"]["snapshot_id"]
    client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "agent:q1:c", "labels": ["Belief"], "logical_graph_id": "q1"},
    )
    assert client.get(f"/graphs/shared/snapshots/{snapshot_id}/nodes/count", headers=headers).json()["count"] == 2

    cypher = client.post("/graphs/shared/cypher", headers=headers, json={"query": "MATCH (n) RETURN n"})
    assert_error(cypher, "unsupported_operation")

    deleted = client.delete("/graphs/shared/logical-graphs/q1", headers=headers).json()
    assert deleted["deleted"] is True
    assert client.get("/graphs/shared/nodes/count", headers=headers, params={"logical_graph_id": "q2"}).json()["count"] == 1


@pytest.fixture()
def logical_server_url(tmp_path: Path) -> str:  # type: ignore[no-untyped-def]
    port = _free_port()
    app = create_app(logical_config(tmp_path))
    config = uvicorn.Config(app, host="127.0.0.1", port=port, log_level="critical", lifespan="on")
    server = uvicorn.Server(config)
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
        yield f"http://127.0.0.1:{port}"
    finally:
        server.should_exit = True
        thread.join(timeout=5)


def test_python_client_logical_graph_proxy(logical_server_url: str) -> None:
    graph = TongGraphClient(logical_server_url, token="alice-token").graph("shared")
    q1 = graph.logical("q1")
    q2 = graph.logical("q2")

    q1.create()
    a = q1.add_node("agent:q1:a", labels=["Belief"], properties={"text": "alpha"})
    b = q1.add_node("agent:q1:b", labels=["Belief"], properties={"text": "beta"})
    q1.add_edge(a, b, "LINK")
    q2.add_node("agent:q2:a", labels=["Belief"], properties={"text": "alpha"})

    assert q1.node_count() == 2
    assert q2.node_count() == 1
    assert q1.query({"match": [{"node": "n", "labels": ["Belief"]}], "return": ["n"]}) == [{"n": a}, {"n": b}]
    assert q1.neighbors(a) == [b]

    with pytest.raises(TongGraphServerError) as missing_scope:
        graph.node_count()
    assert missing_scope.value.code == "logical_graph_required"


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])
