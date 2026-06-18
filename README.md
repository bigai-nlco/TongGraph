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

- **Property graph model:** Add nodes and directed typed edges with scalar
  properties, labels, and external IDs.
- **Compute-first layout:** Traversal reads from compacted outgoing and
  incoming adjacency segments with a mutable delta overlay for recent writes.
- **Local persistence:** SQLite stores metadata, properties, operation logs,
  variables, factors, evidence, traces, and segment manifests.
- **Graph algorithms:** Use Python methods for BFS, weighted shortest path,
  connected components, PageRank, random walks, subgraphs, and batch compute
  jobs.
- **Structured query layer:** Match connected path patterns with labels, edge
  types, property filters, return projection, and row limits.
- **Cypher compatibility subset:** Run embedded `Graph.cypher()` queries for
  supported `MATCH`, `CREATE`, `MERGE`, `RETURN`, parameters, result records,
  and staged local transactions.
- **Sparse probability transfer:** Propagate weighted scores over graph
  neighborhoods with damping and radius-limited active neighborhoods.
- **Finite belief propagation:** Build binary or categorical variables, CPDs,
  factor tables, evidence, and residual asynchronous sum-product inference.

Read [Core Concepts](docs/core-concepts.md) for the full storage, scope,
architecture, data model, and probabilistic model notes.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the current development plan.

## Getting Started

TongGraph exposes its Rust core to Python through PyO3.

### Install

During pre-alpha development, use the source tree. Clone the repository, sync
the local environment, and build the PyO3 extension in place:

```bash
git clone https://github.com/bigai-nlco/TongGraph.git
cd TongGraph
uv sync --dev
uv run python scripts/build_python_extension.py
```

Package-index installation is intentionally not the primary pre-alpha path. The
source-tree workflow keeps the Python package and compiled Rust extension
aligned with the checked-out source.

Verify the package:

```bash
uv run python -c "from tonggraph import Graph; print(Graph().node_count())"
```

Expected output:

```text
0
```

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

Structured path-pattern queries return alias-to-ID row bindings:

```python
rows = graph.query(
    {
        "match": [
            {"node": "a", "labels": ["Person"]},
            {"edge": "rel", "type": "KNOWS", "direction": "out"},
            {"node": "b", "properties": {"active": True}},
        ],
        "return": ["a", "rel", "b"],
        "limit": 10,
    }
)
```

Natural-language query compilation is provider-neutral: pass a callable that
turns `(question, schema)` into the structured DSL, then TongGraph executes the
result locally.

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
