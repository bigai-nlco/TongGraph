from __future__ import annotations

from pathlib import Path
from sqlite3 import connect

from tonggraph import Graph


def test_in_memory_graph_retrieval_and_indexes() -> None:
    graph = Graph()
    alice = graph.add_node("alice", labels=["Person"], properties={"name": "Alice"})
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
    assert graph.get_node(alice).properties == {"name": "Alice"}
    assert graph.get_edge(knows).edge_type == "KNOWS"
    assert graph.nodes_with_label("Person") == [alice, bob]
    assert graph.edges_by_type("KNOWS") == [knows]
    assert graph.neighbors(alice) == [bob]
    assert graph.neighbors(alice, edge_type="SUPPORTS") == []
    assert graph.k_hop(alice, 2) == [bob, claim]


def test_sqlite_backed_graph_reopens_with_indexes(tmp_path: Path) -> None:
    db_path = tmp_path / "tonggraph.db"
    graph = Graph(str(db_path))
    source = graph.add_node("source", labels=["Entity"], properties={"name": "A"})
    target = graph.add_node("target", labels=["Entity"], properties={"name": "B"})
    graph.add_edge(source, target, "LINKS", properties={"probability": "0.75"})
    del graph

    with connect(db_path) as connection:
        rows = connection.execute(
            "SELECT op, object_id, payload FROM op_log ORDER BY seq"
        ).fetchall()
    assert rows == [
        ("add_node", source, "source"),
        ("add_node", target, "target"),
        ("add_edge", 0, "LINKS"),
    ]

    reopened = Graph(str(db_path))
    assert reopened.node_count() == 2
    assert reopened.edge_count() == 1
    assert reopened.get_node_id("source") == source
    assert reopened.nodes_with_label("Entity") == [source, target]
    assert reopened.neighbors(source, edge_type="LINKS") == [target]
    assert reopened.get_edge(0).properties["probability"] == "0.75"


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
