# Vector Search

This example stores caller-provided embeddings and combines similarity with
graph metadata filters.

```python
from tonggraph import Graph


graph = Graph()
guide = graph.add_node(
    "guide",
    labels=["Document"],
    properties={"published": True},
)
notes = graph.add_node(
    "notes",
    labels=["Document"],
    properties={"published": False},
)

graph.create_vector_index(
    "documents",
    target="node",
    dimensions=3,
    metric="cosine",
    model="demo-embedding",
)
graph.upsert_vectors(
    "documents",
    {
        guide: [1.0, 0.0, 0.0],
        notes: [0.5, 0.5, 0.0],
    },
)

results = graph.search_vector(
    "documents",
    [1.0, 0.0, 0.0],
    labels=["Document"],
    properties={"published": True},
)
assert results == [{"kind": "node", "id": guide, "score": 1.0}]
```

Snapshots freeze vector state just like graph records:

```python
snapshot = graph.snapshot()
graph.upsert_vector("documents", guide, [0.0, 1.0, 0.0])
assert snapshot.get_vector("documents", guide) == [1.0, 0.0, 0.0]
```

Returned IDs can be passed to lookup, traversal, and subgraph methods.
