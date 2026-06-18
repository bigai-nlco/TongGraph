# Property Graph Basics

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
