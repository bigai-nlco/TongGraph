"""Python SDK for TongGraph."""

try:
    from ._tonggraph import Edge, Graph, Node, __version__
except ImportError as exc:  # pragma: no cover - exercised only before building.
    raise ImportError(
        "TongGraph's PyO3 extension is not built. Run "
        "`python scripts/build_python_extension.py` from the repository root."
    ) from exc

__all__ = ["Edge", "Graph", "Node", "__version__"]

