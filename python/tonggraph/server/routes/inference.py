from __future__ import annotations

from fastapi import APIRouter, Request

from ..access import require_graph_access
from ..schemas import (
    ActiveSubgraphRequest,
    BeliefPropagationRequest,
    CpdCreateRequest,
    EvidenceCreateRequest,
    FactorCreateRequest,
    FactorTableCreateRequest,
    LocalPropagateRequest,
    PropagateRequest,
    TraceCreateRequest,
    VariableCreateRequest,
)
from ..serialization import serialize

router = APIRouter(prefix="/graphs/{graph}")


@router.post("/propagate")
async def propagate(request: Request, graph: str, payload: PropagateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "scores": serialize(
                graph_obj.propagate(
                    payload.seeds,
                    payload.steps,
                    edge_property=payload.edge_property,
                    damping=payload.damping,
                    edge_type=payload.edge_type,
                )
            )
        },
    )


@router.post("/local-propagate")
async def local_propagate(request: Request, graph: str, payload: LocalPropagateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "scores": serialize(
                graph_obj.local_propagate(
                    payload.seeds,
                    radius=payload.radius,
                    query_nodes=payload.query_nodes,
                    edge_type=payload.edge_type,
                    edge_property=payload.edge_property,
                    damping=payload.damping,
                )
            )
        },
    )


@router.post("/variables")
async def add_variable(request: Request, graph: str, payload: VariableCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        variable_id = graph_obj.add_variable(
            payload.domain,
            owner_id=payload.owner_id,
            prior=payload.prior,
            posterior=payload.posterior,
            states=payload.states,
        )
        return {"id": variable_id, "variable": serialize(graph_obj.get_variable(variable_id))}

    return request.app.state.registry.call(graph, op)


@router.get("/variables/{variable_id}")
async def get_variable(request: Request, graph: str, variable_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"variable": serialize(graph_obj.get_variable(variable_id))})


@router.get("/variables/{variable_id}/posterior")
async def posterior(request: Request, graph: str, variable_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"posterior": serialize(graph_obj.posterior(variable_id))})


@router.post("/factors")
async def add_factor(request: Request, graph: str, payload: FactorCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        factor_id = graph_obj.add_factor(
            payload.input_variables,
            payload.output_variables,
            payload.function,
            parameters=payload.parameters,
        )
        return {"id": factor_id, "factor": serialize(graph_obj.get_factor(factor_id))}

    return request.app.state.registry.call(graph, op)


@router.get("/factors/{factor_id}")
async def get_factor(request: Request, graph: str, factor_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"factor": serialize(graph_obj.get_factor(factor_id))})


@router.post("/factor-tables")
async def add_factor_table(request: Request, graph: str, payload: FactorTableCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        factor_id = graph_obj.add_factor_table(payload.variables, payload.values)
        return {"id": factor_id, "factor": serialize(graph_obj.get_factor(factor_id))}

    return request.app.state.registry.call(graph, op)


@router.post("/cpds")
async def add_cpd(request: Request, graph: str, payload: CpdCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        factor_id = graph_obj.add_cpd(payload.variable_id, payload.parent_variables, payload.values)
        return {"id": factor_id, "factor": serialize(graph_obj.get_factor(factor_id))}

    return request.app.state.registry.call(graph, op)


@router.post("/evidence")
async def add_evidence(request: Request, graph: str, payload: EvidenceCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        evidence_id = graph_obj.add_evidence(payload.variable_id, payload=payload.payload)
        return {"id": evidence_id, "evidence": serialize(graph_obj.get_evidence(evidence_id))}

    return request.app.state.registry.call(graph, op)


@router.get("/evidence/{evidence_id}")
async def get_evidence(request: Request, graph: str, evidence_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"evidence": serialize(graph_obj.get_evidence(evidence_id))})


@router.post("/traces")
async def add_trace(request: Request, graph: str, payload: TraceCreateRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")

    def op(graph_obj):
        trace_id = graph_obj.add_trace(payload.payload)
        return {"id": trace_id, "trace": serialize(graph_obj.get_trace(trace_id))}

    return request.app.state.registry.call(graph, op)


@router.get("/traces/{trace_id}")
async def get_trace(request: Request, graph: str, trace_id: int) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(graph, lambda graph_obj: {"trace": serialize(graph_obj.get_trace(trace_id))})


@router.post("/inference/active-subgraph")
async def compile_active_subgraph(request: Request, graph: str, payload: ActiveSubgraphRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "active_subgraph": serialize(
                graph_obj.compile_active_subgraph(
                    payload.query_variables,
                    evidence=payload.evidence,
                    radius=payload.radius,
                    max_nodes=payload.max_nodes,
                    max_factors=payload.max_factors,
                )
            )
        },
    )


@router.post("/belief-propagation")
async def belief_propagation(request: Request, graph: str, payload: BeliefPropagationRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write" if payload.persist else "read")
    return request.app.state.registry.call(
        graph,
        lambda graph_obj: {
            "result": serialize(
                graph_obj.belief_propagation(
                    payload.query_variables,
                    evidence=payload.evidence,
                    radius=payload.radius,
                    max_iters=payload.max_iters,
                    tolerance=payload.tolerance,
                    damping=payload.damping,
                    persist=payload.persist,
                )
            )
        },
    )
