# Full-Text Search

TongGraph provides explicit named full-text indexes for node or edge string
properties. Full-text search is a separate retrieval surface from the structured
query DSL and embedded Cypher subset. Search results return graph entity IDs that
can be passed to record lookup, traversal, or subgraph APIs.

## Create And Inspect Indexes

```python
from tonggraph import Graph


graph = Graph("documents.db")
graph.create_fulltext_index(
    "documents",
    target="node",
    properties=["title", "content"],
    tokenizer="unicode61",
)

print(graph.fulltext_indexes())
```

An index has one target (`node` or `edge`), an ordered non-empty list of property
names, and one tokenizer. Only string property values are indexed. Labels, edge
types, and `external_id` remain exact lookup or filtering fields rather than
indexed text.

Index names are unique. `drop_fulltext_index(name)` removes a definition and its
derived index rows. `rebuild_fulltext_index(name=None)` rebuilds one index or all
indexes from the property graph source of truth.

## Search

```python
results = graph.search_text(
    "documents",
    "graph database",
    mode="all",
    labels=["Document"],
    properties={"published": True},
    limit=20,
    offset=0,
)
```

Each result is a dictionary:

```python
{
    "kind": "node",
    "id": 42,
    "score": 0.9,
    "matched_fields": ["title", "content"],
}
```

Results are ordered by descending score and then ascending internal ID. Scores
are deterministic within a result set and are calculated by the shared Rust
matcher, so in-memory graphs, SQLite graphs, and snapshots use the same final
filtering and ordering.

Node indexes accept `labels` and exact scalar `properties` filters. Edge indexes
accept `edge_type` and exact scalar `properties` filters. Passing a filter for
the wrong entity kind raises an error.

## Query Modes

| Mode | Meaning |
|---|---|
| `all` | Every normalized query term or fragment must occur. |
| `any` | At least one normalized query term or fragment must occur. |
| `phrase` | The normalized phrase must occur contiguously in one indexed field. |
| `prefix` | Every query term must prefix a token; supported only by `unicode61`. |

Queries are parsed as data and do not expose raw FTS5 syntax.

## Tokenizers

`unicode61` is the default. It provides Unicode-aware token lookup and is the
recommended choice for English and whitespace-separated text.

`trigram` provides substring lookup and is useful for Chinese text or other
content without word separators. Every trigram query fragment must contain at
least three Unicode characters. Prefix mode is not available for trigram
indexes.

## Storage And Consistency

In-memory graphs evaluate named indexes through deterministic scans. SQLite-backed
graphs persist index definitions and use SQLite FTS5 for candidate recall. The
Rust matcher then applies the same final matching, filters, scores, and ordering
used by in-memory graphs.

Creating an index backfills existing records. Node and edge creates, updates,
string-to-non-string property changes, and deletes synchronize FTS rows in the
same SQLite transaction as the graph mutation. On reopen, TongGraph recreates
missing FTS tables and rebuilds all persisted index definitions. The graph
records remain the source of truth.

Snapshots copy index definitions and graph records, then search their frozen
state without a persistence handle. Extracted subgraphs also retain the parent
index definitions and search only entities present in that subgraph.

!!! note "Current boundary"
    Full-text search is exposed through `search_text()` rather than through the
    structured query DSL or Cypher. Highlighting, snippets, spelling correction,
    synonyms, field weights, and custom tokenizers are not implemented.
