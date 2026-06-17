# TongGraph

<p align="center">
  <img src="docs/assets/tonggraph-logo.png" alt="TongGraph logo" width="640">
</p>

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

## Highlighted Features

- **Local storage strategy:** SQLite is the local source of truth for records,
  properties, operation logs, variables, factors, evidence, traces, and segment
  manifests. The Rust core keeps traversal and inference on compute-native
  adjacency and message-passing structures.
- **Scope:** TongGraph focuses on embedded property graph storage, local graph
  retrieval, common graph algorithms, sparse score propagation, and
  finite-discrete belief propagation. It is not a distributed graph database or
  a drop-in Cypher/Neo4j replacement.
- **Architecture:** Python users call a PyO3 API backed by a Rust core. The
  core separates durable records, compacted adjacency segments, mutable write
  overlays, graph algorithms, and probabilistic inference state.
- **Data model:** Nodes and directed typed edges have internal `u64` IDs,
  optional external IDs, labels, edge types, and scalar property maps.
- **Probabilistic model:** Probability is explicit and optional. Variables,
  ordered states, factor tables, CPDs, evidence, posteriors, and traces live in
  a separate model layer from graph properties.

Read [Core Concepts](docs/core-concepts.md) for the full storage, scope,
architecture, data model, and probabilistic model notes.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the current development plan.

## Getting Started

TongGraph exposes its Rust core to Python through PyO3.

```python
from tonggraph import Graph

graph = Graph()
alice = graph.add_node(
    "alice",
    labels=["Person"],
    properties={"name": "Alice", "active": True},
)
bob = graph.add_node("bob", labels=["Person"], properties={"name": "Bob"})

graph.add_edge(alice, bob, "KNOWS", properties={"probability": 0.8})

assert graph.neighbors(alice) == [bob]
assert graph.propagate({alice: 1.0}, 1)[bob] == 0.8
```

SQLite-backed local persistence is enabled by passing a database path:

```python
graph = Graph("tonggraph.db")
graph.compact()
```

Common graph algorithms and inference APIs are exposed through direct SDK calls:

```python
graph.bfs(alice, max_depth=2)
graph.shortest_path(alice, bob, weight_property="weight")
graph.connected_components()
graph.pagerank(iterations=20, tolerance=1e-9)
graph.random_walk(alice, 10, seed=7)
graph.subgraph([alice, bob])
```

Discrete factor tables and CPDs can be queried with active-subgraph belief
propagation:

```python
source = graph.add_node("source")
target = graph.add_node("target")
graph.add_edge(source, target, "LINK")

parent = graph.add_variable("binary", owner_id=source, prior={"p": 0.6})
child = graph.add_variable("binary", owner_id=target)
graph.add_cpd(child, [parent], [0.9, 0.1, 0.2, 0.8])

active = graph.compile_active_subgraph([child], evidence={parent: "true"})
result = graph.belief_propagation([child], evidence={parent: "true"}, persist=True)
posterior = graph.posterior(child)
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

Build the local Python extension, run Python tests, and benchmark graph
algorithms:

```bash
uv run python scripts/build_python_extension.py
uv run pytest
uv run python scripts/benchmark_algorithms.py --nodes 1000 --degree 4 --repeat 2
uv run python scripts/benchmark_belief_propagation.py --nodes 1000 --degree 4 --repeat 2
```

Build the PyO3 extension in-place for local source-tree testing:

```bash
uv run python scripts/build_python_extension.py
```

Run the Python SDK tests:

```bash
uv run pytest
```
