# TongGraph Roadmap

## Current Status

TongGraph is not ready for production use. The current priority is to define a
lightweight embedded graph compute database for large sparse graph networks,
including the core storage model, in-memory retrieval kernel, and first Python
API.

### v0.1: In-memory graph kernel

- Rust core
- Node and edge creation
- Node and edge lookup
- Internal ID mapping
- CSR/CSC adjacency
- Neighbor lookup
- Edge-type-filtered neighbor lookup
- K-hop traversal
- Sparse frontier traversal
- Python bindings

### v0.2: Persistence and indexing

- SQLite-backed local metadata store
- External ID to internal `u64` ID mapping
- Local operation log / WAL-like append stream
- Snapshot reads
- Label index
- Edge-type index
- Property index
- Property dictionary and typed value storage
- Factor, evidence, and trace metadata tables
- Immutable segment compaction
- Storage backend abstraction for future LMDB, RocksDB, and custom segment
  backends

SQLite is the default backend, but it is not the graph compute kernel.
Traversal and algorithms should read from CSR/CSC segments plus a mutable delta
overlay. SQLite remains the source of truth for metadata, properties, logs, and
small records.

### v0.3: Graph compute runtime

- BFS
- Shortest path
- Connected components
- PageRank
- Random walk
- Subgraph extraction
- Batch compute API

### v0.4: Probabilistic propagation extension

- Variables
- Priors
- Evidence
- Probability transfer over weighted sparse edges
- Local propagation over active sparse subgraphs
- CPDs
- Factors
- Posterior queries
- Belief propagation
- Inference traces

### v0.5: Query layer

- Minimal graph query DSL
- Pattern query planning
- Optional Cypher-like subset
- Full-text and vector retrieval adapters

## Design Principles

- Lightweight core: keep the engine small enough to embed locally and avoid
  service-only assumptions.
- Compute-native first: storage should serve fast graph computation, not force
  compute to copy into a second representation.
- Embedded by default: applications should be able to link TongGraph as a local
  engine before deploying it as a service.
- Sparse graph first: optimize for large sparse networks, adjacency retrieval,
  frontier traversal, and local propagation.
- SQLite-first, kernel-independent persistence: use SQLite for local reliability
  and metadata while keeping traversal, algorithms, and inference on dedicated
  compute layouts.
- Explicit probability: probabilistic inference should use variables, CPDs, and
  factors rather than overloading graph properties.
- Local inference first: large graphs should be queried and compiled into active
  subgraphs before inference.
- Explainable updates: probabilistic updates should be traceable to evidence and
  factors.
- Backend flexibility: persistence, vector retrieval, and full-text retrieval
  should be replaceable components.

## Language Bindings

The intended core implementation is Rust, with Python as the first user-facing
SDK.

Future bindings may include:

- JavaScript / TypeScript
