from __future__ import annotations

from typing import Any, Callable, Mapping, Sequence

__version__: str

PropertyValue = bool | int | float | str
Properties = Mapping[str, PropertyValue]
Distribution = dict[str, float]
QueryCompiler = Callable[[str, Mapping[str, Any]], Mapping[str, Any]]

def query_dsl_schema() -> dict[str, Any]:
    """Return the structured query DSL schema."""
    ...

def query_nl(
    graph: GraphSnapshot,
    question: str,
    compiler: QueryCompiler,
    *,
    schema: Mapping[str, Any] | None = None,
) -> list[dict[str, int]]:
    """Compile a natural-language question into the query DSL and execute it."""
    ...

class CypherResult:
    """Result returned by Cypher execution."""

    @property
    def keys(self) -> list[str]:
        """Returned column names in order."""
        ...

    @property
    def records(self) -> list[dict[str, Any]]:
        """Rows as dictionaries keyed by returned column name."""
        ...

    @property
    def summary(self) -> dict[str, Any]:
        """Execution counters and statement metadata."""
        ...

    def __len__(self) -> int:
        """Return the number of result records."""
        ...

class Node:
    """Immutable node record returned by graph lookup methods.

    Nodes are addressed by dense integer IDs inside TongGraph. The optional
    external ID lets applications keep a stable string identifier at the API
    boundary.
    """

    @property
    def id(self) -> int:
        """Internal integer node ID."""
        ...

    @property
    def external_id(self) -> str:
        """Application-facing node identifier."""
        ...

    @property
    def labels(self) -> list[str]:
        """Labels attached to the node."""
        ...

    @property
    def properties(self) -> dict[str, PropertyValue]:
        """Node properties as Python scalar values."""
        ...

class Edge:
    """Immutable directed edge record returned by graph lookup methods."""

    @property
    def id(self) -> int:
        """Internal integer edge ID."""
        ...

    @property
    def source(self) -> int:
        """Source node ID."""
        ...

    @property
    def target(self) -> int:
        """Target node ID."""
        ...

    @property
    def edge_type(self) -> str:
        """Application-defined edge type."""
        ...

    @property
    def properties(self) -> dict[str, PropertyValue]:
        """Edge properties as Python scalar values."""
        ...

class Variable:
    """Finite discrete probabilistic variable attached to an optional graph node."""

    @property
    def id(self) -> int:
        """Internal integer variable ID."""
        ...

    @property
    def owner_id(self) -> int | None:
        """Owning graph node ID, if the variable is attached to a node."""
        ...

    @property
    def domain(self) -> str:
        """Variable domain, commonly ``"binary"`` or ``"categorical"``."""
        ...

    @property
    def states(self) -> list[str]:
        """Ordered state labels used by priors, CPDs, factors, and posteriors."""
        ...

    @property
    def prior(self) -> dict[str, PropertyValue]:
        """Prior distribution metadata."""
        ...

    @property
    def posterior(self) -> dict[str, PropertyValue]:
        """Persisted posterior metadata stored with the variable record."""
        ...

class Factor:
    """Probabilistic factor metadata record."""

    @property
    def id(self) -> int:
        """Internal integer factor ID."""
        ...

    @property
    def input_variables(self) -> list[int]:
        """Variables treated as factor inputs."""
        ...

    @property
    def output_variables(self) -> list[int]:
        """Variables treated as factor outputs."""
        ...

    @property
    def function(self) -> str:
        """Factor function name, such as ``"factor_table"`` or ``"cpd"``."""
        ...

    @property
    def parameters(self) -> dict[str, PropertyValue]:
        """Factor parameter metadata."""
        ...

class Evidence:
    """Persisted evidence metadata record."""

    @property
    def id(self) -> int:
        """Internal integer evidence ID."""
        ...

    @property
    def variable_id(self) -> int:
        """Variable ID the evidence applies to."""
        ...

    @property
    def payload(self) -> dict[str, PropertyValue]:
        """Evidence payload. A ``state`` string is used by belief propagation."""
        ...

class Trace:
    """Persisted inference trace metadata record."""

    @property
    def id(self) -> int:
        """Internal integer trace ID."""
        ...

    @property
    def payload(self) -> dict[str, PropertyValue]:
        """Trace payload describing an inference run."""
        ...

class GraphSnapshot:
    """Read-only snapshot of graph state.

    A snapshot copies the current in-memory state and has no persistence handle.
    It supports retrieval and compute methods but not mutation methods.
    """

    def node_count(self) -> int:
        """Return the number of live nodes."""
        ...

    def edge_count(self) -> int:
        """Return the number of live edges."""
        ...

    def variable_count(self) -> int:
        """Return the number of probabilistic variables."""
        ...

    def factor_count(self) -> int:
        """Return the number of factors."""
        ...

    def evidence_count(self) -> int:
        """Return the number of evidence records."""
        ...

    def trace_count(self) -> int:
        """Return the number of trace records."""
        ...

    def node_ids(self) -> list[int]:
        """Return live node IDs ordered by internal ID."""
        ...

    def edge_ids(self) -> list[int]:
        """Return live edge IDs ordered by internal ID."""
        ...

    def nodes(self) -> list[Node]:
        """Return node records ordered by internal ID."""
        ...

    def edges(self) -> list[Edge]:
        """Return edge records ordered by internal ID."""
        ...

    def get_node(self, node_id: int) -> Node:
        """Return a node record by ID.

        Raises:
            KeyError: If the node does not exist.
        """
        ...

    def get_edge(self, edge_id: int) -> Edge:
        """Return an edge record by ID.

        Raises:
            KeyError: If the edge does not exist.
        """
        ...

    def get_variable(self, variable_id: int) -> Variable:
        """Return a variable record by ID.

        Raises:
            KeyError: If the variable does not exist.
        """
        ...

    def get_factor(self, factor_id: int) -> Factor:
        """Return a factor record by ID.

        Raises:
            KeyError: If the factor does not exist.
        """
        ...

    def get_evidence(self, evidence_id: int) -> Evidence:
        """Return an evidence record by ID.

        Raises:
            KeyError: If the evidence record does not exist.
        """
        ...

    def get_trace(self, trace_id: int) -> Trace:
        """Return a trace record by ID.

        Raises:
            KeyError: If the trace record does not exist.
        """
        ...

    def get_node_id(self, external_id: str) -> int | None:
        """Look up an internal node ID by external ID."""
        ...

    def nodes_with_label(self, label: str) -> list[int]:
        """Return node IDs that have a label."""
        ...

    def edges_by_type(self, edge_type: str) -> list[int]:
        """Return edge IDs for an edge type."""
        ...

    def nodes_with_property(self, key: str, value: PropertyValue | None = None) -> list[int]:
        """Return node IDs that contain a property key, optionally filtered by value."""
        ...

    def edges_with_property(self, key: str, value: PropertyValue | None = None) -> list[int]:
        """Return edge IDs that contain a property key, optionally filtered by value."""
        ...

    def neighbors(self, node_id: int, direction: str = "out", edge_type: str | None = None) -> list[int]:
        """Return adjacent node IDs.

        Args:
            node_id: Node to expand.
            direction: ``"out"``, ``"in"``, or ``"both"``.
            edge_type: Optional edge-type filter.
        """
        ...

    def k_hop(self, start: int, hops: int, direction: str = "out", edge_type: str | None = None) -> list[int]:
        """Return nodes reached within ``hops`` traversal steps, excluding ``start``."""
        ...

    def frontier(self, starts: Sequence[int], steps: int, direction: str = "out", edge_type: str | None = None) -> list[int]:
        """Return only the nodes reached at the final traversal step."""
        ...

    def bfs(self, start: int, direction: str = "out", edge_type: str | None = None, max_depth: int | None = None) -> list[int]:
        """Run breadth-first search and return visited nodes in traversal order."""
        ...

    def shortest_path(
        self,
        start: int,
        target: int,
        direction: str = "out",
        edge_type: str | None = None,
        weight_property: str | None = None,
    ) -> dict[str, Any] | None:
        """Return the shortest path and distance, or ``None`` when unreachable."""
        ...

    def connected_components(self, edge_type: str | None = None) -> list[list[int]]:
        """Return weakly connected components using both incoming and outgoing edges."""
        ...

    def pagerank(
        self,
        iterations: int = 20,
        damping: float = 0.85,
        tolerance: float | None = None,
        edge_type: str | None = None,
    ) -> dict[int, float]:
        """Return PageRank scores keyed by node ID."""
        ...

    def random_walk(
        self,
        start: int,
        steps: int,
        direction: str = "out",
        edge_type: str | None = None,
        seed: int | None = None,
    ) -> list[int]:
        """Run a random walk and return the path, including the start node."""
        ...

    def subgraph(self, nodes: Sequence[int], edge_type: str | None = None) -> GraphSnapshot:
        """Return a snapshot containing selected nodes and internal edges."""
        ...

    def compute_batch(self, jobs: Sequence[Mapping[str, Any]]) -> list[Any]:
        """Run multiple compute jobs and return results in input order."""
        ...

    def query(self, spec: Mapping[str, Any]) -> list[dict[str, int]]:
        """Run a structured path-pattern query and return alias-to-ID row bindings."""
        ...

    def query_schema(self) -> dict[str, Any]:
        """Return the structured query DSL schema."""
        ...

    def cypher(
        self,
        query: str,
        parameters: Mapping[str, Any] | None = None,
    ) -> CypherResult:
        """Run a read-only Cypher query against the snapshot."""
        ...

class GraphTransaction:
    """Explicit staged Cypher transaction."""

    def run(
        self,
        query: str,
        parameters: Mapping[str, Any] | None = None,
    ) -> CypherResult:
        """Run a Cypher query inside this transaction."""
        ...

    def commit(self) -> None:
        """Commit staged changes."""
        ...

    def rollback(self) -> None:
        """Discard staged changes."""
        ...

    def __enter__(self) -> GraphTransaction:
        """Enter the transaction context."""
        ...

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> bool:
        """Commit on success and roll back on exception."""
        ...

class Graph(GraphSnapshot):
    """Mutable embedded graph database.

    ``Graph()`` creates an in-memory graph. ``Graph(path)`` or ``Graph.open(path)``
    opens a SQLite-backed graph at ``path`` and persists metadata, properties,
    probabilistic records, and compacted compute segments.
    """

    def __init__(self, path: str | None = None) -> None:
        """Create an in-memory graph or open a SQLite-backed graph."""
        ...

    @staticmethod
    def open(path: str) -> Graph:
        """Open a SQLite-backed graph from ``path``."""
        ...

    def add_node(
        self,
        external_id: str | None = None,
        labels: Sequence[str] | None = None,
        properties: Properties | None = None,
    ) -> int:
        """Add a node and return its internal ID."""
        ...

    def add_edge(
        self,
        source: int,
        target: int,
        edge_type: str,
        properties: Properties | None = None,
    ) -> int:
        """Add a directed edge and return its internal ID."""
        ...

    def add_nodes(self, records: Sequence[Mapping[str, Any]]) -> list[int]:
        """Atomically add node records and return their internal IDs."""
        ...

    def add_edges(self, records: Sequence[Mapping[str, Any]]) -> list[int]:
        """Atomically add directed edge records and return their internal IDs."""
        ...

    def compact(self) -> None:
        """Compact the mutable adjacency overlay into a persisted compute segment."""
        ...

    def refresh(self) -> None:
        """Reload a SQLite-backed graph from disk after another handle writes."""
        ...

    def cypher(
        self,
        query: str,
        parameters: Mapping[str, Any] | None = None,
    ) -> CypherResult:
        """Run a Cypher query in an autocommit transaction."""
        ...

    def transaction(self, write: bool = True) -> GraphTransaction:
        """Create an explicit staged Cypher transaction."""
        ...

    def snapshot(self) -> GraphSnapshot:
        """Return a read-only snapshot of the current graph state."""
        ...

    def propagate(
        self,
        seeds: Mapping[int, float],
        steps: int,
        edge_property: str = "probability",
        damping: float = 1.0,
        edge_type: str | None = None,
    ) -> dict[int, float]:
        """Transfer probability mass over outgoing edges for a fixed number of steps."""
        ...

    def local_propagate(
        self,
        seeds: Mapping[int, float],
        radius: int = 2,
        query_nodes: Sequence[int] | None = None,
        edge_type: str | None = None,
        edge_property: str = "probability",
        damping: float = 1.0,
    ) -> dict[int, float]:
        """Transfer probability only inside a radius-limited active graph neighborhood."""
        ...

    def add_variable(
        self,
        domain: str,
        owner_id: int | None = None,
        prior: Properties | None = None,
        posterior: Properties | None = None,
        states: Sequence[str] | None = None,
    ) -> int:
        """Add a finite discrete variable and return its ID."""
        ...

    def add_factor(
        self,
        input_variables: Sequence[int],
        output_variables: Sequence[int],
        function: str,
        parameters: Properties | None = None,
    ) -> int:
        """Add factor metadata and return its ID."""
        ...

    def add_factor_table(self, variables: Sequence[int], values: Sequence[float]) -> int:
        """Add a dense factor table for ordered variables and return its factor ID."""
        ...

    def add_cpd(self, variable_id: int, parent_variables: Sequence[int], values: Sequence[float]) -> int:
        """Add a conditional probability table for ``variable_id`` and parents."""
        ...

    def add_evidence(self, variable_id: int, payload: Properties | None = None) -> int:
        """Persist evidence metadata for a variable and return the evidence ID."""
        ...

    def add_trace(self, payload: Properties | None = None) -> int:
        """Persist an inference trace metadata record and return its ID."""
        ...

    def compile_active_subgraph(
        self,
        query_variables: Sequence[int],
        evidence: Mapping[int, str] | None = None,
        radius: int = 2,
        max_nodes: int = 10000,
        max_factors: int = 50000,
    ) -> dict[str, Any]:
        """Compile a radius-limited inference subgraph around queries and evidence."""
        ...

    def belief_propagation(
        self,
        query_variables: Sequence[int] | None = None,
        evidence: Mapping[int, str] | None = None,
        radius: int = 2,
        max_iters: int = 1000,
        tolerance: float = 1e-6,
        damping: float = 0.2,
        persist: bool = False,
    ) -> dict[str, Any]:
        """Run residual asynchronous sum-product belief propagation."""
        ...

    def posterior(self, variable_id: int) -> Distribution:
        """Return the current posterior distribution for a variable."""
        ...

__all__: list[str]
