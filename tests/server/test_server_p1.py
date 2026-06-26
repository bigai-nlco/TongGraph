from __future__ import annotations

import time
from pathlib import Path

import pytest
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
                    "alice": {"token": "alice-token", "graphs": {"shared": "write"}},
                    "bob": {"token": "bob-token", "graphs": {"shared": "read"}},
                    "carol": {"token": "carol-token", "graphs": {}},
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


def add_node(client: TestClient, external_id: str) -> int:
    return client.post("/graphs/shared/nodes", headers=auth("alice-token"), json={"external_id": external_id}).json()["id"]


def build_algorithm_graph(client: TestClient) -> dict[str, int]:
    ids = {name: add_node(client, name) for name in ["a", "b", "c", "d", "e"]}
    edges = [
        ("a", "b", 2.0),
        ("a", "c", 1.0),
        ("c", "b", 0.5),
        ("b", "d", 1.0),
    ]
    for source, target, weight in edges:
        response = client.post(
            "/graphs/shared/edges",
            headers=auth("alice-token"),
            json={
                "source": ids[source],
                "target": ids[target],
                "edge_type": "LINK",
                "properties": {"weight": weight},
            },
        )
        assert response.status_code == 200
    return ids


def test_traversal_algorithms_subgraph_and_compute_batch(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv("TONGGRAPH_TEST_ADMIN_TOKEN", "admin-token")
    client = make_client(tmp_path)
    ids = build_algorithm_graph(client)
    headers = auth("alice-token")

    assert client.get(f"/graphs/shared/traversal/neighbors/{ids['a']}", headers=headers).json()["ids"] == [ids["b"], ids["c"]]
    assert client.get(
        "/graphs/shared/traversal/k-hop",
        headers=headers,
        params={"start": ids["a"], "hops": 2, "edge_type": "LINK"},
    ).json()["ids"] == [ids["b"], ids["c"], ids["d"]]
    assert client.post(
        "/graphs/shared/traversal/frontier",
        headers=headers,
        json={"starts": [ids["a"]], "steps": 2, "edge_type": "LINK"},
    ).json()["ids"] == [ids["d"]]

    assert client.get(
        "/graphs/shared/algorithms/bfs", headers=headers, params={"start": ids["a"], "max_depth": 1}
    ).json()["ids"] == [ids["a"], ids["b"], ids["c"]]
    assert client.get(
        "/graphs/shared/algorithms/shortest-path",
        headers=headers,
        params={"start": ids["a"], "target": ids["b"], "weight_property": "weight"},
    ).json()["path"] == {"nodes": [ids["a"], ids["c"], ids["b"]], "distance": 1.5}
    assert client.get("/graphs/shared/algorithms/connected-components", headers=headers).json()["components"] == [
        [ids["a"], ids["b"], ids["c"], ids["d"]],
        [ids["e"]],
    ]

    scores = client.get(
        "/graphs/shared/algorithms/pagerank",
        headers=headers,
        params={"iterations": 25, "tolerance": 1e-12},
    ).json()["scores"]
    assert set(scores) == {str(value) for value in ids.values()}
    assert sum(scores.values()) == pytest.approx(1.0)

    walk_one = client.get(
        "/graphs/shared/algorithms/random-walk", headers=headers, params={"start": ids["a"], "steps": 4, "seed": 7}
    ).json()["ids"]
    walk_two = client.get(
        "/graphs/shared/algorithms/random-walk", headers=headers, params={"start": ids["a"], "steps": 4, "seed": 7}
    ).json()["ids"]
    assert walk_one == walk_two

    subgraph = client.post(
        "/graphs/shared/subgraph",
        headers=headers,
        json={"nodes": [ids["a"], ids["b"], ids["c"]], "edge_type": "LINK"},
    ).json()["snapshot"]
    assert subgraph["node_count"] == 3
    assert subgraph["edge_count"] == 3
    assert subgraph["node_ids"] == [ids["a"], ids["b"], ids["c"]]

    results = client.post(
        "/graphs/shared/compute/batch",
        headers=headers,
        json={
            "jobs": [
                {"op": "bfs", "start": ids["a"], "max_depth": 1},
                {"op": "shortest_path", "start": ids["a"], "target": ids["b"], "weight_property": "weight"},
                {"op": "subgraph", "nodes": [ids["a"], ids["b"]], "edge_type": "LINK"},
            ]
        },
    ).json()["results"]
    assert results[0] == [ids["a"], ids["b"], ids["c"]]
    assert results[1] == {"nodes": [ids["a"], ids["c"], ids["b"]], "distance": 1.5}
    assert results[2]["node_count"] == 2

    assert_error(
        client.get(f"/graphs/shared/traversal/neighbors/{ids['a']}", headers=headers, params={"direction": "sideways"}),
        "invalid_request",
    )
    assert_error(
        client.post("/graphs/shared/compute/batch", headers=headers, json={"jobs": [{"op": "missing"}]}),
        "invalid_request",
    )


def test_snapshot_lifecycle_stability_query_cypher_and_ttl(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv("TONGGRAPH_TEST_ADMIN_TOKEN", "admin-token")
    client = make_client(tmp_path)
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
    client.post("/graphs/shared/edges", headers=headers, json={"source": alice, "target": bob, "edge_type": "KNOWS"})

    snapshot = client.post("/graphs/shared/snapshots", headers=headers, json={"ttl_seconds": 0.2}).json()["snapshot"]
    snapshot_id = snapshot["snapshot_id"]
    assert snapshot["owner_user_id"] == "alice"

    client.post(
        "/graphs/shared/nodes",
        headers=headers,
        json={"external_id": "carol", "labels": ["Person"], "properties": {"name": "Carol"}},
    )
    assert client.get("/graphs/shared/nodes/count", headers=headers).json()["count"] == 3
    assert client.get(f"/graphs/shared/snapshots/{snapshot_id}/nodes/count", headers=headers).json()["count"] == 2
    assert client.get(f"/graphs/shared/snapshots/{snapshot_id}/nodes/{alice}", headers=headers).json()["node"]["external_id"] == "alice"

    query_result = client.post(
        f"/graphs/shared/snapshots/{snapshot_id}/query",
        headers=headers,
        json={"spec": {"match": [{"node": "n", "external_id": "carol"}]}},
    ).json()["result"]
    assert query_result == []

    cypher_records = client.post(
        f"/graphs/shared/snapshots/{snapshot_id}/cypher",
        headers=headers,
        json={"query": "MATCH (n:Person) RETURN count(*) AS total"},
    ).json()["result"]["records"]
    assert cypher_records == [{"total": 2}]

    batch = client.post(
        f"/graphs/shared/snapshots/{snapshot_id}/compute/batch",
        headers=headers,
        json={"jobs": [{"op": "bfs", "start": alice, "max_depth": 1}]},
    ).json()["results"]
    assert batch == [[alice, bob]]

    client.post(
        "/graphs/shared/vector/indexes",
        headers=headers,
        json={"name": "embeddings", "dimensions": 2, "target": "node", "metric": "cosine"},
    )
    client.put(f"/graphs/shared/vector/embeddings/{alice}", headers=headers, json={"vector": [1.0, 0.0]})
    client.put(f"/graphs/shared/vector/embeddings/{bob}", headers=headers, json={"vector": [0.5, 0.5]})
    vector_snapshot = client.post("/graphs/shared/snapshots", headers=headers, json={"ttl_seconds": 60}).json()["snapshot"]["snapshot_id"]
    client.put(f"/graphs/shared/vector/embeddings/{alice}", headers=headers, json={"vector": [0.0, 1.0]})
    snapshot_vectors = client.post(
        f"/graphs/shared/snapshots/{vector_snapshot}/vector/embeddings/search-batch",
        headers=headers,
        json={"query_vectors": [[1.0, 0.0], [0.5, 0.5]], "limit": 1},
    ).json()["results"]
    assert [[row["id"] for row in batch] for batch in snapshot_vectors] == [[alice], [bob]]

    time.sleep(0.25)
    assert_error(client.get(f"/graphs/shared/snapshots/{snapshot_id}/nodes/count", headers=headers), "snapshot_not_found")


def test_snapshot_owner_admin_and_access_rules(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv("TONGGRAPH_TEST_ADMIN_TOKEN", "admin-token")
    client = make_client(tmp_path)
    alice_headers = auth("alice-token")
    bob_headers = auth("bob-token")
    admin_headers = auth("admin-token")

    a = add_node(client, "a")
    b = add_node(client, "b")
    client.post("/graphs/shared/edges", headers=alice_headers, json={"source": a, "target": b, "edge_type": "LINK"})

    assert client.get("/graphs/shared/algorithms/bfs", headers=bob_headers, params={"start": a}).json()["ids"] == [a, b]
    assert_error(
        client.get("/graphs/shared/algorithms/bfs", headers=auth("carol-token"), params={"start": a}),
        "permission_denied",
    )

    snapshot_id = client.post("/graphs/shared/snapshots", headers=alice_headers, json={"ttl_seconds": 60}).json()["snapshot"]["snapshot_id"]
    assert client.get("/graphs/shared/snapshots", headers=bob_headers).json()["snapshots"] == []
    assert_error(client.get(f"/graphs/shared/snapshots/{snapshot_id}/nodes/count", headers=bob_headers), "permission_denied")
    assert_error(client.delete(f"/graphs/shared/snapshots/{snapshot_id}", headers=bob_headers), "permission_denied")

    admin_list = client.get("/graphs/shared/snapshots", headers=admin_headers).json()["snapshots"]
    assert [item["snapshot_id"] for item in admin_list] == [snapshot_id]
    assert client.delete(f"/graphs/shared/snapshots/{snapshot_id}", headers=admin_headers).status_code == 200
    assert_error(client.get(f"/graphs/shared/snapshots/{snapshot_id}/nodes/count", headers=alice_headers), "snapshot_not_found")
