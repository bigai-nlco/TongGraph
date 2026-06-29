# Changelog

## Unreleased

### Added

- Adds named full-text indexes for node and edge properties, including Unicode
  word search, trigram substring search, filters, persistence, and snapshot reads.
- Adds named vector indexes for caller-provided node and edge embeddings with
  deterministic cosine, dot-product, and Euclidean exact search.
- Adds `Graph.schema()` / `Graph.stats()` and snapshot equivalents for
  Text2Query, GraphRAG planning, and local diagnostics.
- Adds `profile=True` for structured queries and embedded Cypher results.
- Adds `Graph.retrieve_context()` for full-text/vector candidate retrieval plus
  local graph expansion and ranking.
- Adds CSV/JSONL import helpers, JSONL export helpers, and a JSON benchmark
  runner under `tests/benchmark`.
- Adds the optional `tonggraph[server]` local HTTP server with token auth,
  graph-level ACLs, admin graph creation, persisted server state, and core
  storage/retrieval/query endpoints.
- Extends TongGraph Server with traversal, runtime algorithm, `subgraph()`,
  `compute_batch()`, batch vector search, and TTL-bound read-only snapshot HTTP
  endpoints.
- Adds a synchronous `TongGraphClient` for the local server using Python standard
  library HTTP APIs and JSON-compatible return values.
- Adds TongGraph Server operations support with request logging, JSON metrics,
  elapsed-time headers, request timeout errors, and graph lifecycle summaries.
- Adds TongGraph Server inference endpoints and Python client wrappers for
  probability transfer, variables, factors, evidence, traces, active subgraphs,
  and belief propagation.
- Adds an exact vector search benchmark for embedded and server APIs with 10k and
  100k local scale guidance.

### Changed

- Completes the embedded Cypher CRUD subset with `SET`, `REMOVE`, `DELETE`,
  `DETACH DELETE`, direct Python graph mutation methods, and transactional
  stale-handle protection for consumed graph IDs.
- Expands embedded Cypher reads with comma-separated multi-pattern `MATCH`.
- Rebuilds the local PyO3 extension automatically before pytest when source
  files are newer than the checked-in development extension artifact.

## 0.1.0

TongGraph 0.1.0 is the first self-contained release of the Rust-core graph
engine and Python package. It focuses on local graph context, memory, sparse
graph compute, and explicit finite-discrete belief propagation.

### Rust Core And Python Package

- Exposes the Rust core through a Python-first PyO3 package.
- Provides in-memory graphs and SQLite-backed graphs through the same `Graph`
  API.
- Exposes read-only `GraphSnapshot` views for stable retrieval and compute.
- Ships typed Python records for nodes, edges, variables, factors, evidence,
  traces, Cypher results, and transactions.

### Property Graph Model And Persistence

- Supports directed typed edges, labels, external IDs, and scalar node and edge
  properties.
- Maintains label, edge-type, and property indexes for common lookup paths.
- Persists graph records, properties, operation logs, variables, factors,
  evidence, traces, and posteriors in SQLite.
- Stores compacted outgoing and incoming adjacency segments in local sidecar
  files and rebuilds them when needed.
- Provides bulk append APIs with `add_nodes()` and `add_edges()`.
- Provides ordered scan APIs with `node_ids()`, `edge_ids()`, `nodes()`, and
  `edges()`.
- Detects stale SQLite handles and exposes `refresh()` for reloading after
  another handle writes.

### Traversal, Algorithms, And Batch Compute

- Provides `neighbors()`, `k_hop()`, and `frontier()` for typed directional
  traversal.
- Adds BFS, weighted shortest path, connected components, PageRank, random walk,
  and induced subgraph extraction.
- Adds `compute_batch()` for running multiple compute jobs in one API call.
- Keeps traversal and algorithm execution in Rust-owned adjacency structures.

### Structured Query DSL And Natural-Language Hook

- Adds `Graph.query()` and `GraphSnapshot.query()` for connected path-pattern
  matching.
- Supports node labels, external IDs, edge types, direction filters, property
  filters, repeated aliases, return projection, and row limits.
- Rejects unknown query fields and invalid pattern shapes.
- Exposes `query_dsl_schema()` and `query_nl()` so applications can compile
  natural language into the structured DSL with their own provider-neutral
  compiler.

### Embedded Cypher Compatibility And Transactions

- Adds `Graph.cypher()` for an embedded Cypher compatibility subset.
- Supports selected `MATCH`, `CREATE`, `MERGE`, `WHERE`, `RETURN`, `ORDER BY`,
  `LIMIT`, parameters, and result records.
- Supports read-only Cypher execution on snapshots.
- Adds staged local transactions through `Graph.transaction()`, with commit and
  rollback behavior.

### Sparse Probability Transfer

- Adds weighted probability transfer over sparse graph edges with
  `propagate()`.
- Adds radius-limited active-neighborhood propagation with `local_propagate()`.
- Supports damping, custom edge-property weights, and edge-type filtering.

### Finite Discrete Belief Propagation

- Adds binary and categorical variables with ordered states.
- Adds factor tables, CPDs, evidence records, posterior reads, and inference
  traces.
- Adds active-subgraph compilation around query variables and evidence.
- Runs residual asynchronous sum-product belief propagation with convergence
  diagnostics and warnings.
- Supports persisted posteriors and traces for SQLite-backed graphs.
