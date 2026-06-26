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


class NodeCreateRequest(BaseModel):
    external_id: str | None = None
    labels: list[str] | None = None
    properties: dict[str, Any] | None = None


class NodeUpdateRequest(BaseModel):
    external_id: str | None = None
    add_labels: list[str] | None = None
    remove_labels: list[str] | None = None
    set_properties: dict[str, Any] | None = None
    remove_properties: list[str] | None = None


class EdgeCreateRequest(BaseModel):
    source: int
    target: int
    edge_type: str
    properties: dict[str, Any] | None = None


class EdgeUpdateRequest(BaseModel):
    set_properties: dict[str, Any] | None = None
    remove_properties: list[str] | None = None


class FullTextIndexRequest(BaseModel):
    name: str
    properties: list[str]
    target: str = "node"
    tokenizer: str = "unicode61"


class TextSearchRequest(BaseModel):
    query: str
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


class VectorSearchRequest(BaseModel):
    query_vector: list[float]
    labels: list[str] | None = None
    edge_type: str | None = None
    properties: dict[str, Any] | None = None
    min_score: float | None = None
    limit: int = 20
    offset: int = 0


class VectorBatchSearchRequest(BaseModel):
    query_vectors: list[list[float]]
    labels: list[str] | None = None
    edge_type: str | None = None
    properties: dict[str, Any] | None = None
    min_score: float | None = None
    limit: int = 20
    offset: int = 0


class QueryRequest(BaseModel):
    spec: dict[str, Any]
    profile: bool = False


class CypherRequest(BaseModel):
    query: str
    parameters: dict[str, Any] | None = None
    profile: bool = False


class CypherTransactionRequest(BaseModel):
    statements: list[CypherRequest]


class FrontierRequest(BaseModel):
    starts: list[int]
    steps: int
    direction: str = "out"
    edge_type: str | None = None


class SubgraphRequest(BaseModel):
    nodes: list[int]
    edge_type: str | None = None


class ComputeBatchRequest(BaseModel):
    jobs: list[dict[str, Any]]


class SnapshotCreateRequest(BaseModel):
    ttl_seconds: float = Field(default=600.0, gt=0, le=3600.0)
