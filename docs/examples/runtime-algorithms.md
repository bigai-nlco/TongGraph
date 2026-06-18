# Runtime Algorithms And Batch Compute

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
