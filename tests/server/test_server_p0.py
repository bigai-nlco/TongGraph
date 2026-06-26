from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from fastapi.testclient import TestClient

from tonggraph.server.app import create_app
from tonggraph.server.config import parse_config


def make_client(tmp_path: Path) -> TestClient:
    config = parse_config(
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
                    "alice": {
                        "token": "alice-token",
                        "graphs": {"shared": "write"},
                    },
                    "bob": {"token": "bob-token", "graphs": {}},
                },
            },
        }
    )
    return TestClient(create_app(config))


def auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def assert_error(response, code: str) -> None:  # type: ignore[no-untyped-def]
    assert response.status_code >= 400
    payload = response.json()
    assert payload["error"]["code"] == code
    assert "message" in payload["error"]
    assert "request_id" in payload["error"]


def test_health_auth_acl_admin_graph_persistence(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv("TONGGRAPH_TEST_ADMIN_TOKEN", "admin-token")
    client = make_client(tmp_path)

    assert client.get("/health").json()["status"] == "ok"
    assert_error(client.get("/graphs"), "unauthenticated")
    assert client.get("/graphs", headers=auth("alice-token")).json()["graphs"][0]["name"] == "shared"
    assert client.get("/graphs", headers=auth("bob-token")).json()["graphs"] == []

    denied = client.post("/admin/graphs", headers=auth("alice-token"), json={"name": "alice_memory"})
    assert_error(denied, "admin_required")

    created = client.post(
        "/admin/graphs",
        headers=auth("admin-token"),
        json={"name": "alice_memory", "grants": {"alice": "write", "bob": "read"}},
    )
    assert created.status_code == 200
    assert created.json()["graph"]["name"] == "alice_memory"
    assert (tmp_path / "server-state.json").exists()

    # Recreate the app to verify server-state.json reloads dynamic graphs and grants.
    reloaded = make_client(tmp_path)
    names = [item["name"] for item in reloaded.get("/graphs", headers=auth("alice-token")).json()["graphs"]]
    assert "alice_memory" in names
    bob_names = [item["name"] for item in reloaded.get("/graphs", headers=auth("bob-token")).json()["graphs"]]
    assert bob_names == ["alice_memory"]

    readonly = reloaded.post(
        "/graphs/alice_memory/nodes",
        headers=auth("bob-token"),
        json={"external_id": "blocked"},
    )
    assert_error(readonly, "permission_denied")

    revoked = reloaded.delete("/admin/graphs/alice_memory/grants/bob", headers=auth("admin-token"))
    assert revoked.status_code == 200
    assert reloaded.get("/graphs", headers=auth("bob-token")).json()["graphs"] == []


def test_records_retrieval_query_and_cypher(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv("TONGGRAPH_TEST_ADMIN_TOKEN", "admin-token")
    client = make_client(tmp_path)
    headers = auth("alice-token")

    alice = client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "alice", "labels": ["Person"], "properties": {"name": "Alice", "active": True}},
    ).json()["id"]
    bob = client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "bob", "labels": ["Person"], "properties": {"name": "Bob"}},
    ).json()["id"]
    edge = client.post(
        "/graphs/shared/edges",
        headers=headers,
        json={"source": alice, "target": bob, "edge_type": "KNOWS", "properties": {"note": "graph friend"}},
    ).json()["id"]

    assert client.get(f"/graphs/shared/nodes/{alice}", headers=headers).json()["node"]["external_id"] == "alice"
    assert client.get(f"/graphs/shared/edges/{edge}", headers=headers).json()["edge"]["edge_type"] == "KNOWS"
    assert client.get("/graphs/shared/nodes/count", headers=headers).json()["count"] == 2
    assert client.get("/graphs/shared/edges/count", headers=headers).json()["count"] == 1
    assert client.get("/graphs/shared/nodes/by-label/Person", headers=headers).json()["ids"] == [alice, bob]
    assert client.get("/graphs/shared/nodes/by-property", headers=headers, params={"key": "active", "value": "true"}).json()["ids"] == [alice]
    assert client.get("/graphs/shared/edges/by-type/KNOWS", headers=headers).json()["ids"] == [edge]

    ft = client.post(
        "/graphs/shared/fulltext/indexes",
        headers=headers,
        json={"name": "people", "properties": ["name"], "target": "node"},
    )
    assert ft.status_code == 200
    text_results = client.post(
        "/graphs/shared/fulltext/people/search",
        headers=headers,
        json={"query": "Alice", "labels": ["Person"]},
    ).json()["results"]
    assert text_results[0]["id"] == alice

    vi = client.post(
        "/graphs/shared/vector/indexes",
        headers=headers,
        json={"name": "embeddings", "dimensions": 2, "target": "node", "metric": "cosine"},
    )
    assert vi.status_code == 200
    assert client.put(f"/graphs/shared/vector/embeddings/{alice}", headers=headers, json={"vector": [1.0, 0.0]}).status_code == 200
    assert client.put(f"/graphs/shared/vector/embeddings/{bob}", headers=headers, json={"vector": [0.5, 0.5]}).status_code == 200
    vector_results = client.post(
        "/graphs/shared/vector/embeddings/search",
        headers=headers,
        json={"query_vector": [1.0, 0.0], "labels": ["Person"]},
    ).json()["results"]
    assert [row["id"] for row in vector_results] == [alice, bob]
    batch_results = client.post(
        "/graphs/shared/vector/embeddings/search-batch",
        headers=headers,
        json={"query_vectors": [[1.0, 0.0], [0.5, 0.5]], "labels": ["Person"], "limit": 1},
    ).json()["results"]
    assert [[row["id"] for row in batch] for batch in batch_results] == [[alice], [bob]]
    invalid_batch = client.post(
        "/graphs/shared/vector/embeddings/search-batch",
        headers=headers,
        json={"query_vectors": [[1.0, 0.0], [1.0]]},
    )
    assert_error(invalid_batch, "invalid_request")
    assert "index 1" in invalid_batch.json()["error"]["message"]
    assert client.get(f"/graphs/shared/vector/embeddings/{alice}", headers=headers).json()["vector"] == [1.0, 0.0]

    query_rows = client.post(
        "/graphs/shared/query",
        headers=headers,
        json={"spec": {"match": [{"node": "n", "external_id": "alice"}]}},
    ).json()["result"]
    assert query_rows == [{"n": alice}]

    cypher_rows = client.post(
        "/graphs/shared/cypher",
        headers=headers,
        json={"query": "MATCH (n:Person) RETURN count(*) AS total"},
    ).json()["result"]["records"]
    assert cypher_rows == [{"total": 2}]

    tx = client.post(
        "/graphs/shared/cypher/transaction",
        headers=headers,
        json={"statements": [{"query": "CREATE (n:Person {external_id: 'carol', name: 'Carol'}) RETURN n"}]},
    )
    assert tx.status_code == 200
    assert client.get("/graphs/shared/nodes/count", headers=headers).json()["count"] == 3


def test_concurrent_writes_and_error_shape(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv("TONGGRAPH_TEST_ADMIN_TOKEN", "admin-token")
    client = make_client(tmp_path)
    headers = auth("alice-token")

    def create_node(index: int) -> int:
        response = client.post(
            "/graphs/shared/nodes",
            headers=headers,
            json={"external_id": f"node-{index}", "properties": {"index": index}},
        )
        assert response.status_code == 200
        return response.json()["id"]

    with ThreadPoolExecutor(max_workers=4) as executor:
        ids = list(executor.map(create_node, range(8)))

    assert sorted(ids) == list(range(8))
    assert client.get("/graphs/shared/nodes/count", headers=headers).json()["count"] == 8

    missing = client.get("/graphs/shared/nodes/999", headers=headers)
    assert_error(missing, "not_found")

    invalid_graph = client.post("/admin/graphs", headers=auth("admin-token"), json={"name": "../bad"})
    assert_error(invalid_graph, "invalid_request")
