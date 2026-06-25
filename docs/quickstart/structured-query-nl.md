# Structured Query From Natural Language Compiler

TongGraph keeps natural-language query compilation provider-neutral. You pass a
callable that turns `(question, schema)` into the structured query DSL; TongGraph
executes the returned DSL locally.

```python
from typing import Any, Mapping

from tonggraph import Graph, query_nl

graph = Graph()
alice = graph.add_node(
    "alice",
    labels=["Person"],
    properties={"name": "Alice", "active": True},
)
bob = graph.add_node(
    "bob",
    labels=["Person"],
    properties={"name": "Bob", "active": True},
)
graph.add_edge(alice, bob, "KNOWS", properties={"note": "team"})


def compiler(question: str, schema: Mapping[str, Any]) -> Mapping[str, Any]:
    assert schema["name"] == "tonggraph_query_dsl_v0"
    if "Alice" not in question:
        raise ValueError("demo compiler only handles Alice questions")
    return {
        "match": [
            {"node": "a", "labels": ["Person"], "properties": {"name": "Alice"}},
            {"edge": "rel", "type": "KNOWS", "direction": "out"},
            {"node": "b", "labels": ["Person"], "properties": {"active": True}},
        ],
        "return": ["a", "rel", "b"],
        "limit": 10,
    }


rows = query_nl(graph, "Who does Alice know?", compiler)
print(rows)
```

Production systems can replace the demo compiler with an LLM, rules engine, or
query-template service. Tests should use fake compiler callables so they do not
need network access or API keys.
