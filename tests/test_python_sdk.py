from __future__ import annotations

from pathlib import Path
from sqlite3 import connect

import pytest

from tonggraph import Graph, query_dsl_schema, query_nl


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


def test_query_layer_structured_queries_snapshots_reopen_and_nl(tmp_path: Path) -> None:
    db_path = tmp_path / "query.db"
    graph = Graph(str(db_path))
    alice = graph.add_node(
        "alice",
        labels=["Person"],
        properties={"name": "Alice", "rank": 3, "active": True, "group": "ai"},
    )
    bob = graph.add_node(
        "bob",
        labels=["Person"],
        properties={"name": "Bob", "rank": 2, "active": True, "group": "research"},
    )
    carol = graph.add_node(
        "carol",
        labels=["Person"],
        properties={"name": "Carol", "rank": 4, "active": False, "group": "research"},
    )
    alice_knows_bob = graph.add_edge(
        alice,
        bob,
        "KNOWS",
        properties={"note": "team alpha", "weight": 0.8},
    )
    graph.add_edge(
        bob,
        carol,
        "KNOWS",
        properties={"note": "team beta", "weight": 0.6},
    )
    graph.add_edge(carol, alice, "KNOWS", properties={"note": "loop"})

    spec = {
        "match": [
            {
                "node": "a",
                "labels": ["Person"],
                "where": [
                    {"property": "rank", "op": "gte", "value": 2},
                    {
                        "property": "group",
                        "op": "in",
                        "value": ["ai", "research"],
                    },
                ],
            },
            {
                "edge": "rel",
                "type": "KNOWS",
                "direction": "out",
                "where": [{"property": "note", "op": "contains", "value": "team"}],
            },
            {"node": "b", "labels": ["Person"], "properties": {"active": True}},
        ],
        "return": ["a", "rel", "b"],
        "limit": 10,
    }
    expected = [{"a": alice, "rel": alice_knows_bob, "b": bob}]

    assert graph.query(spec) == expected
    assert graph.snapshot().query(spec) == expected
    assert graph.query_schema()["name"] == "tonggraph_query_dsl_v0"
    assert query_dsl_schema()["operators"] == [
        "eq",
        "ne",
        "lt",
        "lte",
        "gt",
        "gte",
        "in",
        "contains",
    ]

    def compiler(question: str, schema: dict[str, object]) -> dict[str, object]:
        assert question == "Who does Alice know on the team?"
        assert schema["name"] == "tonggraph_query_dsl_v0"
        return spec

    assert query_nl(graph, "Who does Alice know on the team?", compiler) == expected
    with pytest.raises(TypeError, match="mapping"):
        query_nl(graph, "bad compiler", lambda _question, _schema: [])  # type: ignore[return-value]

    del graph
    reopened = Graph(str(db_path))
    assert reopened.query(spec) == expected


def test_query_layer_rejects_wrong_pattern_shapes() -> None:
    graph = Graph()
    a = graph.add_node("a")
    b = graph.add_node("b")
    graph.add_edge(a, b, "KNOWS")

    with pytest.raises(ValueError, match="must be an edge pattern"):
        graph.query(
            {
                "match": [
                    {"node": "a"},
                    {"node": "b"},
                    {"node": "c"},
                ],
            }
        )

    with pytest.raises(ValueError, match="must be a node pattern"):
        graph.query(
            {
                "match": [
                    {"edge": "bad"},
                ],
            }
        )


def test_cypher_api_create_match_snapshot_transaction_and_reopen(tmp_path: Path) -> None:
    db_path = tmp_path / "cypher.db"
    graph = Graph(str(db_path))

    created = graph.cypher(
        """
        CREATE (a:Person {external_id: 'alice', name: 'Alice', rank: 3})
               -[:KNOWS {since: 2026}]->
               (b:Person {external_id: 'bob', name: 'Bob', rank: 2})
        RETURN a, b
        """
    )
    assert created.keys == ["a", "b"]
    assert created.summary["statement_type"] == "write"
    assert created.summary["nodes_created"] == 2
    assert created.summary["relationships_created"] == 1
    assert len(created) == 1
    assert created.records[0]["a"].external_id == "alice"
    assert created.records[0]["b"].properties["name"] == "Bob"

    rows = graph.cypher(
        """
        MATCH (a:Person)-[r:KNOWS]->(b:Person)
        WHERE a.name = $name AND b.rank IN [2, 3]
        RETURN a.name AS source, type(r) AS rel, b.name AS target, id(a) AS source_id
        ORDER BY target
        LIMIT 5
        """,
        {"name": "Alice"},
    )
    assert rows.keys == ["source", "rel", "target", "source_id"]
    assert rows.records == [
        {"source": "Alice", "rel": "KNOWS", "target": "Bob", "source_id": 0}
    ]
    assert graph.cypher(
        "MATCH (b:Person) WHERE b.rank IN [$rank] RETURN b.name AS name",
        {"rank": 2},
    ).records == [{"name": "Bob"}]

    snapshot = graph.snapshot()
    assert snapshot.cypher("MATCH (n:Person) RETURN count(*) AS total").records == [
        {"total": 2}
    ]
    with pytest.raises(ValueError, match="cannot execute write"):
        snapshot.cypher("CREATE (n:Person {name: 'Nope'}) RETURN n")

    with graph.transaction() as tx:
        staged = tx.run("CREATE (c:Person {external_id: 'carol', name: 'Carol'}) RETURN c")
        assert staged.records[0]["c"].external_id == "carol"
        assert graph.get_node_id("carol") is None
    assert graph.get_node_id("carol") == 2

    with graph.transaction() as tx:
        tx.run("CREATE (f:Person {external_id: 'frank', name: 'Frank'}) RETURN f")
        tx.commit()
    assert graph.get_node_id("frank") == 3

    with graph.transaction() as tx:
        tx.run("CREATE (g:Person {external_id: 'grace', name: 'Grace'}) RETURN g")
        tx.rollback()
    assert graph.get_node_id("grace") is None

    tx = graph.transaction()
    with pytest.raises(ValueError, match="relationship type"):
        tx.run(
            """
            CREATE (x:Person {external_id: 'partial:x'})
                   -[:]->
                   (y:Person {external_id: 'partial:y'})
            RETURN x
            """
        )
    tx.commit()
    assert graph.get_node_id("partial:x") is None
    assert graph.get_node_id("partial:y") is None

    tx = graph.transaction()
    tx.run("CREATE (d:Person {external_id: 'dave', name: 'Dave'}) RETURN d")
    tx.rollback()
    assert graph.get_node_id("dave") is None

    read_only = graph.transaction(write=False)
    with pytest.raises(ValueError, match="requires a writable"):
        read_only.run("CREATE (e:Person {name: 'Eve'}) RETURN e")
    read_only.rollback()

    del graph
    reopened = Graph(str(db_path))
    reopened_rows = reopened.cypher(
        "MATCH (n:Person) WHERE n.name CONTAINS 'o' RETURN n.name AS name ORDER BY name DESC"
    )
    assert reopened_rows.records == [{"name": "Carol"}, {"name": "Bob"}]


def test_graph_crud_sdk_and_cypher_persist_transactionally(tmp_path: Path) -> None:
    db_path = tmp_path / "crud.db"
    graph = Graph(str(db_path))
    alice = graph.add_node(
        "alice",
        labels=["Person"],
        properties={"name": "Alice", "obsolete": True},
    )
    bob = graph.add_node("bob", labels=["Person"])
    edge = graph.add_edge(alice, bob, "KNOWS", properties={"old": "yes"})

    updated = graph.update_node(
        alice,
        external_id="alice-2",
        add_labels=["Researcher"],
        remove_labels=["Person"],
        set_properties={"rank": 2},
        remove_properties=["obsolete"],
    )
    assert updated.external_id == "alice-2"
    assert updated.labels == ["Researcher"]
    assert updated.properties == {"name": "Alice", "rank": 2}
    assert graph.get_node_id("alice") is None
    assert graph.get_node_id("alice-2") == alice

    updated_edge = graph.update_edge(
        edge,
        set_properties={"weight": 0.75},
        remove_properties=["old"],
    )
    assert updated_edge.properties == {"weight": 0.75}
    with pytest.raises(ValueError, match="relationships"):
        graph.delete_node(alice)

    with graph.transaction() as tx:
        staged = tx.run(
            "MATCH (a {external_id: 'alice-2'})-[r:KNOWS]->(b) "
            "SET a.name = 'Alicia', a += {active: true}, r.weight = 0.8 "
            "REMOVE a.rank RETURN a.name AS name, r.weight AS weight"
        )
        assert staged.records == [{"name": "Alicia", "weight": 0.8}]
        assert staged.summary["properties_set"] == 3
        assert staged.summary["properties_removed"] == 1
        assert graph.get_node(alice).properties["name"] == "Alice"
    assert graph.get_node(alice).properties == {"name": "Alicia", "active": True}

    with graph.transaction() as tx:
        tx.run("MATCH (b {external_id: 'bob'}) DETACH DELETE b")
        tx.rollback()
    assert graph.get_node_id("bob") == bob

    deleted = graph.cypher(
        "MATCH (a {external_id: 'alice-2'})-[r:KNOWS]->(b) DELETE r, b"
    )
    assert deleted.summary["nodes_deleted"] == 1
    assert deleted.summary["relationships_deleted"] == 1
    assert graph.edge_count() == 0
    assert graph.node_count() == 1

    del graph
    reopened = Graph(str(db_path))
    assert reopened.get_node_id("alice-2") == alice
    assert reopened.get_node(alice).properties == {"name": "Alicia", "active": True}
    assert reopened.nodes_with_label("Researcher") == [alice]
    assert reopened.edge_count() == 0
    reopened.delete_node(alice)
    assert reopened.node_count() == 0


def test_failed_sqlite_graph_change_preserves_existing_segment(tmp_path: Path) -> None:
    db_path = tmp_path / "atomic-segment.db"
    graph = Graph(str(db_path))
    node = graph.add_node("node", properties={"value": 1})
    target = graph.add_node("target")
    graph.add_edge(node, target, "LINK")
    graph.compact()
    manifest = Path(f"{db_path}.segments") / "manifest.txt"
    segment = Path(f"{db_path}.segments") / "segment-v1.bin"
    assert manifest.exists()
    assert segment.exists()

    with connect(db_path) as connection:
        connection.execute(
            "CREATE TRIGGER fail_node_update BEFORE UPDATE ON nodes "
            "BEGIN SELECT RAISE(ABORT, 'forced failure'); END"
        )

    with pytest.raises(ValueError, match="forced failure"):
        graph.cypher(
            "MATCH (n {external_id: 'node'}) SET n.value = 2 RETURN n.value"
        )

    assert manifest.exists()
    assert segment.exists()
    assert graph.get_node(node).properties["value"] == 1
    with connect(db_path) as connection:
        stored = connection.execute(
            "SELECT properties FROM nodes WHERE id = ?", (node,)
        ).fetchone()
    assert stored == ("value\tint\t1",)


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

    graph.add_edge(a, c, "Q", properties={"probability": "0.9"})
    filtered = graph.propagate({a: 1.0}, 2, edge_type="P")
    assert filtered[a] == 1.0
    assert filtered[b] == 0.5
    assert filtered[c] == 0.125

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


def test_mutation_validation_and_lookup_errors_surface_cleanly() -> None:
    graph = Graph()
    node = graph.add_node("entity")

    invalid_operations = [
        (lambda: graph.add_node("entity"), 'external_id "entity" already exists'),
        (lambda: graph.add_node(""), "external_id cannot be empty"),
        (lambda: graph.add_node(labels=[""]), "label cannot be empty"),
        (lambda: graph.add_node(properties={"": "value"}), "property key cannot be empty"),
        (
            lambda: graph.add_node(properties={"payload": []}),
            "property values must be str, int, float, or bool",
        ),
        (
            lambda: graph.add_node(properties={"score": float("nan")}),
            "float property values must be finite",
        ),
        (lambda: graph.add_edge(node, 999, "LINK"), "node 999 not found"),
        (lambda: graph.add_edge(node, node, ""), "edge_type cannot be empty"),
    ]

    for operation, message in invalid_operations:
        with pytest.raises(ValueError, match=message):
            operation()

    with pytest.raises(KeyError, match="node 999 not found"):
        graph.get_node(999)
    with pytest.raises(KeyError, match="edge 999 not found"):
        graph.get_edge(999)


def test_traversal_direction_type_filters_and_snapshot_read_only() -> None:
    graph = Graph()
    alice = graph.add_node("alice")
    bob = graph.add_node("bob")
    carol = graph.add_node("carol")
    dave = graph.add_node("dave")
    graph.add_edge(alice, bob, "KNOWS")
    graph.add_edge(carol, bob, "KNOWS")
    graph.add_edge(bob, dave, "LIKES")
    graph.add_edge(dave, alice, "KNOWS")

    assert graph.neighbors(bob, direction="out") == [dave]
    assert graph.neighbors(bob, direction="in", edge_type="KNOWS") == [alice, carol]
    assert graph.neighbors(bob, direction="both") == [dave, alice, carol]
    assert graph.k_hop(bob, 2, direction="in", edge_type="KNOWS") == [
        alice,
        carol,
        dave,
    ]
    assert graph.frontier([bob, bob], 0) == [bob]
    assert graph.frontier([bob], 2, direction="in", edge_type="KNOWS") == [dave]
    assert graph.bfs(bob, direction="in", edge_type="KNOWS") == [bob, alice, carol, dave]
    assert graph.shortest_path(dave, bob, direction="out", edge_type="KNOWS") == {
        "nodes": [dave, alice, bob],
        "distance": 2.0,
    }
    assert graph.connected_components(edge_type="LIKES") == [[alice], [bob, dave], [carol]]

    snapshot = graph.snapshot()
    assert not hasattr(snapshot, "add_node")
    graph.add_node("later")
    assert snapshot.node_count() == 4

    with pytest.raises(ValueError, match="direction must be"):
        graph.neighbors(alice, direction="sideways")


def test_compute_batch_validation_and_subgraph_snapshot_result() -> None:
    graph = Graph()
    a = graph.add_node("a")
    b = graph.add_node("b")
    c = graph.add_node("c")
    graph.add_edge(a, b, "LINK", properties={"weight": 2.0})
    graph.add_edge(b, c, "LINK", properties={"weight": 3.0})

    results = graph.compute_batch(
        [
            {"op": "subgraph", "nodes": [a, b], "edge_type": "LINK"},
            {"op": "random_walk", "start": a, "steps": 2, "edge_type": "LINK", "seed": 11},
        ]
    )
    subgraph = results[0]
    assert subgraph.node_count() == 2
    assert subgraph.edge_count() == 1
    assert subgraph.neighbors(a, edge_type="LINK") == [b]
    assert results[1] == [a, b, c]

    invalid_batches = [
        ({"op": "bfs"}, "compute_batch jobs must be a list"),
        ([{"op": "missing"}], 'job 0 has unknown op "missing"'),
        ([{"op": "bfs", "start": 999}], "job 0: node 999 not found"),
        ([{"op": "random_walk", "start": a}], 'job 0 missing "steps"'),
    ]
    for jobs, message in invalid_batches:
        with pytest.raises(ValueError, match=message):
            graph.compute_batch(jobs)  # type: ignore[arg-type]


def test_query_layer_directions_repeated_aliases_defaults_and_validation() -> None:
    graph = Graph()
    alice = graph.add_node("alice", labels=["Person"])
    bob = graph.add_node("bob", labels=["Person"])
    carol = graph.add_node("carol", labels=["Person"])
    alice_to_bob = graph.add_edge(alice, bob, "KNOWS", properties={"rank": 1})
    bob_to_carol = graph.add_edge(bob, carol, "KNOWS", properties={"rank": 2})
    carol_to_alice = graph.add_edge(carol, alice, "KNOWS", properties={"rank": 3})

    assert graph.query(
        {
            "match": [
                {"node": "target", "id": alice},
                {"edge": "rel", "type": "KNOWS", "direction": "in"},
                {"node": "source"},
            ],
            "return": ["source", "rel"],
        }
    ) == [{"rel": carol_to_alice, "source": carol}]

    assert graph.query(
        {
            "match": [
                {"node": "center", "id": bob},
                {"edge": "rel", "type": "KNOWS", "direction": "both"},
                {"node": "other"},
            ],
            "return": ["other", "rel"],
        }
    ) == [
        {"other": carol, "rel": bob_to_carol},
        {"other": alice, "rel": alice_to_bob},
    ]

    assert graph.query(
        {
            "match": [
                {"node": "a", "id": alice},
                {"edge": "ab", "type": "KNOWS"},
                {"node": "b"},
                {"edge": "bc", "type": "KNOWS"},
                {"node": "c"},
                {"edge": "ca", "type": "KNOWS"},
                {"node": "a"},
            ],
            "return": ["a", "b", "c"],
        }
    ) == [{"a": alice, "b": bob, "c": carol}]

    assert graph.query(
        {
            "match": [
                {"node": "a", "id": alice},
                {"edge": "e", "type": "KNOWS"},
                {"node": "b", "id": bob},
                {"edge": "e", "type": "KNOWS", "direction": "in"},
                {"node": "a", "id": alice},
            ],
            "return": ["e"],
        }
    ) == [{"e": alice_to_bob}]

    assert graph.query(
        {
            "match": [
                {"node": "a", "id": alice},
                {"edge": "rel", "type": "KNOWS"},
                {"node": "b"},
            ],
        }
    ) == [{"a": alice, "b": bob, "rel": alice_to_bob}]

    invalid_queries = [
        ([], "query spec must be a dict"),
        ({"match": [{"node": "n", "id": alice}], "return": ["missing"]}, "return alias"),
        (
            {"match": [{"node": "same"}, {"edge": "same", "type": "KNOWS"}, {"node": "n"}]},
            'alias "same" cannot refer to both nodes and edges',
        ),
        (
            {"match": [{"node": "n", "where": [{"property": "rank", "op": "in", "value": 1}]}]},
            "op 'in' requires a list value",
        ),
    ]
    for spec, message in invalid_queries:
        with pytest.raises(ValueError, match=message):
            graph.query(spec)  # type: ignore[arg-type]

    strict_queries = [
        ({"match": [{"node": "n"}], "bogus": True}, "unknown field"),
        ({"match": [{"node": "n", "label": ["Person"]}]}, "unknown field"),
        (
            {"match": [{"node": "a"}, {"edge": "e", "edge_type": "KNOWS"}, {"node": "b"}]},
            "unknown field",
        ),
        (
            {
                "match": [
                    {
                        "node": "n",
                        "where": [
                            {
                                "property": "rank",
                                "op": "eq",
                                "value": 1,
                                "extra": True,
                            }
                        ],
                    }
                ]
            },
            "unknown field",
        ),
    ]
    for spec, message in strict_queries:
        with pytest.raises(ValueError, match=message):
            graph.query(spec)

    schema = graph.query_schema()
    assert schema["top_level_fields"] == ["match", "return", "limit"]
    assert schema["node_pattern"]["allowed_fields"] == [
        "node",
        "id",
        "external_id",
        "labels",
        "properties",
        "where",
    ]
    assert schema["edge_pattern"]["allowed_fields"] == [
        "edge",
        "id",
        "type",
        "direction",
        "properties",
        "where",
    ]


def test_sqlite_reopen_preserves_compacted_directional_adjacency(tmp_path: Path) -> None:
    db_path = tmp_path / "directional.db"
    graph = Graph(str(db_path))
    alice = graph.add_node("alice")
    bob = graph.add_node("bob")
    carol = graph.add_node("carol")
    dave = graph.add_node("dave")
    graph.add_edge(alice, bob, "FOLLOWS")
    graph.add_edge(carol, bob, "FOLLOWS")
    graph.add_edge(bob, dave, "LIKES")
    graph.compact()
    del graph

    reopened = Graph(str(db_path))
    assert reopened.neighbors(bob, direction="in", edge_type="FOLLOWS") == [alice, carol]
    assert reopened.neighbors(bob, direction="both") == [dave, alice, carol]
    assert reopened.bfs(bob, direction="in") == [bob, alice, carol]
    assert reopened.query(
        {
            "match": [
                {"node": "target", "id": bob},
                {"edge": "rel", "type": "FOLLOWS", "direction": "in"},
                {"node": "source"},
            ],
            "return": ["source", "rel"],
        }
    ) == [{"rel": 0, "source": alice}, {"rel": 1, "source": carol}]


def test_sqlite_recovers_from_unusable_segment_sidecars(tmp_path: Path) -> None:
    def create_graph(name: str) -> Path:
        db_path = tmp_path / f"{name}.db"
        graph = Graph(str(db_path))
        source = graph.add_node("source")
        target = graph.add_node("target")
        graph.add_edge(source, target, "LINK")
        graph.compact()
        del graph
        return db_path

    bad_manifest = create_graph("bad_manifest")
    manifest_path = Path(f"{bad_manifest}.segments") / "manifest.txt"
    manifest_path.write_text("bad manifest", encoding="utf-8")
    reopened = Graph(str(bad_manifest))
    assert reopened.neighbors(0, edge_type="LINK") == [1]
    assert "version=tonggraph-segment-v1" in manifest_path.read_text(encoding="utf-8")

    missing_segment = create_graph("missing_segment")
    segment_path = Path(f"{missing_segment}.segments") / "segment-v1.bin"
    segment_path.unlink()
    reopened = Graph(str(missing_segment))
    assert reopened.neighbors(0, edge_type="LINK") == [1]
    assert segment_path.exists()

    corrupt_segment = create_graph("corrupt_segment")
    segment_path = Path(f"{corrupt_segment}.segments") / "segment-v1.bin"
    segment_path.write_bytes(b"not a segment")
    reopened = Graph(str(corrupt_segment))
    assert reopened.neighbors(0, edge_type="LINK") == [1]


def test_sqlite_stale_handle_requires_refresh(tmp_path: Path) -> None:
    db_path = tmp_path / "multi_handle.db"
    first = Graph(str(db_path))
    assert first.add_node("a") == 0

    second = Graph(str(db_path))
    assert second.add_node("b") == 1

    with pytest.raises(ValueError, match="call refresh\\(\\) before writing"):
        first.add_node("c")
    assert first.get_node_id("b") is None

    first.refresh()
    assert first.get_node_id("b") == 1
    assert first.add_node("c") == 2

    with pytest.raises(ValueError, match="SQLite-backed"):
        Graph().refresh()


def test_sqlite_counter_only_transaction_invalidates_stale_handles(tmp_path: Path) -> None:
    db_path = tmp_path / "counter_only.db"
    graph = Graph(str(db_path))
    stale = Graph(str(db_path))

    with graph.transaction() as tx:
        tx.run("CREATE (n {external_id: 'transient'})")
        tx.run("MATCH (n {external_id: 'transient'}) DELETE n")

    assert graph.node_count() == 0
    with pytest.raises(ValueError, match="call refresh\\(\\) before writing"):
        stale.add_node("real")

    stale.refresh()
    assert stale.add_node("real") == 1


def test_bulk_ingest_scans_reopen_and_rollback(tmp_path: Path) -> None:
    db_path = tmp_path / "bulk.db"
    graph = Graph(str(db_path))
    node_ids = graph.add_nodes(
        [
            {"external_id": "a", "labels": ["Entity"], "properties": {"rank": 1}},
            {"external_id": "b", "labels": ["Entity"], "properties": {"rank": 2}},
        ]
    )
    assert node_ids == [0, 1]
    edge_ids = graph.add_edges(
        [
            {
                "source": node_ids[0],
                "target": node_ids[1],
                "edge_type": "LINK",
                "properties": {"probability": 0.5},
            }
        ]
    )
    assert edge_ids == [0]
    assert graph.node_ids() == [0, 1]
    assert graph.edge_ids() == [0]
    assert [node.external_id for node in graph.nodes()] == ["a", "b"]
    assert [edge.edge_type for edge in graph.edges()] == ["LINK"]

    with pytest.raises(ValueError, match='external_id "dup" already exists'):
        graph.add_nodes([{"external_id": "dup"}, {"external_id": "dup"}])
    assert graph.node_count() == 2

    with pytest.raises(ValueError, match="node 999 not found"):
        graph.add_edges(
            [
                {"source": node_ids[0], "target": node_ids[1], "edge_type": "LINK"},
                {"source": 999, "target": node_ids[1], "edge_type": "LINK"},
            ]
        )
    assert graph.edge_count() == 1

    snapshot = graph.snapshot()
    assert snapshot.node_ids() == [0, 1]
    assert snapshot.edge_ids() == [0]
    del graph

    reopened = Graph(str(db_path))
    assert reopened.node_ids() == [0, 1]
    assert reopened.edge_ids() == [0]
    assert [node.external_id for node in reopened.nodes()] == ["a", "b"]
    assert [edge.edge_type for edge in reopened.edges()] == ["LINK"]


def test_categorical_belief_propagation_uses_public_python_api_ordering() -> None:
    graph = Graph()
    parent = graph.add_variable("binary", prior={"false": 0.5, "true": 0.5})
    child = graph.add_variable("categorical", states=["sun", "rain", "snow"])
    graph.add_cpd(child, [parent], [0.7, 0.2, 0.1, 0.1, 0.3, 0.6])

    result = graph.belief_propagation(
        [child],
        evidence={parent: "true"},
        damping=0.0,
        tolerance=1e-12,
    )

    assert result["converged"] is True
    assert result["warnings"] == []
    assert result["diagnostics"]["active_variables"] == 2
    assert result["diagnostics"]["active_factors"] == 1
    assert result["beliefs"][child]["sun"] == pytest.approx(0.1)
    assert result["beliefs"][child]["rain"] == pytest.approx(0.3)
    assert result["beliefs"][child]["snow"] == pytest.approx(0.6)
    assert graph.posterior(child) == {"rain": 1 / 3, "snow": 1 / 3, "sun": 1 / 3}

    stopped = graph.belief_propagation(
        [child],
        evidence={parent: "true"},
        damping=0.0,
        max_iters=0,
    )
    assert stopped["converged"] is False
    assert stopped["diagnostics"]["max_iters"] == 0
    assert any("did not converge" in warning for warning in stopped["warnings"])
