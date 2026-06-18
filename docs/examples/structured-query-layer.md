# Structured Query Layer

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
