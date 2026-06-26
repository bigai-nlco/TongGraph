# Vector Search

TongGraph provides explicit named vector indexes for node or edge embeddings.
Vectors are stored separately from scalar graph properties and are searched
through a dedicated API rather than through Cypher or the structured query DSL.
The caller supplies embeddings; TongGraph does not call a model provider or make
network requests.

## Create And Inspect Indexes

```python
from tonggraph import Graph


graph = Graph("documents.db")
graph.create_vector_index(
    "documents",
    target="node",
    dimensions=384,
    metric="cosine",
    model="example-embedding",
    model_version="1",
)
print(graph.vector_indexes())
```

An index fixes its target (`node` or `edge`), dimensions, and metric. Optional
model metadata documents an embedding family without binding TongGraph to a
provider. Definitions are immutable; replace one by dropping and recreating it.

## Write And Read Vectors

```python
graph.upsert_vector("documents", document_id, embedding)
graph.upsert_vectors(
    "documents",
    {first_id: first_embedding, second_id: second_embedding},
)
vector = graph.get_vector("documents", document_id)
graph.delete_vector("documents", document_id)
```

A batch is atomic: all entity IDs, dimensions, and values are validated before
anything is written. Upsert replaces an existing vector. Delete is idempotent as
long as the referenced entity still exists. Deleting a graph entity removes its
vectors from every compatible index.

Vectors are finite float32 values. Cosine indexes reject zero vectors. Vector
writes are explicit: changing labels or properties does not regenerate an
embedding.

## Search

```python
results = graph.search_vector(
    "documents",
    query_embedding,
    labels=["Document"],
    properties={"published": True},
    min_score=0.7,
    limit=20,
    offset=0,
)
```

Each result contains `kind`, `id`, and `score`. Results are ordered by descending
score and then ascending entity ID. Node indexes accept all-label and exact scalar
property filters. Edge indexes accept one `edge_type` and exact scalar property
filters.

Use `search_vectors()` when many queries share the same index and filters:

```python
batch_results = graph.search_vectors(
    "documents",
    [first_query_embedding, second_query_embedding],
    labels=["Document"],
    limit=10,
)
```

It returns one result list per query vector in input order. The implementation
still uses deterministic exact search; batching keeps repeated query loops inside
the Rust core and avoids per-query Python boundary overhead.

Supported metrics are:

| Metric | Score |
|---|---|
| `cosine` | Cosine similarity. |
| `dot` | Dot product. |
| `euclidean` | `1 / (1 + distance)`. |

All metrics therefore use higher-is-better ordering. `min_score` is applied
before pagination.

## Storage And Snapshots

In-memory and SQLite graphs use the same Rust exact-scan implementation, giving
deterministic results across backends. SQLite persists index definitions and
little-endian float32 BLOBs. Opening a graph validates dimensions, finite values,
entity references, and cosine non-zero constraints; malformed persisted vectors
fail explicitly instead of being ignored.

Snapshots copy definitions and vectors at creation time. Extracted subgraphs keep
all definitions but only retain vectors for nodes and edges present in the
subgraph.

!!! note "Current boundary"
    The first implementation is exact search, not ANN/HNSW. It does not provide
    quantization, GPU search, automatic embedding generation, hybrid ranking, or
    Cypher/structured-DSL integration.
