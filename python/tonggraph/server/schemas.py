"""Pydantic request models for the optional server."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field


class GrantRequest(BaseModel):
    user: str
    access: str = Field(pattern="^(read|write)$")


class CreateGraphRequest(BaseModel):
    name: str
    grants: dict[str, str] = Field(default_factory=dict)
    logical_graphs: bool = False


class LogicalGraphCreateRequest(BaseModel):
    logical_graph_id: str


class BackupGraphRequest(BaseModel):
    note: str | None = None


class RestoreBackupRequest(BaseModel):
    graph: str
    overwrite: bool = False
    grants: dict[str, str] = Field(default_factory=dict)


class UserCreateRequest(BaseModel):
    user_id: str
    token: str | None = None
    admin: bool = False
    disabled: bool = False
    graphs: dict[str, str] = Field(default_factory=dict)


class UserUpdateRequest(BaseModel):
    admin: bool | None = None
    disabled: bool | None = None
    graphs: dict[str, str] | None = None


class UserTokenRotateRequest(BaseModel):
    token: str | None = None


class NodeCreateRequest(BaseModel):
    external_id: str | None = None
    labels: list[str] | None = None
    properties: dict[str, Any] | None = None
    logical_graph_id: str | None = None


class NodeBatchCreateRequest(BaseModel):
    records: list[NodeCreateRequest]
    logical_graph_id: str | None = None


class NodeUpdateRequest(BaseModel):
    external_id: str | None = None
    add_labels: list[str] | None = None
    remove_labels: list[str] | None = None
    set_properties: dict[str, Any] | None = None
    remove_properties: list[str] | None = None
    logical_graph_id: str | None = None


class EdgeCreateRequest(BaseModel):
    source: int
    target: int
    edge_type: str
    properties: dict[str, Any] | None = None
    logical_graph_id: str | None = None


class EdgeBatchCreateRequest(BaseModel):
    records: list[EdgeCreateRequest]
    logical_graph_id: str | None = None


class EdgeUpdateRequest(BaseModel):
    set_properties: dict[str, Any] | None = None
    remove_properties: list[str] | None = None
    logical_graph_id: str | None = None


class FullTextIndexRequest(BaseModel):
    name: str
    properties: list[str]
    target: str = "node"
    tokenizer: str = "unicode61"


class TextSearchRequest(BaseModel):
    query: str
    logical_graph_id: str | None = None
    mode: str = "all"
    labels: list[str] | None = None
    edge_type: str | None = None
    properties: dict[str, Any] | None = None
    limit: int = 20
    offset: int = 0


class VectorIndexRequest(BaseModel):
    name: str
    dimensions: int
    target: str = "node"
    metric: str = "cosine"
    model: str | None = None
    model_version: str | None = None


class VectorUpsertRequest(BaseModel):
    vector: list[float]
    logical_graph_id: str | None = None


class VectorBatchUpsertRequest(BaseModel):
    vectors: dict[int, list[float]]
    logical_graph_id: str | None = None


class VectorBatchDeleteRequest(BaseModel):
    entity_ids: list[int]
    logical_graph_id: str | None = None


class VectorSearchRequest(BaseModel):
    query_vector: list[float]
    logical_graph_id: str | None = None
    labels: list[str] | None = None
    edge_type: str | None = None
    properties: dict[str, Any] | None = None
    min_score: float | None = None
    limit: int = 20
    offset: int = 0


class VectorBatchSearchRequest(BaseModel):
    query_vectors: list[list[float]]
    logical_graph_id: str | None = None
    labels: list[str] | None = None
    edge_type: str | None = None
    properties: dict[str, Any] | None = None
    min_score: float | None = None
    limit: int = 20
    offset: int = 0


class QueryRequest(BaseModel):
    spec: dict[str, Any]
    profile: bool = False
    logical_graph_id: str | None = None


class RetrieveContextRequest(BaseModel):
    text_query: str | None = None
    logical_graph_id: str | None = None
    text_index: str | None = None
    vector_query: list[float] | None = None
    vector_index: str | None = None
    labels: list[str] | None = None
    edge_type: str | None = None
    properties: dict[str, Any] | None = None
    radius: int = 1
    direction: str = "both"
    limit: int = 20
    text_weight: float = 1.0
    vector_weight: float = 1.0
    graph_weight: float = 0.1


class ImportPathRequest(BaseModel):
    path: str


class ExportNodesRequest(BaseModel):
    path: str
    nodes: list[int] | None = None


class ExportEdgesRequest(BaseModel):
    path: str
    edges: list[int] | None = None


class ExportRowsRequest(BaseModel):
    path: str
    rows: list[dict[str, Any]]


class CypherRequest(BaseModel):
    query: str
    parameters: dict[str, Any] | None = None
    profile: bool = False
    logical_graph_id: str | None = None


class CypherTransactionRequest(BaseModel):
    statements: list[CypherRequest]


class FrontierRequest(BaseModel):
    starts: list[int]
    logical_graph_id: str | None = None
    steps: int
    direction: str = "out"
    edge_type: str | None = None


class SubgraphRequest(BaseModel):
    nodes: list[int]
    logical_graph_id: str | None = None
    edge_type: str | None = None


class ComputeBatchRequest(BaseModel):
    jobs: list[dict[str, Any]]
    logical_graph_id: str | None = None


class SnapshotCreateRequest(BaseModel):
    ttl_seconds: float = Field(default=600.0, gt=0, le=3600.0)
    logical_graph_id: str | None = None


class PropagateRequest(BaseModel):
    seeds: dict[int, float]
    steps: int
    edge_property: str = "probability"
    damping: float = 1.0
    edge_type: str | None = None


class LocalPropagateRequest(BaseModel):
    seeds: dict[int, float]
    radius: int = 2
    query_nodes: list[int] | None = None
    edge_type: str | None = None
    edge_property: str = "probability"
    damping: float = 1.0


class VariableCreateRequest(BaseModel):
    domain: str
    owner_id: int | None = None
    prior: dict[str, Any] | None = None
    posterior: dict[str, Any] | None = None
    states: list[str] | None = None


class FactorCreateRequest(BaseModel):
    input_variables: list[int]
    output_variables: list[int]
    function: str
    parameters: dict[str, Any] | None = None


class FactorTableCreateRequest(BaseModel):
    variables: list[int]
    values: list[float]


class CpdCreateRequest(BaseModel):
    variable_id: int
    parent_variables: list[int]
    values: list[float]


class EvidenceCreateRequest(BaseModel):
    variable_id: int
    payload: dict[str, Any] | None = None


class TraceCreateRequest(BaseModel):
    payload: dict[str, Any] | None = None


class ActiveSubgraphRequest(BaseModel):
    query_variables: list[int]
    evidence: dict[int, str] | None = None
    radius: int = 2
    max_nodes: int = 10000
    max_factors: int = 50000


class BeliefPropagationRequest(BaseModel):
    query_variables: list[int] | None = None
    evidence: dict[int, str] | None = None
    radius: int = 2
    max_iters: int = 1000
    tolerance: float = 1e-6
    damping: float = 0.2
    persist: bool = False
