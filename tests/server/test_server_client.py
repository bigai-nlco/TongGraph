from __future__ import annotations

import socket
import sys
import threading
import time
import urllib.error
from pathlib import Path
from typing import Any

import pytest
import uvicorn

from tonggraph.server.app import create_app
from tonggraph.server.client import TongGraphClient, TongGraphServerError, _error_from_http
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
                    "admin": {
                        "admin": True,
                        "token_env": "TONGGRAPH_TEST_ADMIN_TOKEN",
                        "graphs": {"*": "write"},
                    },
                    "alice": {"token": "alice-token", "graphs": {"shared": "write"}},
                    "bob": {"token": "bob-token", "graphs": {"shared": "read"}},
                    "carol": {"token": "carol-token", "graphs": {}},
                },
            },
        }
    )


@pytest.fixture()
def server_url(tmp_path: Path, monkeypatch) -> str:  # type: ignore[no-untyped-def]
    monkeypatch.setenv("TONGGRAPH_TEST_ADMIN_TOKEN", "admin-token")
    port = _free_port()
    app = create_app(make_config(tmp_path))
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


def test_client_service_admin_auth_and_error_handling(server_url: str) -> None:
    admin = TongGraphClient(server_url, token="admin-token")
    alice = TongGraphClient(server_url, token="alice-token")
    bob = TongGraphClient(server_url, token="bob-token")
    anonymous = TongGraphClient(server_url)

    assert admin.health()["status"] == "ok"
    assert [graph["name"] for graph in alice.graphs()] == ["shared"]

    with pytest.raises(TongGraphServerError) as unauthenticated:
        anonymous.graphs()
    assert unauthenticated.value.code == "unauthenticated"
    assert unauthenticated.value.status_code == 401
    assert unauthenticated.value.request_id

    created = admin.create_graph("project", grants={"alice": "write", "bob": "read"})
    assert created["name"] == "project"
    assert "project" in [graph["name"] for graph in admin.admin_graphs()]
    assert admin.grant_graph("project", "bob", "write") == {"graph": "project", "user": "bob", "access": "write"}
    assert admin.revoke_graph("project", "bob")["revoked"] is True

    with pytest.raises(TongGraphServerError) as denied:
        bob.graph("shared").add_node("blocked")
    assert denied.value.code == "permission_denied"
    assert denied.value.status_code == 403
    assert denied.value.graph == "shared"


def test_client_records_retrieval_query_compute_and_snapshots(server_url: str) -> None:
    graph = TongGraphClient(server_url, token="alice-token").graph("shared")

    alice = graph.add_node("alice", labels=["Person"], properties={"name": "Alice", "active": True})
    bob = graph.add_node("bob", labels=["Person"], properties={"name": "Bob"})
    carol = graph.add_node("carol", labels=["Person"], properties={"name": "Carol"})
    edge = graph.add_edge(alice, bob, "KNOWS", properties={"note": "graph friend", "weight": 2.0})
    graph.add_edge(bob, carol, "KNOWS", properties={"weight": 1.0})

    assert graph.open()["open"] is True
    assert graph.node_count() == 3
    assert graph.edge_count() == 2
    assert graph.node_ids() == [alice, bob, carol]
    assert graph.edge_ids() == [edge, edge + 1]
    assert graph.get_node(alice)["external_id"] == "alice"
    assert graph.update_node(alice, set_properties={"rank": 1})["properties"]["rank"] == 1
    assert graph.get_edge(edge)["edge_type"] == "KNOWS"
    assert graph.update_edge(edge, set_properties={"rank": 2})["properties"]["rank"] == 2
    assert graph.nodes_with_label("Person") == [alice, bob, carol]
    assert graph.nodes_with_property("active", True) == [alice]
    assert graph.edges_by_type("KNOWS") == [edge, edge + 1]
    assert graph.edges_with_property("rank", 2) == [edge]
    assert graph.stats()["nodes"] == 3
    assert "labels" in graph.schema()

    graph.create_fulltext_index("people", ["name"], target="node")
    assert graph.fulltext_indexes()[0]["name"] == "people"
    assert graph.search_text("people", "Alice", labels=["Person"])[0]["id"] == alice
    assert graph.rebuild_fulltext_index("people")["rebuilt"] is True

    graph.create_vector_index("embeddings", 2, target="node", metric="cosine")
    assert graph.vector_indexes()[0]["name"] == "embeddings"
    assert graph.upsert_vector("embeddings", alice, [1.0, 0.0]) is True
    assert graph.upsert_vector("embeddings", bob, [0.5, 0.5]) is True
    assert graph.get_vector("embeddings", alice) == [1.0, 0.0]
    assert [row["id"] for row in graph.search_vector("embeddings", [1.0, 0.0], labels=["Person"])] == [alice, bob]
    batches = graph.search_vectors("embeddings", [[1.0, 0.0], [0.5, 0.5]], labels=["Person"], limit=1)
    assert [[row["id"] for row in batch] for batch in batches] == [[alice], [bob]]

    assert graph.query({"match": [{"node": "n", "external_id": "alice"}]}) == [{"n": alice}]
    assert graph.cypher("MATCH (n:Person) RETURN count(*) AS total")["records"] == [{"total": 3}]
    tx_result = graph.cypher_transaction(
        [{"query": "CREATE (n:Person {external_id: 'dave', name: 'Dave'}) RETURN n"}]
    )
    assert tx_result[0]["records"][0]["n"]["id"] == 3

    assert graph.neighbors(alice) == [bob]
    assert graph.k_hop(alice, 2) == [bob, carol]
    assert graph.frontier([alice], 2) == [carol]
    assert graph.bfs(alice, max_depth=1) == [alice, bob]
    assert graph.shortest_path(alice, carol, weight_property="weight") == {"nodes": [alice, bob, carol], "distance": 3.0}
    assert graph.connected_components() == [[alice, bob, carol], [3]]
    assert set(graph.pagerank(iterations=5)) == {str(alice), str(bob), str(carol), "3"}
    assert graph.random_walk(alice, 2, seed=7)[0] == alice
    assert graph.subgraph([alice, bob])["node_count"] == 2
    assert graph.compute_batch([{"op": "bfs", "start": alice, "max_depth": 1}]) == [[alice, bob]]

    snapshot = graph.create_snapshot(ttl_seconds=60)
    graph.add_node("later")
    assert snapshot.metadata["snapshot_id"] == snapshot.snapshot_id
    assert snapshot.node_count() == 4
    assert snapshot.edge_count() == 2
    assert snapshot.node_ids() == [alice, bob, carol, 3]
    assert snapshot.get_node(alice)["external_id"] == "alice"
    assert snapshot.query({"match": [{"node": "n", "external_id": "later"}]}) == []
    assert snapshot.cypher("MATCH (n:Person) RETURN count(*) AS total")["records"] == [{"total": 4}]
    assert snapshot.compute_batch([{"op": "bfs", "start": alice, "max_depth": 1}]) == [[alice, bob]]
    snapshot_batches = snapshot.search_vectors("embeddings", [[1.0, 0.0], [0.5, 0.5]], limit=1)
    assert [[row["id"] for row in batch] for batch in snapshot_batches] == [[alice], [bob]]
    assert graph.snapshots()[0]["snapshot_id"] == snapshot.snapshot_id
    assert graph.snapshot(snapshot.snapshot_id).node_count() == 4
    assert snapshot.delete() is True

    assert graph.delete_vector("embeddings", bob) is True
    assert graph.drop_vector_index("embeddings")["dropped"] is True
    assert graph.drop_fulltext_index("people")["dropped"] is True
    assert graph.delete_edge(edge + 1) is True
    assert graph.delete_node(3) is True
    assert graph.delete_node(carol) is True
    assert graph.refresh()["refreshed"] is True
    assert graph.compact()["compacted"] is True


def test_client_timeout_and_headers_are_passed(monkeypatch) -> None:  # type: ignore[no-untyped-def]
    captured: dict[str, Any] = {}

    class FakeResponse:
        def __enter__(self):  # type: ignore[no-untyped-def]
            return self

        def __exit__(self, *args):  # type: ignore[no-untyped-def]
            return None

        def read(self) -> bytes:
            return b'{"status":"ok"}'

    def fake_urlopen(request, timeout):  # type: ignore[no-untyped-def]
        captured["timeout"] = timeout
        headers = {key.lower(): value for key, value in request.header_items()}
        captured["authorization"] = request.get_header("Authorization")
        captured["custom"] = headers.get("x-test")
        captured["url"] = request.full_url
        return FakeResponse()

    monkeypatch.setattr("urllib.request.urlopen", fake_urlopen)
    client = TongGraphClient("http://example.test", token="secret", timeout=12.5, headers={"X-Test": "yes"})

    assert client.health() == {"status": "ok"}
    assert captured == {
        "timeout": 12.5,
        "authorization": "Bearer secret",
        "custom": "yes",
        "url": "http://example.test/health",
    }


def test_client_import_boundary_and_http_error_mapping() -> None:
    from tonggraph.server.client import RemoteGraph, RemoteSnapshot

    assert TongGraphClient.__name__ == "TongGraphClient"
    assert RemoteGraph.__name__ == "RemoteGraph"
    assert RemoteSnapshot.__name__ == "RemoteSnapshot"
    assert "fastapi" not in sys.modules or sys.modules["fastapi"] is not None

    body = b'{"error":{"code":"permission_denied","message":"nope","graph":"g","request_id":"req-1"}}'
    error = urllib.error.HTTPError("http://example.test", 403, "Forbidden", {}, _Bytes(body))
    mapped = _error_from_http(error)
    assert mapped.code == "permission_denied"
    assert mapped.message == "nope"
    assert mapped.status_code == 403
    assert mapped.graph == "g"
    assert mapped.request_id == "req-1"


class _Bytes:
    def __init__(self, body: bytes) -> None:
        self.body = body

    def read(self) -> bytes:
        return self.body

    def close(self) -> None:
        pass


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])
