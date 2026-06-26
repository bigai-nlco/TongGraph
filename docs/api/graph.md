# Graph

`Graph` is the mutable embedded graph database. `Graph()` creates an in-memory
graph; `Graph(path)` or `Graph.open(path)` opens a SQLite-backed graph.

## Method Guides

| Methods | Design reference |
|---|---|
| `neighbors`, `k_hop`, `frontier` | [Neighbor expansion and k-hop traversal](../design/algorithms.md#neighbor-expansion) |
| `bfs` | [Breadth-first search](../design/algorithms.md#breadth-first-search) |
| `shortest_path` | [Weighted shortest path](../design/algorithms.md#weighted-shortest-path) |
| `connected_components` | [Connected components](../design/algorithms.md#connected-components) |
| `pagerank` | [PageRank](../design/algorithms.md#pagerank) |
| `random_walk` | [Random walk](../design/algorithms.md#random-walk) |
| `propagate`, `local_propagate` | [Sparse probability transfer](../design/algorithms.md#sparse-probability-transfer) |
| `compute_batch` | [Batch compute](../design/algorithms.md#batch-compute) |
| `query`, `query_schema` | [Structured path-pattern query DSL](../design/query-layer.md) |
| `schema`, `stats` | Graph introspection for Text2Query, GraphRAG planning, and local diagnostics |
| `retrieve_context` | [GraphRAG retrieval](../quickstart/graphrag-retrieval.md) |
| `import_nodes_csv`, `import_edges_csv`, `import_nodes_jsonl`, `import_edges_jsonl`, `export_nodes_jsonl`, `export_edges_jsonl`, `export_query_rows_jsonl` | Local CSV/JSONL import and export helpers |
| `create_fulltext_index`, `drop_fulltext_index`, `fulltext_indexes`, `rebuild_fulltext_index`, `search_text` | [Full-text search](../design/fulltext-search.md) |
| `create_vector_index`, `drop_vector_index`, `vector_indexes`, `upsert_vector`, `upsert_vectors`, `get_vector`, `delete_vector`, `delete_vectors`, `search_vector` | [Vector search](../design/vector-search.md) |
| `update_node`, `update_edge`, `delete_node`, `delete_edge` | [Persistence and graph mutations](../design/persistence.md) |
| `cypher`, `transaction` | [Cypher compatibility](../design/cypher-compatibility.md) |
| `add_variable`, `add_factor_table`, `add_cpd`, `add_evidence`, `compile_active_subgraph`, `belief_propagation`, `posterior` | [Belief propagation](../design/belief-propagation.md) |
| `compact`, `open` | [Persistence](../design/persistence.md) |

## Reference

::: tonggraph.Graph
    options:
      heading_level: 3
