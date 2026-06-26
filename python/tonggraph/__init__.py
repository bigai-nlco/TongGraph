"""Python SDK for TongGraph."""

try:
    from ._tonggraph import (
        CypherResult,
        Edge,
        Evidence,
        Factor,
        Graph,
        GraphSnapshot,
        GraphTransaction,
        Node,
        Trace,
        Variable,
        __version__,
    )
except ImportError as exc:  # pragma: no cover - exercised only before building.
    raise ImportError(
        "TongGraph's PyO3 extension is not built. Run "
        "`python scripts/build_python_extension.py` from the repository root."
    ) from exc

from .helpers import install_graph_helpers
from .query import query_dsl_schema, query_nl

install_graph_helpers(Graph, GraphSnapshot)

__all__ = [
    "Edge",
    "Evidence",
    "Factor",
    "Graph",
    "GraphSnapshot",
    "GraphTransaction",
    "CypherResult",
    "Node",
    "Trace",
    "Variable",
    "__version__",
    "query_dsl_schema",
    "query_nl",
]
