# TongGraph

TongGraph is a lightweight, high-performance embedded graph compute database for
large sparse graph networks, with optional probabilistic propagation.

It is designed to serve two workloads from the same core:

- Graph storage and retrieval workloads: property graph storage, indexing,
  adjacency lookup, neighborhood retrieval, and pattern-oriented query
  primitives.
- Graph compute workloads: fast subgraph extraction, iterative graph algorithms,
  sparse-matrix-style kernels, and probability transfer over sparse networks.

TongGraph is currently pre-alpha. The repository is being initialized and the
public API, storage format, and query interfaces are expected to evolve quickly.

## Why TongGraph

Most graph databases are optimized first as query systems. They are useful for
property graph modeling and traversal, but high-speed graph computation often
requires projecting data into a separate in-memory representation.

TongGraph starts from the opposite direction:

1. Use a compact compute-native graph layout as the core representation.
2. Expose graph database features on top of that layout.
3. Add probabilistic computation as an optional extension, not as a mandatory
   property model.

The goal is to make graph retrieval, graph analytics, and probabilistic graph
inference fast enough to run inside agent systems, research tools, and local
applications without requiring a heavyweight external database service.

The core assumption is that many real workloads are large, sparse, and
neighborhood-driven. TongGraph should make sparse graph storage, retrieval, and
propagation efficient without forcing applications to adopt a heavy graph
database server.

## Local Storage Strategy

TongGraph should use a local database for reliability and development velocity,
but the local database is not the graph compute kernel.

The default early storage backend is SQLite:

```text
SQLite
  - schema metadata
  - external ID to internal u64 ID mappings
  - labels, edge types, and property dictionaries
  - node and edge property records
  - operation log / WAL-like append stream
  - probabilistic variable and factor metadata
  - evidence and inference trace records

TongGraph compute core
  - CSR outgoing adjacency
  - CSC incoming adjacency
  - edge-type segmented adjacency
  - mutable delta overlay
  - immutable compacted graph segments
  - posterior arrays and inference workspaces
```

This gives TongGraph a practical local-first source of truth while keeping
traversal, graph algorithms, subgraph extraction, and belief propagation on
compute-native data structures.

Longer term, storage should be pluggable:

- SQLite backend: default local-first backend for metadata, logs, and small to
  medium graphs.
- LMDB backend: mmap-friendly backend for read-heavy workloads.
- RocksDB backend: write-heavy backend for large ingest and LSM-style
  compaction.
- Custom segment backend: maximum-performance backend once the access patterns
  are stable.

## Scope

TongGraph aims to provide:

- Embedded graph storage
- Lightweight local graph retrieval
- Property graph nodes and edges
- Labels, edge types, and properties
- Fast adjacency retrieval
- Large sparse graph network support
- Label, edge-type, and property indexes
- CSR/CSC-style compute layouts
- Snapshot reads and append-friendly writes
- Subgraph extraction
- Common graph algorithms
- Optional probability propagation over graph neighborhoods
- Optional Bayesian network and factor graph support
- Python bindings over a systems-language core

TongGraph does not initially aim to be:

- A drop-in Neo4j replacement
- A fully compatible Cypher implementation
- A distributed graph database
- A general-purpose relational database
- A probabilistic programming language

## Architecture

TongGraph separates graph storage, graph compute, and probabilistic inference.

```text
TongGraph
  Graph Store
    - nodes
    - edges
    - labels
    - edge types
    - properties
    - indexes

  Compute Store
    - internal u64 IDs
    - outgoing adjacency (CSR)
    - incoming adjacency (CSC)
    - edge-type segmented adjacency
    - mutable delta overlay
    - immutable compacted segments

  Compute Runtime
    - k-hop traversal
    - BFS / DFS
    - shortest path
    - connected components
    - PageRank
    - random walk
    - Pregel-style iterative compute
    - GraphBLAS-style sparse kernels

  Probabilistic Extension
    - random variables
    - priors
    - evidence
    - CPDs
    - factors
    - posterior state
    - belief propagation
    - inference traces
```

## Data Model

At the base layer, TongGraph is a property graph:

```text
Node
  id: u64
  labels: LabelSet
  properties: PropertyMap

Edge
  id: u64
  source: u64
  target: u64
  type: EdgeType
  properties: PropertyMap
```

External IDs can be strings or UUIDs, but the compute engine uses dense or
semi-dense `u64` IDs internally.

## Probabilistic Model

Probability is optional. A graph can be used as a normal property graph without
creating any probabilistic variables or running any inference machinery.

When probabilistic inference is needed, TongGraph adds a separate model layer:

```text
Variable
  id: u64
  owner: optional graph object id
  domain: binary | categorical | continuous
  prior
  posterior

Factor
  id: u64
  inputs: [variable_id]
  outputs: [variable_id]
  function
  parameters
```

This keeps graph properties separate from probabilistic semantics. A property
such as `weight = 0.8` is just data; a variable with a CPD or factor is part of
an inference model.

Probabilistic propagation should work as a lightweight transition layer over
sparse graph neighborhoods first, then scale up to Bayesian network and factor
graph inference when the model requires richer semantics.

TongGraph should support three probability modes over time:

- Weighted graph mode: edge weights and scores for ranking, traversal, and
  local probability transfer.
- Bayesian network mode: directed acyclic dependency graphs with CPDs.
- Factor graph mode: variables and factors as a general inference substrate,
  including loopy belief propagation.

Bayesian networks can be compiled into the factor graph runtime.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the current development plan.

## Python SDK Preview

TongGraph exposes its Rust core to Python through PyO3.

```python
from tonggraph import Graph

graph = Graph()
alice = graph.add_node("alice", labels=["Person"], properties={"name": "Alice"})
bob = graph.add_node("bob", labels=["Person"], properties={"name": "Bob"})
graph.add_edge(alice, bob, "KNOWS", properties={"probability": "0.8"})

assert graph.neighbors(alice) == [bob]
assert graph.k_hop(alice, 1) == [bob]
assert graph.propagate({alice: 1.0}, 1)[bob] == 0.8
```

SQLite-backed local persistence is enabled by passing a database path:

```python
graph = Graph("tonggraph.db")
```

## Development

Sync the Python development environment with uv:

```bash
uv sync --dev
```

Run the Rust test suite:

```bash
cargo test
```

Build the PyO3 extension in-place for local source-tree testing:

```bash
uv run python scripts/build_python_extension.py
```

Run the Python SDK tests:

```bash
uv run pytest
```
