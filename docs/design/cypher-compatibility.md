# Cypher Compatibility

TongGraph exposes an embedded `Graph.cypher()` API as the first compatibility
step toward Neo4j Cypher 25. This is not yet a full Cypher 25 implementation.
The current engine is a deterministic local subset over the existing TongGraph
property graph model.

## Public API

`Graph.cypher(query, parameters=None)` runs one autocommit statement. Write
statements are staged on a snapshot and committed back to the graph only after
the statement succeeds. SQLite-backed graphs persist the staged graph records in
one SQLite transaction.

`Graph.transaction(write=True)` returns a context manager with `run()`,
`commit()`, and `rollback()`. Transaction writes are staged until commit. If the
context exits with an exception, staged writes are discarded.

`GraphSnapshot.cypher(query, parameters=None)` runs read-only Cypher against a
snapshot and rejects write statements.

Cypher results contain:

| Field | Meaning |
|---|---|
| `keys` | Returned column names in order. |
| `records` | Row dictionaries keyed by column name. Node and relationship values are returned as `Node` and `Edge`. |
| `summary` | Statement type, row count, and counters for created/deleted entities, set/removed properties, and added/removed labels. |

## Supported Subset

| Area | Status |
|---|---|
| `MATCH` | Single connected path pattern with node labels, one relationship type, directed or undirected expansion. |
| `OPTIONAL MATCH` | Supported for one pattern; returns one null-like row when no match is found. |
| `WHERE` | `AND`-combined comparisons, `CONTAINS`, `IN`, scalar literals, parameters, variables, and property access. |
| `RETURN` | Variables, properties, `id()`, `elementId()`, `labels()`, `type()`, `startNode()`, `endNode()`, `count(*)`, and `count(var)`. |
| Ordering and paging | `ORDER BY`, `ASC`/`DESC`, `SKIP`, and `LIMIT` with integer literals. |
| `CREATE` | Directed node/relationship path creation with scalar properties. |
| `MERGE` | Exact-pattern match-or-create for the supported pattern subset. |
| `SET` | Assigns scalar properties, merges map literals or parameters with `+=`, and adds node labels. Assigning `null` removes a property. |
| `REMOVE` | Removes node or relationship properties and node labels. `external_id` cannot be removed. |
| `DELETE` | Deletes relationships and nodes without remaining relationships. Multiple bindings are deduplicated. |
| `DETACH DELETE` | Deletes nodes together with their incident relationships. Nodes that own probabilistic variables are protected. |
| `UNION` | Combines supported read queries with identical return keys. |
| Parameters | Scalar, list, map, and null Python values. Only scalar values can be stored as properties. |
| Transactions | Staged embedded transactions for Cypher writes; SQLite commit for graph records is atomic. |

## Not Yet Supported

| Area | Status |
|---|---|
| Multi-clause pipelines | `WITH`, `UNWIND`, subqueries, `CALL`, `FOREACH`, and `NEXT` are not implemented. |
| Advanced paths | Variable-length/quantified paths, shortest path syntax, path variables, and path return values are not implemented. |
| Aggregation | Only whole-result `count(*)` and `count(var)` are implemented. Grouped aggregation is not implemented. |
| Advanced writes | `ON CREATE SET`, `ON MATCH SET`, `DELETE ... RETURN`, mixed update-and-delete statements, and multi-stage write pipelines are not implemented. |
| Schema | Cypher index and constraint DDL are not implemented. Existing TongGraph indexes remain internal. |
| Full value model | Stored properties remain scalar `bool`, `int`, `float`, and `str`; temporal, spatial, vector, byte, and list properties are not implemented. |
| Compatibility surfaces | Bolt, APOC, admin commands, multi-database management, and clustering are out of scope. |

## ACID Scope

For SQLite-backed graphs, supported Cypher write statements and explicit Cypher
transaction commits write graph records to SQLite before publishing them to
in-memory indexes. If persistence fails, the original graph state is left
unchanged. Current guarantees cover embedded local use with TongGraph's existing
single-live-writer stale-handle model. Updates and deletions are persisted as one
change set, and transaction commits reject a stale graph mutation version.

`external_id` may be changed with `SET` or the Python CRUD API, but it remains a
non-empty unique node identity and cannot be removed. Plain node deletion rejects
incident relationships; use `DETACH DELETE` when relationship removal is intended.
Nodes referenced by probabilistic variables through `owner_id` cannot be deleted.

Segment sidecars remain derived traversal caches. They are not the source of
truth and can be rebuilt from SQLite records.
