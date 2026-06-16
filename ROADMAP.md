# TongGraph Roadmap

## Current Status

TongGraph is not ready for production use. v0.1 is implemented for the current
in-memory Rust/Python API surface. v0.2 and v0.3 are implemented for local
SQLite-backed storage, property cataloging, persisted compute segments, snapshot
reads, and graph compute runtime APIs.

Status markers:

- `[x]` implemented
- `[ ]` not implemented
- `[ ] (partial)` partially implemented

### v0.1: In-memory graph kernel

- [x] Rust core
- [x] Node and edge creation
- [x] Node and edge lookup
- [x] Internal ID mapping
- [x] CSR/CSC adjacency
- [x] Neighbor lookup
- [x] Edge-type-filtered neighbor lookup
- [x] K-hop traversal
- [x] Sparse frontier traversal
- [x] Python bindings

### v0.2: Persistence and indexing

- [x] SQLite-backed local metadata store
- [x] External ID to internal `u64` ID mapping
- [x] Local operation log / WAL-like append stream
- [x] Snapshot reads
- [x] Label index
- [x] Edge-type index
- [x] Property index
- [x] Property dictionary and typed value storage
- [x] Factor, evidence, and trace metadata tables
- [x] Immutable segment compaction
- [x] Storage backend abstraction for future LMDB, RocksDB, and custom segment
  backends

SQLite is the default backend, but it is not the graph compute kernel.
Traversal and algorithms read from CSR/CSC segments plus a mutable delta
overlay. SQLite remains the source of truth for metadata, properties, logs, and
small records. Compacted compute segments are stored in sidecar files next to
SQLite databases.

### v0.3: Graph compute runtime

- [x] BFS
- [x] Shortest path
- [x] Connected components
- [x] PageRank
- [x] Random walk
- [x] Subgraph extraction
- [x] Batch compute API

### v0.4: Probabilistic propagation extension

- [x] Variables
- [x] Priors
- [x] Evidence
- [x] Probability transfer over weighted sparse edges
- [ ] (partial) Local propagation over active sparse subgraphs: weighted sparse
  propagation exists; active-subgraph compilation is still pending.
- [ ] CPDs
- [x] Factors
- [ ] Posterior queries
- [ ] Belief propagation
- [x] Inference traces

### v0.5: Query layer

- [ ] Minimal graph query DSL
- [ ] Pattern query planning
- [ ] Optional Cypher-like subset
- [ ] Full-text and vector retrieval adapters

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
