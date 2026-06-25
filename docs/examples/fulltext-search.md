# Full-Text Search

This example indexes node text, filters results with graph metadata, and shows a
Chinese trigram index.

```python
from tonggraph import Graph


graph = Graph()
guide = graph.add_node(
    "guide",
    labels=["Document"],
    properties={
        "title": "Graph Database Guide",
        "content": "A local embedded graph engine",
        "published": True,
    },
)
graph.add_node(
    "notes",
    labels=["Document"],
    properties={"title": "Database Notes", "content": "Relational storage"},
)

graph.create_fulltext_index(
    "documents",
    target="node",
    properties=["title", "content"],
)

results = graph.search_text(
    "documents",
    "graph database",
    labels=["Document"],
    properties={"published": True},
)
assert [result["id"] for result in results] == [guide]

chinese = graph.add_node("chinese", properties={"content": "本地图数据库全文检索"})
graph.create_fulltext_index(
    "chinese-content",
    target="node",
    properties=["content"],
    tokenizer="trigram",
)
assert graph.search_text("chinese-content", "图数据")[0]["id"] == chinese
```

Use a returned ID with normal graph APIs:

```python
node = graph.get_node(results[0]["id"])
subgraph = graph.subgraph([node.id])
```
