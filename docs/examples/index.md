# Examples

These examples were run against the current Python extension with `uv run
python`.

- [Property Graph Basics](property-graph-basics.md): label indexes, outgoing
  neighbor lookup, two-hop traversal, frontier extraction, and probability
  transfer.
- [Structured Query Layer](structured-query-layer.md): typed path matching,
  property filters, snapshots, and the provider-neutral natural-language query
  hook.
- [Cypher Compatibility](cypher-compatibility.md): embedded `Graph.cypher()`,
  result records, parameters, snapshots, and staged transactions.
- [Runtime Algorithms And Batch Compute](runtime-algorithms.md): BFS, weighted
  shortest path, connected components, PageRank, random walk, and
  `compute_batch`.
- [Local Probability Transfer](local-probability-transfer.md): global sparse
  propagation compared with radius-limited local propagation.
- [SQLite Reopen](sqlite-reopen.md): SQLite persistence, segment sidecars, and
  reopened indexes.
- [Belief Propagation](belief-propagation.md): CPDs, active subgraph
  compilation, evidence, persisted posteriors, and traces.
- [Benchmarks](benchmarks.md): JSON benchmark artifacts from `tests/benchmark`
  for traversal, query, GraphRAG, persistence, and inference workloads.
