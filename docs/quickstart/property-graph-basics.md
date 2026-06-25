# Property Graph Basics

Use labels for node categories, edge types for relationships, and scalar
properties for attributes that should be indexed or returned later.

```python
from tonggraph import Graph

graph = Graph()

alice = graph.add_node(
    "person:alice",
    labels=["Person"],
    properties={"name": "Alice", "role": "researcher", "active": True},
)
bob = graph.add_node(
    "person:bob",
    labels=["Person"],
    properties={"name": "Bob", "role": "engineer", "active": True},
)
paper = graph.add_node(
    "paper:tonggraph",
    labels=["Paper"],
    properties={"title": "TongGraph Notes"},
)

graph.add_edge(alice, bob, "KNOWS", properties={"weight": 0.8})
graph.add_edge(bob, paper, "AUTHORED", properties={"year": 2026})

print(graph.nodes_with_label("Person"))
print(graph.nodes_with_property("role", "engineer"))
print(graph.neighbors(alice, edge_type="KNOWS"))
print(graph.k_hop(alice, 2))
```

Use `Graph.snapshot()` when you need a read-only point-in-time copy:

```python
snapshot = graph.snapshot()
graph.add_node("person:carol", labels=["Person"])

assert snapshot.node_count() == 3
assert graph.node_count() == 4
```
