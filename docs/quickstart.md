# Quickstart

TongGraph is currently Python-first. The implementation is Rust/PyO3 under the
hood, but Python users work with the `tonggraph` package.

!!! tip "Choose an install path"
    During pre-alpha development, use the source tree. Clone the repository,
    sync the local environment, and build the PyO3 extension in place.

## Install

=== "Python"

    ```bash
    git clone https://github.com/bigai-nlco/TongGraph.git
    cd TongGraph
    uv sync --dev
    uv run python scripts/build_python_extension.py
    ```

    Package-index installation is intentionally not the primary pre-alpha path.
    The documented path above keeps the Python package and compiled Rust
    extension aligned with the checked-out source.

## Verify The Package

=== "Python"

    ```bash
    uv run python -c "from tonggraph import Graph; print(Graph().node_count())"
    ```

    Expected output:

    ```text
    0
    ```

## First Graph

=== "Python"

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

    print(graph.node_count())
    print(graph.neighbors(alice))
    print(graph.nodes_with_label("Person"))
    ```

    Expected output:

    ```text
    2
    [1]
    [0, 1]
    ```

## Use SQLite Persistence

=== "Python"

    ```python
    from tonggraph import Graph

    graph = Graph("tonggraph.db")
    source = graph.add_node("source", labels=["Entity"])
    target = graph.add_node("target", labels=["Entity"])
    graph.add_edge(source, target, "LINKS", properties={"probability": 0.75})
    graph.compact()
    del graph

    reopened = Graph.open("tonggraph.db")
    print(reopened.node_count(), reopened.edge_count())
    print(reopened.neighbors(source, edge_type="LINKS"))
    ```

    Expected output:

    ```text
    2 1
    [1]
    ```

!!! note "Local files"
    SQLite-backed graphs create a `.db` file and a sibling `.segments/`
    directory when compacted. Both are local data artifacts and should not be
    committed.

## Run Tests

=== "Python"

    ```bash
    uv run python scripts/build_python_extension.py
    uv run pytest
    ```

For graph algorithm and inference benchmarks:

=== "Python"

    ```bash
    uv run python scripts/benchmark_algorithms.py --nodes 1000 --degree 4 --repeat 2
    uv run python scripts/benchmark_belief_propagation.py --nodes 1000 --degree 4 --repeat 2
    ```
