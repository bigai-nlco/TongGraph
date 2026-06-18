# Core Concepts

TongGraph is an embedded graph compute database. It keeps a local property graph
as the source of truth, maintains compute-native adjacency structures for fast
traversal, and exposes optional finite-discrete probabilistic inference when a
graph needs belief updates.

## Why TongGraph

Most graph databases start as query systems. They are useful for property graph
modeling and traversal, but high-speed graph computation often requires moving
data into a separate in-memory representation.

TongGraph starts from the opposite direction:

1. Use a compact compute-native graph layout as the core representation.
2. Expose graph database features on top of that layout.
3. Add probabilistic computation as an optional layer, not as a mandatory
   property model.

This makes sparse graph retrieval, graph algorithms, and local probabilistic
updates practical inside agent systems, research tools, and desktop or server
applications without requiring a separate graph database service.

The core assumption is that many useful graph workloads are large, sparse, and
neighborhood-driven. TongGraph is optimized around that shape.

## Local Storage Strategy

TongGraph uses local storage for durability and reproducible development, but
the database file is not the compute kernel.

SQLite is the default backend. It stores:

- schema metadata and operation logs
- external IDs and internal integer IDs
- labels, edge types, and property catalogs
- node and edge property records
- probabilistic variable, factor, evidence, posterior, and trace records
- compacted segment manifests

The Rust core owns the traversal and inference working state:

- outgoing and incoming adjacency structures
- edge-type indexes
- mutable delta overlays for recent writes
- immutable compacted graph segments
- posterior arrays and message-passing workspaces

This split keeps the system local-first and recoverable while allowing graph
algorithms and belief propagation to run over compact in-memory structures.

Future storage backends may specialize for different access patterns: SQLite for
local-first metadata and small to medium graphs, LMDB for mmap-heavy reads,
RocksDB for large ingest, and custom segment storage for maximum traversal
throughput.

## Scope

TongGraph is intended to provide:

- embedded graph storage
- property graph nodes and directed typed edges
- labels, edge types, and scalar properties
- external IDs mapped to internal integer IDs
- adjacency lookup, neighborhood traversal, and local retrieval
- structured path-pattern queries
- label, edge-type, and property indexes
- snapshot reads and append-friendly writes
- common graph algorithms
- sparse probability transfer over graph neighborhoods
- finite-discrete variables, factors, CPDs, evidence, posteriors, and traces
- Python bindings over a Rust core

TongGraph is not trying to be a drop-in Neo4j replacement, a distributed graph
database, a full Cypher-compatible query engine, a general-purpose relational
database, or a full probabilistic programming language.

## Architecture

TongGraph separates durable records, compute layout, and inference semantics.

```text
TongGraph
  Graph Store
    - nodes, edges, labels, edge types, properties
    - indexes and operation log

  Compute Store
    - internal u64 IDs
    - outgoing and incoming adjacency
    - mutable delta overlay
    - immutable compacted segments

  Compute Runtime
    - neighbors, k-hop, frontier
    - BFS, shortest path, connected components
    - PageRank, random walk, subgraph extraction
    - batch compute jobs

  Query Layer
    - structured node-edge-node path patterns
    - labels, edge types, and property filters
    - return projection and row limits

  Probabilistic Extension
    - variables, ordered states, priors, posteriors
    - factor tables, CPDs, evidence
    - active subgraph compilation
    - residual asynchronous belief propagation
    - inference traces
```

Applications interact with the Python API. The PyO3 boundary converts Python
values into Rust records and converts Rust results back into Python objects,
lists, and dictionaries.

## Data Model

At the base layer, TongGraph is a property graph.

```text
Node
  id: u64
  external_id: string
  labels: [string]
  properties: {string: scalar}

Edge
  id: u64
  source: u64
  target: u64
  edge_type: string
  properties: {string: scalar}
```

External IDs are application-facing strings. The compute engine uses internal
`u64` IDs for compact storage and fast adjacency lookup.

Properties are scalar metadata. They can be used by graph algorithms, for
example as edge weights, but they do not automatically create probabilistic
semantics.

## Query Model

The query layer matches one connected path pattern over the property graph.
A query is a structured dictionary rather than a string language:

```python
{
    "match": [
        {"node": "a", "labels": ["Person"]},
        {"edge": "rel", "type": "KNOWS", "direction": "out"},
        {"node": "b", "properties": {"active": True}},
    ],
    "return": ["a", "rel", "b"],
}
```

The result is a list of row dictionaries that bind aliases to internal node or
edge IDs. The same query can run against a mutable `Graph` or a read-only
`GraphSnapshot`.

Natural-language query support is intentionally provider-neutral. Applications
can pass a compiler callable that converts a question into the structured DSL;
TongGraph validates and executes the resulting local query.

See [Query Layer](design/query-layer.md) for the full DSL.

## Probabilistic Model

Probability is optional. A TongGraph graph can be used as a normal property
graph without defining variables or running inference.

When inference is needed, TongGraph adds a separate finite-discrete model layer:

```text
Variable
  id: u64
  owner_id: optional graph node id
  domain: binary | categorical
  states: [string]
  prior
  posterior

Factor
  id: u64
  input_variables: [variable_id]
  output_variables: [variable_id]
  function
  parameters

Factor Table
  factor_id: u64
  variables: [variable_id]
  values: [f64]

Evidence
  variable_id: u64
  state: string
```

This keeps graph data and inference semantics explicit. A property such as
`probability = 0.8` can drive sparse score propagation, while a variable with a
CPD or factor table participates in belief propagation.

TongGraph currently supports weighted sparse propagation and finite-discrete
sum-product belief propagation over active subgraphs. Bayesian-network-style
CPDs are compiled into the same factor-table runtime.
