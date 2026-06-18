# Local Probability Transfer

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
