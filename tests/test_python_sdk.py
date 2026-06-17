from __future__ import annotations

from pathlib import Path
from sqlite3 import connect

import pytest

from tonggraph import Graph


def test_in_memory_graph_retrieval_and_indexes() -> None:
    graph = Graph()
    alice = graph.add_node(
        "alice",
        labels=["Person"],
        properties={"name": "Alice", "rank": 1, "active": True},
    )
    bob = graph.add_node("bob", labels=["Person"], properties={"name": "Bob"})
    claim = graph.add_node("claim:1", labels=["Claim"])

    knows = graph.add_edge(
        alice,
        bob,
        "KNOWS",
        properties={"probability": "0.5"},
    )
    graph.add_edge(bob, claim, "SUPPORTS", properties={"probability": "0.25"})

    assert graph.node_count() == 3
    assert graph.edge_count() == 2
    assert graph.get_node_id("alice") == alice
    assert graph.get_node(alice).properties == {
        "name": "Alice",
        "rank": 1,
        "active": True,
    }
    assert graph.get_edge(knows).edge_type == "KNOWS"
    assert graph.nodes_with_label("Person") == [alice, bob]
    assert graph.edges_by_type("KNOWS") == [knows]
    assert graph.nodes_with_property("name") == [alice, bob]
    assert graph.nodes_with_property("rank", 1) == [alice]
    assert graph.nodes_with_property("active", True) == [alice]
    assert graph.edges_with_property("probability", "0.5") == [knows]
    assert graph.neighbors(alice) == [bob]
    assert graph.neighbors(alice, edge_type="SUPPORTS") == []
    assert graph.frontier([alice], 2) == [claim]
    assert graph.k_hop(alice, 2) == [bob, claim]

    snapshot = graph.snapshot()
    graph.add_node("later")
    assert snapshot.node_count() == 3
    assert snapshot.nodes_with_property("rank", 1) == [alice]


def test_sqlite_backed_graph_reopens_with_indexes(tmp_path: Path) -> None:
    db_path = tmp_path / "tonggraph.db"
    graph = Graph(str(db_path))
    source = graph.add_node("source", labels=["Entity"], properties={"name": "A"})
    target = graph.add_node("target", labels=["Entity"], properties={"name": "B"})
    graph.add_edge(source, target, "LINKS", properties={"probability": 0.75})
    variable = graph.add_variable(
        "binary",
        owner_id=source,
        prior={"p": 0.25},
        posterior={"p": 0.5},
    )
    factor = graph.add_factor([variable], [], "likelihood", parameters={"weight": 2})
    evidence = graph.add_evidence(variable, {"observed": True})
    trace = graph.add_trace({"step": 1, "note": "initial"})
    graph.compact()
    del graph

    with connect(db_path) as connection:
        rows = connection.execute(
            "SELECT op, object_id, payload FROM op_log ORDER BY seq"
        ).fetchall()
        node_property_rows = connection.execute(
            "SELECT node_id, key, value_type, value_text FROM node_properties "
            "ORDER BY node_id, key"
        ).fetchall()
        edge_property_rows = connection.execute(
            "SELECT edge_id, key, value_type, value_text FROM edge_properties "
            "ORDER BY edge_id, key"
        ).fetchall()
        property_key_rows = connection.execute(
            "SELECT scope, key FROM property_keys ORDER BY scope, key"
        ).fetchall()
        property_value_rows = connection.execute(
            "SELECT scope, key, value_type, value_text FROM property_values "
            "ORDER BY scope, key, value_type, value_text"
        ).fetchall()
    assert rows == [
        ("add_node", source, "source"),
        ("add_node", target, "target"),
        ("add_edge", 0, "LINKS"),
        ("add_variable", variable, "binary"),
        ("add_factor", factor, "likelihood"),
        ("add_evidence", evidence, str(variable)),
        ("add_trace", trace, "trace"),
    ]
    assert node_property_rows == [
        (source, "name", "string", "A"),
        (target, "name", "string", "B"),
    ]
    assert edge_property_rows == [(0, "probability", "float", "0.75")]
    assert property_key_rows == [("edge", "probability"), ("node", "name")]
    assert property_value_rows == [
        ("edge", "probability", "float", "0.75"),
        ("node", "name", "string", "A"),
        ("node", "name", "string", "B"),
    ]
    assert (Path(f"{db_path}.segments") / "manifest.txt").exists()

    reopened = Graph(str(db_path))
    assert reopened.node_count() == 2
    assert reopened.edge_count() == 1
    assert reopened.variable_count() == 1
    assert reopened.factor_count() == 1
    assert reopened.evidence_count() == 1
    assert reopened.trace_count() == 1
    assert reopened.get_node_id("source") == source
    assert reopened.nodes_with_label("Entity") == [source, target]
    assert reopened.nodes_with_property("name", "A") == [source]
    assert reopened.edges_with_property("probability", 0.75) == [0]
    assert reopened.neighbors(source, edge_type="LINKS") == [target]
    assert reopened.get_edge(0).properties["probability"] == 0.75
    assert reopened.get_variable(variable).prior == {"p": 0.25}
    assert reopened.get_variable(variable).posterior == {"p": 0.5}
    assert reopened.posterior(variable) == {"false": 0.5, "true": 0.5}
    assert reopened.get_factor(factor).parameters == {"weight": 2}
    assert reopened.get_evidence(evidence).payload == {"observed": True}
    assert reopened.get_trace(trace).payload == {"step": 1, "note": "initial"}


def test_graph_compute_runtime_algorithms_and_batch() -> None:
    graph = Graph()
    a = graph.add_node("a")
    b = graph.add_node("b")
    c = graph.add_node("c")
    d = graph.add_node("d")
    e = graph.add_node("e")
    graph.add_edge(a, b, "LINK", properties={"weight": 2.0})
    graph.add_edge(a, c, "LINK", properties={"weight": 1.0})
    graph.add_edge(c, b, "LINK", properties={"weight": 0.5})
    graph.add_edge(b, d, "LINK", properties={"weight": 1.0})

    assert graph.bfs(a, max_depth=1) == [a, b, c]
    assert graph.shortest_path(a, b, weight_property="weight") == {
        "nodes": [a, c, b],
        "distance": 1.5,
    }
    assert graph.shortest_path(d, e) is None
    assert graph.connected_components() == [[a, b, c, d], [e]]

    ranks = graph.pagerank(iterations=25, tolerance=1e-12)
    assert set(ranks) == {a, b, c, d, e}
    assert sum(ranks.values()) == pytest.approx(1.0)
    assert graph.random_walk(a, 4, seed=7) == graph.random_walk(a, 4, seed=7)

    subgraph = graph.subgraph([a, b, c])
    assert subgraph.node_count() == 3
    assert subgraph.edge_count() == 3
    assert subgraph.get_edge(2).source == c

    snapshot = graph.snapshot()
    assert snapshot.bfs(a, max_depth=1) == [a, b, c]
    assert snapshot.shortest_path(a, b, weight_property="weight") == {
        "nodes": [a, c, b],
        "distance": 1.5,
    }

    results = graph.compute_batch(
        [
            {"op": "bfs", "start": a, "max_depth": 1},
            {
                "op": "shortest_path",
                "start": a,
                "target": b,
                "weight_property": "weight",
            },
            {"op": "pagerank", "iterations": 5},
        ]
    )
    assert results[0] == [a, b, c]
    assert results[1] == {"nodes": [a, c, b], "distance": 1.5}
    assert set(results[2]) == {a, b, c, d, e}


def test_weighted_shortest_path_rejects_negative_weights() -> None:
    graph = Graph()
    a = graph.add_node()
    b = graph.add_node()
    graph.add_edge(a, b, "LINK", properties={"weight": -1.0})

    with pytest.raises(ValueError, match="finite and non-negative"):
        graph.shortest_path(a, b, weight_property="weight")


def test_weighted_probability_transfer_over_sparse_edges() -> None:
    graph = Graph()
    a = graph.add_node()
    b = graph.add_node()
    c = graph.add_node()
    graph.add_edge(a, b, "P", properties={"probability": "0.5"})
    graph.add_edge(b, c, "P", properties={"probability": "0.25"})

    result = graph.propagate({a: 1.0}, 2)
    assert result[a] == 1.0
    assert result[b] == 0.5
    assert result[c] == 0.125

    local = graph.local_propagate({a: 1.0}, radius=1, edge_type="P")
    assert local[a] == 1.0
    assert local[b] == 0.5
    assert c not in local
    with pytest.raises(ValueError, match="finite and non-negative"):
        graph.local_propagate({a: -1.0}, radius=1)
    with pytest.raises(ValueError, match="damping"):
        graph.local_propagate({a: 1.0}, radius=1, damping=1.5)


def test_belief_propagation_api_posterior_and_reopen(tmp_path: Path) -> None:
    db_path = tmp_path / "belief.db"
    graph = Graph(str(db_path))
    source = graph.add_node("source")
    target = graph.add_node("target")
    graph.add_edge(source, target, "LINK")
    parent = graph.add_variable("binary", source, {"p": 0.6}, {})
    child = graph.add_variable("binary", owner_id=target)
    weather = graph.add_variable(
        "categorical",
        states=["sun", "rain", "snow"],
        prior={"sun": 0.5, "rain": 0.25, "snow": 0.25},
    )

    assert graph.get_variable(parent).states == ["false", "true"]
    assert graph.get_variable(weather).states == ["sun", "rain", "snow"]
    assert graph.posterior(child) == {"false": 0.5, "true": 0.5}

    factor = graph.add_cpd(child, [parent], [0.9, 0.1, 0.2, 0.8])
    active = graph.compile_active_subgraph([child], evidence={parent: "true"}, radius=1)
    assert active["variables"] == [parent, child]
    assert active["factors"] == [factor]
    assert active["truncated"] is False

    result = graph.belief_propagation(
        [child],
        evidence={parent: "true"},
        tolerance=1e-12,
        damping=0.0,
        persist=False,
    )
    assert result["schedule"] == "residual_async"
    assert result["converged"] is True
    assert result["beliefs"][child]["false"] == pytest.approx(0.2)
    assert result["beliefs"][child]["true"] == pytest.approx(0.8)
    assert graph.posterior(child) == {"false": 0.5, "true": 0.5}

    persisted = graph.belief_propagation(
        [child],
        evidence={parent: "true"},
        tolerance=1e-12,
        damping=0.0,
        persist=True,
    )
    assert persisted["trace_id"] == 0
    assert graph.posterior(child)["true"] == pytest.approx(0.8)
    del graph

    reopened = Graph(str(db_path))
    assert reopened.posterior(child)["false"] == pytest.approx(0.2)
    assert reopened.posterior(child)["true"] == pytest.approx(0.8)
    assert reopened.trace_count() == 1


def test_belief_propagation_rejects_invalid_domains_and_potentials() -> None:
    graph = Graph()
    with pytest.raises(ValueError, match="states are required"):
        graph.add_variable("categorical")

    variable = graph.add_variable("binary")
    with pytest.raises(ValueError, match="all zero"):
        graph.add_factor_table([variable], [0.0, 0.0])
    with pytest.raises(ValueError, match="finite and non-negative"):
        graph.add_factor_table([variable], [1.0, float("nan")])
    with pytest.raises(ValueError, match="duplicate variable"):
        graph.add_factor_table([variable, variable], [1.0, 0.0, 0.0, 1.0])

    parent = graph.add_variable("binary")
    child = graph.add_variable("binary")
    with pytest.raises(ValueError, match="sum to 1.0"):
        graph.add_cpd(child, [parent], [90.0, 10.0, 2.0, 8.0])
    with pytest.raises(ValueError, match="state"):
        graph.belief_propagation([variable], evidence={variable: "missing"})
