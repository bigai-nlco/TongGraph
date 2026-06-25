# GraphRAG Retrieval

TongGraph does not need to own vector search. A retriever can produce candidate
node IDs, then TongGraph expands and scores the local graph neighborhood.

```python
from tonggraph import Graph

graph = Graph()

query = graph.add_node("query:graphrag", labels=["Query"])
paper = graph.add_node(
    "doc:paper",
    labels=["Document"],
    properties={"title": "GraphRAG systems"},
)
chunk = graph.add_node(
    "chunk:paper:1",
    labels=["Chunk"],
    properties={"text": "Graph retrieval improves grounded generation."},
)
entity = graph.add_node(
    "entity:graph",
    labels=["Entity"],
    properties={"name": "graph retrieval"},
)

graph.add_edge(paper, chunk, "HAS_CHUNK")
graph.add_edge(chunk, entity, "MENTIONS")
graph.add_edge(query, entity, "SEEKS", properties={"weight": 0.9})

vector_candidates = [(chunk, 0.82), (entity, 0.76)]

context = []
seen = set()
for node_id, score in vector_candidates:
    for expanded_id in [node_id, *graph.k_hop(node_id, 1, direction="both")]:
        if expanded_id in seen:
            continue
        seen.add(expanded_id)
        node = graph.get_node(expanded_id)
        context.append(
            {
                "id": node.id,
                "external_id": node.external_id,
                "labels": node.labels,
                "score": score,
                "properties": node.properties,
            }
        )

for row in context:
    print(row)
```

This pattern keeps vector retrieval, full-text retrieval, and graph expansion as
replaceable pieces. TongGraph owns the local graph structure and deterministic
expansion.
