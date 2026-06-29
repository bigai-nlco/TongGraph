from __future__ import annotations

import re

from fastapi import APIRouter, Request

from ...query import query_dsl_schema
from ..access import require_graph_access
from ..schemas import CypherRequest, CypherTransactionRequest, QueryRequest
from ..serialization import serialize

router = APIRouter(prefix="/graphs/{graph}")

_WRITE_KEYWORDS = re.compile(r"\b(CREATE|MERGE|SET|REMOVE|DELETE|DETACH)\b", re.IGNORECASE)


def _cypher_access(query: str) -> str:
    return "write" if _WRITE_KEYWORDS.search(query) else "read"


@router.get("/query/schema")
async def query_schema(request: Request, graph: str) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return {"schema": serialize(query_dsl_schema())}


@router.post("/query")
async def structured_query(request: Request, graph: str, payload: QueryRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"result": serialize(graph_obj.query(payload.spec, profile=payload.profile))})


@router.post("/cypher")
async def cypher(request: Request, graph: str, payload: CypherRequest) -> dict[str, object]:
    require_graph_access(request, graph, _cypher_access(payload.query))
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {"result": serialize(graph_obj.cypher(payload.query, payload.parameters, profile=payload.profile))},
    )


@router.post("/cypher/transaction")
async def cypher_transaction(request: Request, graph: str, payload: CypherTransactionRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        tx = graph_obj.transaction()
        results = []
        try:
            for statement in payload.statements:
                results.append(tx.run(statement.query, statement.parameters, profile=statement.profile))
            tx.commit()
        except Exception:
            tx.rollback()
            raise
        return {"results": serialize(results)}

    return request.app.state.registry.call(graph, op)
