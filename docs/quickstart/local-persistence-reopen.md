# Local Persistence And Reopen

Pass a SQLite path to `Graph` for local persistence. Call `compact()` when you
want TongGraph to write compute-oriented segment files next to the database.

```python
from pathlib import Path

from tonggraph import Graph

db_path = Path("local-memory.db")

graph = Graph(str(db_path))
source = graph.add_node("source", labels=["Entity"], properties={"name": "A"})
target = graph.add_node("target", labels=["Entity"], properties={"name": "B"})
graph.add_edge(source, target, "LINKS", properties={"weight": 0.75})
graph.compact()

del graph

reopened = Graph(str(db_path))
assert reopened.node_count() == 2
assert reopened.edge_count() == 1
assert reopened.neighbors(source, edge_type="LINKS") == [target]
assert reopened.get_node(source).properties["name"] == "A"
```

The SQLite file stores durable metadata, properties, operation logs, factors,
evidence, and traces. Segment files store compacted traversal data.
