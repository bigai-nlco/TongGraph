# Query Layer

TongGraph adds a small structured query layer for local graph retrieval.
It is intentionally not a Cypher implementation. The first query surface is a
Python dictionary DSL that maps directly to a single connected path pattern and
executes inside the Rust core.

The query layer is useful when a caller needs more than one adjacency lookup but
does not need a full graph database language:

- find paths with node labels and edge types
- filter node and edge properties
- return selected node or edge aliases
- run the same read-only query against `Graph` or `GraphSnapshot`
- compile natural-language questions into the structured DSL with a
  caller-supplied LLM function

## Structured DSL

A query uses a `match` list that alternates node, edge, and node pattern
dictionaries.

```python
spec = {
    "match": [
        {
            "node": "a",
            "labels": ["Person"],
            "where": [{"property": "rank", "op": "gte", "value": 2}],
        },
        {
            "edge": "rel",
            "type": "KNOWS",
            "direction": "out",
            "where": [{"property": "note", "op": "contains", "value": "team"}],
        },
        {
            "node": "b",
            "labels": ["Person"],
            "properties": {"active": True},
        },
    ],
    "return": ["a", "rel", "b"],
    "limit": 100,
}

rows = graph.query(spec)
```

Rows are plain alias-to-ID dictionaries:

```python
[{"a": 0, "rel": 0, "b": 1}]
```

The IDs are TongGraph internal node or edge IDs. Fetch records with
`get_node()` or `get_edge()` when the caller needs labels, external IDs, or
properties.

## Pattern Elements

Node patterns support:

| Field | Meaning |
|---|---|
| `node` | Required alias string. |
| `id` | Optional internal node ID. |
| `external_id` | Optional application-facing node ID. |
| `labels` | Optional list of labels that must all be present. |
| `properties` | Optional exact-match property dictionary. |
| `where` | Optional list of property filters. |

Edge patterns support:

| Field | Meaning |
|---|---|
| `edge` | Optional alias string. Omit it when the edge is only used for expansion. |
| `id` | Optional internal edge ID. |
| `type` | Optional edge type. |
| `direction` | `out`, `in`, or `both`, relative to the surrounding node patterns. Defaults to `out`. |
| `properties` | Optional exact-match property dictionary. |
| `where` | Optional list of property filters. |

`return` is optional. When omitted, TongGraph returns every declared alias in
pattern order. `limit` is optional and stops execution after that many rows.

Query parsing is strict. Unknown top-level fields, node fields, edge fields, or
filter fields raise an error instead of being ignored. This is intentional so
LLM-compiled query specs fail fast when they hallucinate names such as `label`
instead of `labels` or `edge_type` instead of `type`.

## Filters

`properties` is shorthand for equality filters:

```python
{"node": "p", "properties": {"active": True}}
```

is equivalent to:

```python
{
    "node": "p",
    "where": [{"property": "active", "op": "eq", "value": True}],
}
```

`where` supports:

| Operator | Value shape | Meaning |
|---|---|---|
| `eq` | scalar | Property value equals `value`. |
| `ne` | scalar | Property exists and is not equal to `value`. |
| `lt` | scalar number | Numeric property is less than `value`. |
| `lte` | scalar number | Numeric property is less than or equal to `value`. |
| `gt` | scalar number | Numeric property is greater than `value`. |
| `gte` | scalar number | Numeric property is greater than or equal to `value`. |
| `in` | list of scalars | Property equals one value in the list. |
| `contains` | string | String property contains the given substring, case-sensitively. |

Missing properties do not match any filter, including `ne`.

## Planning And Execution

The planner chooses a node anchor using the most selective information available:
internal node ID, external ID, labels, and equality property indexes. It then
expands left and right through TongGraph adjacency structures and filters each
candidate edge and node.

Repeated aliases act as equality constraints. This can express simple cycles or
require two pattern positions to bind to the same node or edge.

```python
cycle = {
    "match": [
        {"node": "a", "external_id": "alice"},
        {"edge": "ab", "type": "KNOWS"},
        {"node": "b"},
        {"edge": "ba", "type": "KNOWS"},
        {"node": "a"},
    ],
    "return": ["a", "b"],
}
```

The engine covers one connected path pattern. It does not yet implement
multi-pattern joins, optional matches, aggregation, sorting, or vector retrieval.
Full-text retrieval is available separately through named indexes and
`Graph.search_text()`; it is not embedded in this DSL. Cypher compatibility is
documented separately.

## Natural-Language Compilation Hook

TongGraph includes a provider-neutral helper for natural-language query
workflows:

```python
from tonggraph import query_nl


def compiler(question, schema):
    # Call any LLM or local compiler here and return a query spec.
    return {
        "match": [
            {"node": "a", "external_id": "alice"},
            {"edge": "rel", "type": "KNOWS"},
            {"node": "b"},
        ],
        "return": ["a", "rel", "b"],
    }


rows = query_nl(graph, "Who does Alice know?", compiler)
```

The helper passes `(question, schema)` to the supplied compiler, checks that the
compiler returned a mapping, then calls `graph.query()`. TongGraph does not
ship an LLM provider, read API keys, or make network calls for this layer.

Use `query_dsl_schema()` or `graph.query_schema()` to inspect the compact schema
object that should be passed to a compiler.
