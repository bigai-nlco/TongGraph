<div class="tg-hero" markdown>

<p class="tg-logo">
  <img src="assets/tonggraph-logo.png" alt="TongGraph logo">
</p>

<p class="tg-kicker">Embedded sparse graph compute</p>

# TongGraph

TongGraph is a Python-facing embedded graph compute database backed by a Rust
core. It stores property graphs locally, keeps traversal data in compute-native
adjacency layouts, and adds optional finite-discrete probabilistic inference for
active graph neighborhoods.

<p class="tg-tagline">
Use it when you want graph retrieval, graph algorithms, and lightweight belief
updates in-process rather than behind a separate graph database service.
</p>

[Quickstart](quickstart.md){ .md-button .md-button--primary }
[Core Concepts](core-concepts.md){ .md-button }
[API Reference](api.md){ .md-button }

<div class="tg-pill-row" markdown>
<span class="tg-pill">Python API</span>
<span class="tg-pill">Rust core</span>
<span class="tg-pill">SQLite local storage</span>
<span class="tg-pill">CSR/CSC adjacency</span>
<span class="tg-pill">Belief propagation</span>
</div>

</div>

!!! warning "Pre-alpha project"
    The repository describes TongGraph as pre-alpha. The public API, storage
    format, and query interfaces are expected to evolve.

## Goal

TongGraph starts from a compute-native graph kernel and layers database-like
features on top. The goal is to make sparse graph storage, neighborhood
retrieval, graph algorithms, and probabilistic updates practical inside local
applications, research tools, and agent systems.

## Highlighted Features

<div class="grid cards" markdown>

- :material-graph-outline: **Property graph model**

    Add nodes and directed typed edges with scalar properties, labels, and
    external IDs.

- :material-speedometer: **Compute-first layout**

    Traversal reads from compacted outgoing and incoming adjacency segments with
    a mutable delta overlay for recent writes.

- :material-database: **Local persistence**

    SQLite stores metadata, properties, operation logs, variables, factors,
    evidence, traces, and segment manifests.

- :material-function-variant: **Graph algorithms**

    Use Python methods for BFS, weighted shortest path, connected components,
    PageRank, random walks, subgraphs, and batch compute jobs.

- :material-transit-connection-variant: **Sparse probability transfer**

    Propagate weighted scores over graph neighborhoods with damping and
    radius-limited active neighborhoods.

- :material-chart-bell-curve: **Finite belief propagation**

    Build binary or categorical variables, CPDs, factor tables, evidence, and
    residual asynchronous sum-product inference.

</div>

## Minimal Example

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

## Design Philosophy

TongGraph keeps three ideas separate:

1. **Graph properties are data.** A property like `probability=0.8` can be used
   by traversal or propagation, but it is not automatically a probabilistic
   variable.
2. **Probabilistic semantics are explicit.** Variables, states, factors, CPDs,
   evidence, posteriors, and traces live in a separate model layer.
3. **Storage serves compute.** SQLite is a reliable local source of truth, while
   adjacency traversal and inference run in Rust-owned compute structures.

## Where To Go Next

- Start with [Quickstart](quickstart.md) for installation and first graph.
- Read [Core Concepts](core-concepts.md) for storage, architecture, data model,
  scope, and probabilistic model.
- Use [Examples](examples.md) for expected behavior and live outputs.
- Use [API](api.md) when you need method signatures.
- Read [Algorithms](design/algorithms.md) and
  [Belief Propagation](design/belief-propagation.md) for the math behind the
  APIs.
