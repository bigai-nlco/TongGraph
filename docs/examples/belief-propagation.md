# Belief Propagation

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
