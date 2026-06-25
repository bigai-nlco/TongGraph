# TongGraph

<p align="center">
  <img src="docs/assets/tonggraph-logo.png" alt="TongGraph logo" width="640">
</p>

TongGraph is a lightweight embedded graph compute database for Python applications that need local GraphRAG retrieval, AI memory, agent context graphs, and probabilistic graph reasoning. TongGraph is for applications that want fast local graph structure, persistence, traversal, algorithms, and inference without running a separate database service.

## Feature Highlights

- Property graph model with labels, typed edges, scalar properties, and external IDs.
- Rust core exposed through a compact Python SDK.
- Local SQLite persistence with reopen, indexes, snapshots, and compute segment
  compaction.
- Traversal and analytics APIs for neighbors, k-hop retrieval, BFS, shortest
  path, connected components, PageRank, random walks, subgraphs, and batch jobs.
- Structured path-query DSL plus a provider-neutral natural-language compiler
  hook.
- Embedded Cypher compatibility subset for common local `CREATE`, `MATCH`,
  `MERGE`, and `RETURN` flows.
- Probabilistic graph layer with variables, CPDs, factor tables, evidence,
  active-subgraph belief propagation, posteriors, and traces.
- Reproducible Python benchmark scripts for graph algorithms and belief
  propagation.

## Getting Started

Use the source tree during development:

```bash
git clone https://github.com/bigai-nlco/TongGraph.git
cd TongGraph
uv sync --dev
uv run python scripts/build_python_extension.py
```

Verify the package:

```bash
uv run python -c "from tonggraph import Graph; print(Graph().node_count())"
```

Create a small graph:

```python
from tonggraph import Graph

graph = Graph()
alice = graph.add_node(
    "alice",
    labels=["Person"],
    properties={"name": "Alice", "active": True},
)
bob = graph.add_node("bob", labels=["Person"], properties={"name": "Bob"})
graph.add_edge(alice, bob, "KNOWS", properties={"weight": 0.8})

print(graph.neighbors(alice))
print(graph.k_hop(alice, 1))
```

Use local persistence by passing a SQLite path:

```python
graph = Graph("memory.db")
graph.add_node("session:1", labels=["Session"])
graph.compact()

reopened = Graph("memory.db")
print(reopened.node_count())
```

Run the Python tests and benchmark scripts:

```bash
uv run pytest
uv run python scripts/benchmark_algorithms.py --nodes 1000 --degree 4 --repeat 2
uv run python scripts/benchmark_belief_propagation.py --nodes 1000 --degree 4 --repeat 2
```

## Documentation

- [Quickstarts](docs/quickstart/index.md)
- [Core concepts](docs/core-concepts.md)
- [Examples](docs/examples/index.md)

## Development

```bash
uv sync --dev
uv run python scripts/build_python_extension.py
uv run pytest
uv run mkdocs build --strict
```
