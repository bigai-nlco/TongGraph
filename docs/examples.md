# Examples

These examples were run against the current Python extension with `uv run
python`. Each example prints structured output so behavior is easy to compare
while learning the API.

## Property Graph Basics

This example builds a tiny property graph and shows label indexes, outgoing
neighbor lookup, two-hop traversal, frontier extraction, and probability
transfer over edge properties.

=== "Python"

    ```python
    import json
    from tonggraph import Graph

    graph = Graph()
    alice = graph.add_node(
        "alice",
        labels=["Person"],
        properties={"name": "Alice", "active": True},
    )
    bob = graph.add_node("bob", labels=["Person"], properties={"name": "Bob"})
    claim = graph.add_node("claim:1", labels=["Claim"], properties={"topic": "graph"})

    graph.add_edge(alice, bob, "KNOWS", properties={"probability": 0.8, "weight": 2.0})
    graph.add_edge(bob, claim, "SUPPORTS", properties={"probability": 0.5, "weight": 1.0})

    result = {
        "node_count": graph.node_count(),
        "person_nodes": graph.nodes_with_label("Person"),
        "alice_neighbors": graph.neighbors(alice),
        "two_hop_from_alice": graph.k_hop(alice, 2),
        "support_frontier": graph.frontier([alice], 2),
        "probability_transfer": {
            str(k): v for k, v in sorted(graph.propagate({alice: 1.0}, 2).items())
        },
    }

    print(json.dumps(result, indent=2, sort_keys=True))
    ```

Output:

```json
{
  "alice_neighbors": [
    1
  ],
  "node_count": 3,
  "person_nodes": [
    0,
    1
  ],
  "probability_transfer": {
    "0": 1.0,
    "1": 0.8,
    "2": 0.4
  },
  "support_frontier": [
    2
  ],
  "two_hop_from_alice": [
    1,
    2
  ]
}
```

## Structured Query Layer

This example matches a typed path, filters node and edge properties, and returns
selected aliases. It also shows `query_nl` with a fake compiler so the natural
language layer stays provider-neutral.

=== "Python"

    ```python
    import json
    from tonggraph import Graph, query_nl

    graph = Graph()
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

    graph.add_edge(alice, bob, "KNOWS", properties={"note": "team alpha", "weight": 0.8})
    graph.add_edge(bob, carol, "KNOWS", properties={"note": "team beta", "weight": 0.6})
    graph.add_edge(carol, alice, "KNOWS", properties={"note": "loop"})

    spec = {
        "match": [
            {
                "node": "a",
                "labels": ["Person"],
                "where": [
                    {"property": "rank", "op": "gte", "value": 2},
                    {"property": "group", "op": "in", "value": ["ai", "research"]},
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

    def compiler(question, schema):
        assert schema["name"] == "tonggraph_query_dsl_v0"
        assert question == "Which active people are known by team members?"
        return spec

    result = {
        "structured": graph.query(spec),
        "snapshot": graph.snapshot().query(spec),
        "natural_language": query_nl(
            graph,
            "Which active people are known by team members?",
            compiler,
        ),
    }

    print(json.dumps(result, indent=2, sort_keys=True))
    ```

Output:

```json
{
  "natural_language": [
    {
      "a": 0,
      "b": 1,
      "rel": 0
    }
  ],
  "snapshot": [
    {
      "a": 0,
      "b": 1,
      "rel": 0
    }
  ],
  "structured": [
    {
      "a": 0,
      "b": 1,
      "rel": 0
    }
  ]
}
```

## Runtime Algorithms And Batch Compute

This example shows breadth-first search, weighted shortest path, weakly
connected components, PageRank, seeded random walk, and `compute_batch` over the
same graph.

=== "Python"

    ```python
    import json
    from tonggraph import Graph

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

    result = {
        "bfs_depth_1": graph.bfs(a, max_depth=1),
        "weighted_shortest_path": graph.shortest_path(a, b, weight_property="weight"),
        "connected_components": graph.connected_components(),
        "pagerank": {
            str(k): round(v, 6)
            for k, v in sorted(graph.pagerank(iterations=25, tolerance=1e-12).items())
        },
        "random_walk_seed_7": graph.random_walk(a, 4, seed=7),
        "batch": graph.compute_batch(
            [
                {"op": "bfs", "start": a, "max_depth": 1},
                {
                    "op": "shortest_path",
                    "start": a,
                    "target": b,
                    "weight_property": "weight",
                },
            ]
        ),
    }

    print(json.dumps(result, indent=2, sort_keys=True))
    ```

Output:

```json
{
  "batch": [
    [
      0,
      1,
      2
    ],
    {
      "distance": 1.5,
      "nodes": [
        0,
        2,
        1
      ]
    }
  ],
  "bfs_depth_1": [
    0,
    1,
    2
  ],
  "connected_components": [
    [
      0,
      1,
      2,
      3
    ],
    [
      4
    ]
  ],
  "pagerank": {
    "0": 0.107503,
    "1": 0.283405,
    "2": 0.153192,
    "3": 0.348397,
    "4": 0.107503
  },
  "random_walk_seed_7": [
    0,
    1,
    3
  ],
  "weighted_shortest_path": {
    "distance": 1.5,
    "nodes": [
      0,
      2,
      1
    ]
  }
}
```

## Local Probability Transfer

This example compares global propagation with radius-limited local propagation.
The global two-step run reaches node `2`; the local run with `radius=1` keeps
the transfer inside the one-hop active neighborhood.

=== "Python"

    ```python
    import json
    from tonggraph import Graph

    graph = Graph()
    a = graph.add_node()
    b = graph.add_node()
    c = graph.add_node()

    graph.add_edge(a, b, "P", properties={"probability": "0.5"})
    graph.add_edge(b, c, "P", properties={"probability": "0.25"})

    result = {
        "global_two_step": {
            str(k): v for k, v in sorted(graph.propagate({a: 1.0}, 2).items())
        },
        "local_radius_1": {
            str(k): v
            for k, v in sorted(
                graph.local_propagate({a: 1.0}, radius=1, edge_type="P").items()
            )
        },
    }

    print(json.dumps(result, indent=2, sort_keys=True))
    ```

Output:

```json
{
  "global_two_step": {
    "0": 1.0,
    "1": 0.5,
    "2": 0.125
  },
  "local_radius_1": {
    "0": 1.0,
    "1": 0.5
  }
}
```

## SQLite Reopen

This example writes graph records to SQLite, compacts adjacency data into a
segment sidecar, reopens the database, and checks that indexes and snapshots
still behave as expected.

=== "Python"

    ```python
    import json
    from pathlib import Path
    from tempfile import TemporaryDirectory
    from tonggraph import Graph

    with TemporaryDirectory() as tmp:
        db_path = Path(tmp) / "tonggraph.db"
        graph = Graph(str(db_path))
        source = graph.add_node("source", labels=["Entity"], properties={"name": "A"})
        target = graph.add_node("target", labels=["Entity"], properties={"name": "B"})
        graph.add_edge(source, target, "LINKS", properties={"probability": 0.75})

        snapshot = graph.snapshot()
        graph.compact()
        del graph

        reopened = Graph(str(db_path))
        result = {
            "source_id": source,
            "snapshot_node_count": snapshot.node_count(),
            "reopened_counts": {
                "nodes": reopened.node_count(),
                "edges": reopened.edge_count(),
            },
            "entity_nodes": reopened.nodes_with_label("Entity"),
            "links_neighbors": reopened.neighbors(source, edge_type="LINKS"),
            "segment_manifest_exists": Path(f"{db_path}.segments/manifest.txt").exists(),
        }

    print(json.dumps(result, indent=2, sort_keys=True))
    ```

Output:

```json
{
  "entity_nodes": [
    0,
    1
  ],
  "links_neighbors": [
    1
  ],
  "reopened_counts": {
    "edges": 1,
    "nodes": 2
  },
  "segment_manifest_exists": true,
  "snapshot_node_count": 2,
  "source_id": 0
}
```

## Belief Propagation

This example creates two binary variables, connects them with a CPD, compiles a
radius-limited active inference problem, applies runtime evidence, and persists
the resulting posterior and trace.

=== "Python"

    ```python
    import json
    from tonggraph import Graph

    graph = Graph()
    source = graph.add_node("source")
    target = graph.add_node("target")
    graph.add_edge(source, target, "LINK")

    parent = graph.add_variable("binary", owner_id=source, prior={"p": 0.6})
    child = graph.add_variable("binary", owner_id=target)
    factor = graph.add_cpd(child, [parent], [0.9, 0.1, 0.2, 0.8])

    active = graph.compile_active_subgraph([child], evidence={parent: "true"}, radius=1)
    result = graph.belief_propagation(
        [child],
        evidence={parent: "true"},
        tolerance=1e-12,
        damping=0.0,
        persist=True,
    )

    output = {
        "factor_id": factor,
        "active": active,
        "beliefs": {str(k): v for k, v in result["beliefs"].items()},
        "posterior": graph.posterior(child),
        "trace_id": result["trace_id"],
        "trace_count": graph.trace_count(),
    }

    print(json.dumps(output, indent=2, sort_keys=True))
    ```

Output:

```json
{
  "active": {
    "boundary_variables": [],
    "factors": [
      0
    ],
    "graph_nodes": [
      0,
      1
    ],
    "truncated": false,
    "variables": [
      0,
      1
    ]
  },
  "beliefs": {
    "1": {
      "false": 0.2,
      "true": 0.8
    }
  },
  "factor_id": 0,
  "posterior": {
    "false": 0.2,
    "true": 0.8
  },
  "trace_count": 1,
  "trace_id": 0
}
```
