from __future__ import annotations

from collections.abc import Callable, Mapping
from typing import Any, Protocol

from ._tonggraph import _query_dsl_schema


class QueryableGraph(Protocol):
    def query(self, spec: Mapping[str, Any]) -> list[dict[str, int]]: ...


QueryCompiler = Callable[[str, Mapping[str, Any]], Mapping[str, Any]]


def query_dsl_schema() -> dict[str, Any]:
    """Return the structured query DSL schema passed to natural-language compilers."""

    return _query_dsl_schema()


def query_nl(
    graph: QueryableGraph,
    question: str,
    compiler: QueryCompiler,
    *,
    schema: Mapping[str, Any] | None = None,
) -> list[dict[str, int]]:
    """Compile a natural-language question into the query DSL and execute it.

    The compiler is caller-supplied so TongGraph does not depend on any LLM
    provider, API key, or network transport.
    """

    query_schema = query_dsl_schema() if schema is None else schema
    spec = compiler(question, query_schema)
    if not isinstance(spec, Mapping):
        raise TypeError("query compiler must return a mapping")
    return graph.query(dict(spec))
