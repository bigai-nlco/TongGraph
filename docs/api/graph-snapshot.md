# GraphSnapshot

`GraphSnapshot` is a read-only copy of graph state. It supports lookup and
compute methods without mutating the original graph or carrying a persistence
handle.

## Method Guides

| Methods | Design reference |
|---|---|
| `neighbors`, `k_hop`, `frontier` | [Neighbor expansion and k-hop traversal](../design/algorithms.md#neighbor-expansion) |
| `bfs` | [Breadth-first search](../design/algorithms.md#breadth-first-search) |
| `shortest_path` | [Weighted shortest path](../design/algorithms.md#weighted-shortest-path) |
| `connected_components` | [Connected components](../design/algorithms.md#connected-components) |
| `pagerank` | [PageRank](../design/algorithms.md#pagerank) |
| `random_walk` | [Random walk](../design/algorithms.md#random-walk) |
| `subgraph`, `compute_batch` | [Batch compute and snapshots](../design/algorithms.md#batch-compute) |
| `query`, `query_schema` | [Structured path-pattern query DSL](../design/query-layer.md) |
| `schema`, `stats` | Graph introspection for frozen views |
| `fulltext_indexes`, `search_text` | [Full-text search](../design/fulltext-search.md) |
| `vector_indexes`, `get_vector`, `search_vector`, `search_vectors` | [Vector search](../design/vector-search.md) |
| `cypher` | [Cypher compatibility](../design/cypher-compatibility.md) |

## Reference

::: tonggraph.GraphSnapshot
    options:
      heading_level: 3
