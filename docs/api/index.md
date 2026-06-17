# Python API

The reference pages in this section are generated from the Python API stubs in
`python/tonggraph/`. The runtime implementation is provided by the compiled
PyO3 extension.

Use these pages for signatures, argument shapes, and return values. For
algorithmic behavior, start with [Algorithms](../design/algorithms.md) and
[Belief Propagation](../design/belief-propagation.md). For storage behavior, see
[Persistence](../design/persistence.md).

## Core Classes

- [`Graph`](graph.md) is the mutable embedded graph database.
- [`GraphSnapshot`](graph-snapshot.md) is a read-only copy of graph state
  used for stable query and compute views.

## Records

- [`Node`](node.md) and [`Edge`](edge.md) describe property graph
  records returned by lookup methods.
- [`Variable`](variable.md), [`Factor`](factor.md),
  [`Evidence`](evidence.md), and [`Trace`](trace.md) describe finite
  discrete inference records.
