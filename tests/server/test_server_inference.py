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
                    "carol": {"token": "carol-token", "graphs": {}},
                },
            },
            "operations": {"request_logging": False},
        }
    )


def auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def assert_error(response, code: str) -> None:  # type: ignore[no-untyped-def]
    assert response.status_code >= 400
    assert response.json()["error"]["code"] == code
    assert "request_id" in response.json()["error"]


def add_node(client: TestClient, external_id: str) -> int:
    return client.post("/graphs/shared/nodes", headers=auth("alice-token"), json={"external_id": external_id}).json()["id"]


def test_probability_transfer_and_access_rules(tmp_path: Path) -> None:
    client = TestClient(create_app(make_config(tmp_path)))
    a = add_node(client, "a")
    b = add_node(client, "b")
    c = add_node(client, "c")
    client.post("/graphs/shared/edges", headers=auth("alice-token"), json={"source": a, "target": b, "edge_type": "P", "properties": {"probability": "0.5"}})
    client.post("/graphs/shared/edges", headers=auth("alice-token"), json={"source": b, "target": c, "edge_type": "P", "properties": {"probability": "0.25"}})
    client.post("/graphs/shared/edges", headers=auth("alice-token"), json={"source": a, "target": c, "edge_type": "Q", "properties": {"probability": "0.9"}})

    propagated = client.post(
        "/graphs/shared/propagate",
        headers=auth("bob-token"),
        json={"seeds": {str(a): 1.0}, "steps": 2, "edge_type": "P"},
    ).json()["scores"]
    assert propagated[str(a)] == 1.0
    assert propagated[str(b)] == 0.5
    assert propagated[str(c)] == 0.125

    local = client.post(
        "/graphs/shared/local-propagate",
        headers=auth("bob-token"),
        json={"seeds": {str(a): 1.0}, "radius": 1, "edge_type": "P"},
    ).json()["scores"]
    assert local[str(a)] == 1.0
    assert local[str(b)] == 0.5
    assert str(c) not in local

    invalid = client.post(
        "/graphs/shared/local-propagate",
        headers=auth("bob-token"),
        json={"seeds": {str(a): -1.0}, "radius": 1},
    )
    assert_error(invalid, "invalid_request")

    denied_write = client.post("/graphs/shared/variables", headers=auth("bob-token"), json={"domain": "binary"})
    assert_error(denied_write, "permission_denied")
    denied_access = client.post("/graphs/shared/propagate", headers=auth("carol-token"), json={"seeds": {str(a): 1.0}, "steps": 1})
    assert_error(denied_access, "permission_denied")


def test_inference_records_and_belief_propagation(tmp_path: Path) -> None:
    client = TestClient(create_app(make_config(tmp_path)))
    headers = auth("alice-token")
    source = add_node(client, "source")
    target = add_node(client, "target")
    client.post("/graphs/shared/edges", headers=headers, json={"source": source, "target": target, "edge_type": "LINK"})

    parent = client.post("/graphs/shared/variables", headers=headers, json={"domain": "binary", "owner_id": source, "prior": {"p": 0.6}, "posterior": {}}).json()["id"]
    child = client.post("/graphs/shared/variables", headers=headers, json={"domain": "binary", "owner_id": target}).json()["id"]
    weather = client.post("/graphs/shared/variables", headers=headers, json={"domain": "categorical", "states": ["sun", "rain", "snow"], "prior": {"sun": 0.5, "rain": 0.25, "snow": 0.25}}).json()["variable"]
    assert weather["states"] == ["sun", "rain", "snow"]
    assert client.get(f"/graphs/shared/variables/{parent}", headers=headers).json()["variable"]["states"] == ["false", "true"]
    assert client.get(f"/graphs/shared/variables/{child}/posterior", headers=headers).json()["posterior"] == {"false": 0.5, "true": 0.5}

    generic_factor = client.post("/graphs/shared/factors", headers=headers, json={"input_variables": [parent], "output_variables": [child], "function": "metadata", "parameters": {"kind": "test"}}).json()["id"]
    assert client.get(f"/graphs/shared/factors/{generic_factor}", headers=headers).json()["factor"]["function"] == "metadata"

    extra = client.post("/graphs/shared/variables", headers=headers, json={"domain": "binary"}).json()["id"]
    table = client.post("/graphs/shared/factor-tables", headers=headers, json={"variables": [extra], "values": [0.4, 0.6]}).json()["factor"]
    assert table["function"] == "factor_table"

    factor = client.post("/graphs/shared/cpds", headers=headers, json={"variable_id": child, "parent_variables": [parent], "values": [0.9, 0.1, 0.2, 0.8]}).json()["id"]
    evidence = client.post("/graphs/shared/evidence", headers=headers, json={"variable_id": parent, "payload": {"state": "true"}}).json()["id"]
    assert client.get(f"/graphs/shared/evidence/{evidence}", headers=headers).json()["evidence"]["payload"] == {"state": "true"}
    trace = client.post("/graphs/shared/traces", headers=headers, json={"payload": {"note": "manual"}}).json()["id"]
    assert client.get(f"/graphs/shared/traces/{trace}", headers=headers).json()["trace"]["payload"] == {"note": "manual"}

    active = client.post(
        "/graphs/shared/inference/active-subgraph",
        headers=auth("bob-token"),
        json={"query_variables": [child], "evidence": {str(parent): "true"}, "radius": 1},
    ).json()["active_subgraph"]
    assert active["variables"] == [parent, child]
    assert active["factors"] == [factor]
    assert active["truncated"] is False

    result = client.post(
        "/graphs/shared/belief-propagation",
        headers=auth("bob-token"),
        json={"query_variables": [child], "evidence": {str(parent): "true"}, "tolerance": 1e-12, "damping": 0.0},
    ).json()["result"]
    assert result["schedule"] == "residual_async"
    assert result["converged"] is True
    assert result["beliefs"][str(child)]["true"] == pytest.approx(0.8)
    assert client.get(f"/graphs/shared/variables/{child}/posterior", headers=headers).json()["posterior"] == {"false": 0.5, "true": 0.5}

    denied_persist = client.post(
        "/graphs/shared/belief-propagation",
        headers=auth("bob-token"),
        json={"query_variables": [child], "evidence": {str(parent): "true"}, "persist": True},
    )
    assert_error(denied_persist, "permission_denied")

    persisted = client.post(
        "/graphs/shared/belief-propagation",
        headers=headers,
        json={"query_variables": [child], "evidence": {str(parent): "true"}, "tolerance": 1e-12, "damping": 0.0, "persist": True},
    ).json()["result"]
    assert persisted["trace_id"] == 1
    posterior = client.get(f"/graphs/shared/variables/{child}/posterior", headers=headers).json()["posterior"]
    assert posterior["true"] == pytest.approx(0.8)


def test_python_client_inference_wrappers(tmp_path: Path, monkeypatch) -> None:  # type: ignore[no-untyped-def]
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
        source = graph.add_node("source")
        target = graph.add_node("target")
        graph.add_edge(source, target, "P", properties={"probability": "0.5"})
        assert graph.propagate({source: 1.0}, 1)[str(target)] == 0.5
        assert graph.local_propagate({source: 1.0}, radius=1)[str(target)] == 0.5

        parent = graph.add_variable("binary", owner_id=source, prior={"p": 0.6}, posterior={})
        child = graph.add_variable("binary", owner_id=target)
        assert graph.get_variable(parent)["states"] == ["false", "true"]
        assert graph.posterior(child) == {"false": 0.5, "true": 0.5}
        factor = graph.add_cpd(child, [parent], [0.9, 0.1, 0.2, 0.8])
        assert graph.get_factor(factor)["function"] == "cpd"
        evidence = graph.add_evidence(parent, {"state": "true"})
        assert graph.get_evidence(evidence)["payload"] == {"state": "true"}
        trace = graph.add_trace({"note": "client"})
        assert graph.get_trace(trace)["payload"] == {"note": "client"}
        table = graph.add_factor_table([graph.add_variable("binary")], [0.3, 0.7])
        assert graph.get_factor(table)["function"] == "factor_table"
        generic = graph.add_factor([parent], [child], "metadata", parameters={"kind": "client"})
        assert graph.get_factor(generic)["parameters"] == {"kind": "client"}

        active = graph.compile_active_subgraph([child], evidence={parent: "true"}, radius=1)
        assert active["variables"] == [parent, child]
        result = graph.belief_propagation([child], evidence={parent: "true"}, tolerance=1e-12, damping=0.0)
        assert result["beliefs"][str(child)]["true"] == pytest.approx(0.8)
        persisted = graph.belief_propagation([child], evidence={parent: "true"}, tolerance=1e-12, damping=0.0, persist=True)
        assert persisted["trace_id"] == 1
        assert graph.posterior(child)["true"] == pytest.approx(0.8)
    finally:
        server.should_exit = True
        thread.join(timeout=5)


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])
