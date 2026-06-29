"""Synchronous HTTP client for TongGraph Server."""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any


class TongGraphServerError(RuntimeError):
    """Error returned by TongGraph Server or raised while calling it."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        status_code: int = 0,
        graph: str | None = None,
        request_id: str | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.status_code = status_code
        self.graph = graph
        self.request_id = request_id

    def __str__(self) -> str:
        location = f" graph={self.graph}" if self.graph else ""
        request = f" request_id={self.request_id}" if self.request_id else ""
        status = f" status={self.status_code}" if self.status_code else ""
        return f"{self.code}: {self.message}{status}{location}{request}"


@dataclass(frozen=True)
class _RequestOptions:
    method: str
    path: str
    body: dict[str, Any] | None = None
    query: dict[str, Any] | None = None


class TongGraphClient:
    """Client for an already running TongGraph Server."""

    def __init__(
        self,
        base_url: str,
        token: str | None = None,
        timeout: float = 30.0,
        headers: dict[str, str] | None = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.token = token
        self.timeout = timeout
        self.headers = dict(headers or {})

    def graph(self, name: str) -> RemoteGraph:
        return RemoteGraph(self, name)

    def health(self) -> dict[str, Any]:
        return self._request("GET", "/health")

    def graphs(self) -> list[dict[str, Any]]:
        return self._request("GET", "/graphs")["graphs"]

    def admin_graphs(self) -> list[dict[str, Any]]:
        return self._request("GET", "/admin/graphs")["graphs"]

    def create_graph(self, name: str, grants: dict[str, str] | None = None) -> dict[str, Any]:
        return self._request("POST", "/admin/graphs", {"name": name, "grants": grants or {}})["graph"]

    def grant_graph(self, graph: str, user: str, access: str) -> dict[str, Any]:
        return self._request(
            "POST",
            f"/admin/graphs/{_quote(graph)}/grants",
            {"user": user, "access": access},
        )

    def revoke_graph(self, graph: str, user: str) -> dict[str, Any]:
        return self._request("DELETE", f"/admin/graphs/{_quote(graph)}/grants/{_quote(user)}")

    def admin_users(self) -> list[dict[str, Any]]:
        return self._request("GET", "/admin/users")["users"]

    def admin_user(self, user: str) -> dict[str, Any]:
        return self._request("GET", f"/admin/users/{_quote(user)}")["user"]

    def create_user(
        self,
        user_id: str,
        token: str | None = None,
        admin: bool = False,
        disabled: bool = False,
        graphs: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        return self._request(
            "POST",
            "/admin/users",
            {
                "user_id": user_id,
                "token": token,
                "admin": admin,
                "disabled": disabled,
                "graphs": graphs or {},
            },
        )["user"]

    def update_user(
        self,
        user: str,
        *,
        admin: bool | None = None,
        disabled: bool | None = None,
        graphs: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        return self._request(
            "PATCH",
            f"/admin/users/{_quote(user)}",
            {"admin": admin, "disabled": disabled, "graphs": graphs},
        )["user"]

    def rotate_user_token(self, user: str, token: str | None = None) -> dict[str, Any]:
        return self._request("POST", f"/admin/users/{_quote(user)}/token", {"token": token})

    def delete_user(self, user: str) -> dict[str, Any]:
        return self._request("DELETE", f"/admin/users/{_quote(user)}")

    def _request(
        self,
        method: str,
        path: str,
        body: dict[str, Any] | None = None,
        query: dict[str, Any] | None = None,
    ) -> Any:
        options = _RequestOptions(method=method, path=path, body=body, query=query)
        request = self._build_request(options)
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                return _decode_response(response.read())
        except urllib.error.HTTPError as exc:
            raise _error_from_http(exc) from None
        except urllib.error.URLError as exc:
            raise TongGraphServerError("request_failed", str(exc.reason), status_code=0) from None

    def _build_request(self, options: _RequestOptions) -> urllib.request.Request:
        data: bytes | None = None
        headers = {"Accept": "application/json", **self.headers}
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        if options.body is not None:
            data = json.dumps(options.body).encode("utf-8")
            headers["Content-Type"] = "application/json"

        path = options.path if options.path.startswith("/") else f"/{options.path}"
        url = f"{self.base_url}{path}"
        if options.query:
            query = _clean_query(options.query)
            if query:
                url = f"{url}?{urllib.parse.urlencode(query, doseq=True)}"
        return urllib.request.Request(url, data=data, headers=headers, method=options.method)


class RemoteGraph:
    """Remote view of one graph managed by TongGraph Server."""

    def __init__(self, client: TongGraphClient, name: str) -> None:
        self.client = client
        self.name = name
        self.path = f"/graphs/{_quote(name)}"

    def snapshot(self, snapshot_id: str, metadata: dict[str, Any] | None = None) -> RemoteSnapshot:
        return RemoteSnapshot(self, snapshot_id, metadata=metadata)

    def create_snapshot(self, ttl_seconds: float = 600.0) -> RemoteSnapshot:
        metadata = self.client._request("POST", f"{self.path}/snapshots", {"ttl_seconds": ttl_seconds})["snapshot"]
        return RemoteSnapshot(self, metadata["snapshot_id"], metadata=metadata)

    def snapshots(self) -> list[dict[str, Any]]:
        return self.client._request("GET", f"{self.path}/snapshots")["snapshots"]

    list_snapshots = snapshots

    def delete_snapshot(self, snapshot_id: str) -> bool:
        return bool(self.client._request("DELETE", f"{self.path}/snapshots/{_quote(snapshot_id)}")["deleted"])

    def open(self) -> dict[str, Any]:
        return self.client._request("POST", f"{self.path}/open")

    def compact(self) -> dict[str, Any]:
        return self.client._request("POST", f"{self.path}/compact")

    def refresh(self) -> dict[str, Any]:
        return self.client._request("POST", f"{self.path}/refresh")

    def stats(self) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/stats")["stats"]

    def schema(self) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/schema")["schema"]

    def node_count(self) -> int:
        return int(self.client._request("GET", f"{self.path}/nodes/count")["count"])

    def edge_count(self) -> int:
        return int(self.client._request("GET", f"{self.path}/edges/count")["count"])

    def node_ids(self) -> list[int]:
        return self.client._request("GET", f"{self.path}/nodes")["ids"]

    def edge_ids(self) -> list[int]:
        return self.client._request("GET", f"{self.path}/edges")["ids"]

    def nodes_with_label(self, label: str) -> list[int]:
        return self.client._request("GET", f"{self.path}/nodes/by-label/{_quote(label)}")["ids"]

    def nodes_with_property(self, key: str, value: Any | None = None) -> list[int]:
        return self.client._request("GET", f"{self.path}/nodes/by-property", query={"key": key, "value": value})["ids"]

    def edges_by_type(self, edge_type: str) -> list[int]:
        return self.client._request("GET", f"{self.path}/edges/by-type/{_quote(edge_type)}")["ids"]

    def edges_with_property(self, key: str, value: Any | None = None) -> list[int]:
        return self.client._request("GET", f"{self.path}/edges/by-property", query={"key": key, "value": value})["ids"]

    def add_node(
        self,
        external_id: str | None = None,
        labels: list[str] | None = None,
        properties: dict[str, Any] | None = None,
    ) -> int:
        return int(
            self.client._request(
                "POST",
                f"{self.path}/nodes",
                {"external_id": external_id, "labels": labels, "properties": properties},
            )["id"]
        )

    def get_node(self, node_id: int) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/nodes/{node_id}")["node"]

    def update_node(
        self,
        node_id: int,
        external_id: str | None = None,
        add_labels: list[str] | None = None,
        remove_labels: list[str] | None = None,
        set_properties: dict[str, Any] | None = None,
        remove_properties: list[str] | None = None,
    ) -> dict[str, Any]:
        return self.client._request(
            "PATCH",
            f"{self.path}/nodes/{node_id}",
            {
                "external_id": external_id,
                "add_labels": add_labels,
                "remove_labels": remove_labels,
                "set_properties": set_properties,
                "remove_properties": remove_properties,
            },
        )["node"]

    def delete_node(self, node_id: int, detach: bool = False) -> bool:
        return bool(self.client._request("DELETE", f"{self.path}/nodes/{node_id}", query={"detach": detach})["deleted"])

    def add_edge(
        self,
        source: int,
        target: int,
        edge_type: str,
        properties: dict[str, Any] | None = None,
    ) -> int:
        return int(
            self.client._request(
                "POST",
                f"{self.path}/edges",
                {"source": source, "target": target, "edge_type": edge_type, "properties": properties},
            )["id"]
        )

    def get_edge(self, edge_id: int) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/edges/{edge_id}")["edge"]

    def update_edge(
        self,
        edge_id: int,
        set_properties: dict[str, Any] | None = None,
        remove_properties: list[str] | None = None,
    ) -> dict[str, Any]:
        return self.client._request(
            "PATCH",
            f"{self.path}/edges/{edge_id}",
            {"set_properties": set_properties, "remove_properties": remove_properties},
        )["edge"]

    def delete_edge(self, edge_id: int) -> bool:
        return bool(self.client._request("DELETE", f"{self.path}/edges/{edge_id}")["deleted"])

    def fulltext_indexes(self) -> list[dict[str, Any]]:
        return self.client._request("GET", f"{self.path}/fulltext/indexes")["indexes"]

    def create_fulltext_index(
        self,
        name: str,
        properties: list[str],
        target: str = "node",
        tokenizer: str = "unicode61",
    ) -> dict[str, Any]:
        return self.client._request(
            "POST",
            f"{self.path}/fulltext/indexes",
            {"name": name, "properties": properties, "target": target, "tokenizer": tokenizer},
        )

    def drop_fulltext_index(self, index: str) -> dict[str, Any]:
        return self.client._request("DELETE", f"{self.path}/fulltext/indexes/{_quote(index)}")

    def rebuild_fulltext_index(self, index: str) -> dict[str, Any]:
        return self.client._request("POST", f"{self.path}/fulltext/{_quote(index)}/rebuild")

    def search_text(
        self,
        index: str,
        query: str,
        mode: str = "all",
        labels: list[str] | None = None,
        edge_type: str | None = None,
        properties: dict[str, Any] | None = None,
        limit: int = 20,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        return self.client._request(
            "POST",
            f"{self.path}/fulltext/{_quote(index)}/search",
            {
                "query": query,
                "mode": mode,
                "labels": labels,
                "edge_type": edge_type,
                "properties": properties,
                "limit": limit,
                "offset": offset,
            },
        )["results"]

    def vector_indexes(self) -> list[dict[str, Any]]:
        return self.client._request("GET", f"{self.path}/vector/indexes")["indexes"]

    def create_vector_index(
        self,
        name: str,
        dimensions: int,
        target: str = "node",
        metric: str = "cosine",
        model: str | None = None,
        model_version: str | None = None,
    ) -> dict[str, Any]:
        return self.client._request(
            "POST",
            f"{self.path}/vector/indexes",
            {
                "name": name,
                "dimensions": dimensions,
                "target": target,
                "metric": metric,
                "model": model,
                "model_version": model_version,
            },
        )

    def drop_vector_index(self, index: str) -> dict[str, Any]:
        return self.client._request("DELETE", f"{self.path}/vector/indexes/{_quote(index)}")

    def upsert_vector(self, index: str, entity_id: int, vector: list[float]) -> bool:
        return bool(
            self.client._request("PUT", f"{self.path}/vector/{_quote(index)}/{entity_id}", {"vector": vector})[
                "upserted"
            ]
        )

    def get_vector(self, index: str, entity_id: int) -> list[float]:
        return self.client._request("GET", f"{self.path}/vector/{_quote(index)}/{entity_id}")["vector"]

    def delete_vector(self, index: str, entity_id: int) -> bool:
        return bool(self.client._request("DELETE", f"{self.path}/vector/{_quote(index)}/{entity_id}")["deleted"])

    def search_vector(
        self,
        index: str,
        query_vector: list[float],
        labels: list[str] | None = None,
        edge_type: str | None = None,
        properties: dict[str, Any] | None = None,
        min_score: float | None = None,
        limit: int = 20,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        return self.client._request(
            "POST",
            f"{self.path}/vector/{_quote(index)}/search",
            _vector_search_body(query_vector, labels, edge_type, properties, min_score, limit, offset),
        )["results"]

    def search_vectors(
        self,
        index: str,
        query_vectors: list[list[float]],
        labels: list[str] | None = None,
        edge_type: str | None = None,
        properties: dict[str, Any] | None = None,
        min_score: float | None = None,
        limit: int = 20,
        offset: int = 0,
    ) -> list[list[dict[str, Any]]]:
        return self.client._request(
            "POST",
            f"{self.path}/vector/{_quote(index)}/search-batch",
            {
                "query_vectors": query_vectors,
                "labels": labels,
                "edge_type": edge_type,
                "properties": properties,
                "min_score": min_score,
                "limit": limit,
                "offset": offset,
            },
        )["results"]

    def query(self, spec: dict[str, Any], profile: bool = False) -> Any:
        return self.client._request("POST", f"{self.path}/query", {"spec": spec, "profile": profile})["result"]

    def cypher(
        self,
        query: str,
        parameters: dict[str, Any] | None = None,
        profile: bool = False,
    ) -> dict[str, Any]:
        return self.client._request(
            "POST",
            f"{self.path}/cypher",
            {"query": query, "parameters": parameters, "profile": profile},
        )["result"]

    def cypher_transaction(self, statements: list[dict[str, Any]]) -> list[Any]:
        return self.client._request("POST", f"{self.path}/cypher/transaction", {"statements": statements})[
            "results"
        ]

    def neighbors(self, node_id: int, direction: str = "out", edge_type: str | None = None) -> list[int]:
        return self.client._request(
            "GET",
            f"{self.path}/traversal/neighbors/{node_id}",
            query={"direction": direction, "edge_type": edge_type},
        )["ids"]

    def k_hop(self, start: int, hops: int, direction: str = "out", edge_type: str | None = None) -> list[int]:
        return self.client._request(
            "GET",
            f"{self.path}/traversal/k-hop",
            query={"start": start, "hops": hops, "direction": direction, "edge_type": edge_type},
        )["ids"]

    def frontier(self, starts: list[int], steps: int, direction: str = "out", edge_type: str | None = None) -> list[int]:
        return self.client._request(
            "POST",
            f"{self.path}/traversal/frontier",
            {"starts": starts, "steps": steps, "direction": direction, "edge_type": edge_type},
        )["ids"]

    def bfs(
        self,
        start: int,
        direction: str = "out",
        edge_type: str | None = None,
        max_depth: int | None = None,
    ) -> list[int]:
        return self.client._request(
            "GET",
            f"{self.path}/algorithms/bfs",
            query={"start": start, "direction": direction, "edge_type": edge_type, "max_depth": max_depth},
        )["ids"]

    def shortest_path(
        self,
        start: int,
        target: int,
        direction: str = "out",
        edge_type: str | None = None,
        weight_property: str | None = None,
    ) -> dict[str, Any] | None:
        return self.client._request(
            "GET",
            f"{self.path}/algorithms/shortest-path",
            query={
                "start": start,
                "target": target,
                "direction": direction,
                "edge_type": edge_type,
                "weight_property": weight_property,
            },
        )["path"]

    def connected_components(self, edge_type: str | None = None) -> list[list[int]]:
        return self.client._request(
            "GET", f"{self.path}/algorithms/connected-components", query={"edge_type": edge_type}
        )["components"]

    def pagerank(
        self,
        iterations: int = 20,
        damping: float = 0.85,
        tolerance: float | None = None,
        edge_type: str | None = None,
    ) -> dict[str, float]:
        return self.client._request(
            "GET",
            f"{self.path}/algorithms/pagerank",
            query={"iterations": iterations, "damping": damping, "tolerance": tolerance, "edge_type": edge_type},
        )["scores"]

    def random_walk(
        self,
        start: int,
        steps: int,
        direction: str = "out",
        edge_type: str | None = None,
        seed: int | None = None,
    ) -> list[int]:
        return self.client._request(
            "GET",
            f"{self.path}/algorithms/random-walk",
            query={"start": start, "steps": steps, "direction": direction, "edge_type": edge_type, "seed": seed},
        )["ids"]

    def subgraph(self, nodes: list[int], edge_type: str | None = None) -> dict[str, Any]:
        return self.client._request("POST", f"{self.path}/subgraph", {"nodes": nodes, "edge_type": edge_type})[
            "snapshot"
        ]

    def compute_batch(self, jobs: list[dict[str, Any]]) -> list[Any]:
        return self.client._request("POST", f"{self.path}/compute/batch", {"jobs": jobs})["results"]

    def propagate(
        self,
        seeds: dict[int, float],
        steps: int,
        edge_property: str = "probability",
        damping: float = 1.0,
        edge_type: str | None = None,
    ) -> dict[str, float]:
        return self.client._request(
            "POST",
            f"{self.path}/propagate",
            {
                "seeds": seeds,
                "steps": steps,
                "edge_property": edge_property,
                "damping": damping,
                "edge_type": edge_type,
            },
        )["scores"]

    def local_propagate(
        self,
        seeds: dict[int, float],
        radius: int = 2,
        query_nodes: list[int] | None = None,
        edge_type: str | None = None,
        edge_property: str = "probability",
        damping: float = 1.0,
    ) -> dict[str, float]:
        return self.client._request(
            "POST",
            f"{self.path}/local-propagate",
            {
                "seeds": seeds,
                "radius": radius,
                "query_nodes": query_nodes,
                "edge_type": edge_type,
                "edge_property": edge_property,
                "damping": damping,
            },
        )["scores"]

    def add_variable(
        self,
        domain: str,
        owner_id: int | None = None,
        prior: dict[str, Any] | None = None,
        posterior: dict[str, Any] | None = None,
        states: list[str] | None = None,
    ) -> int:
        return int(
            self.client._request(
                "POST",
                f"{self.path}/variables",
                {
                    "domain": domain,
                    "owner_id": owner_id,
                    "prior": prior,
                    "posterior": posterior,
                    "states": states,
                },
            )["id"]
        )

    def get_variable(self, variable_id: int) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/variables/{variable_id}")["variable"]

    def posterior(self, variable_id: int) -> dict[str, float]:
        return self.client._request("GET", f"{self.path}/variables/{variable_id}/posterior")["posterior"]

    def add_factor(
        self,
        input_variables: list[int],
        output_variables: list[int],
        function: str,
        parameters: dict[str, Any] | None = None,
    ) -> int:
        return int(
            self.client._request(
                "POST",
                f"{self.path}/factors",
                {
                    "input_variables": input_variables,
                    "output_variables": output_variables,
                    "function": function,
                    "parameters": parameters,
                },
            )["id"]
        )

    def get_factor(self, factor_id: int) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/factors/{factor_id}")["factor"]

    def add_factor_table(self, variables: list[int], values: list[float]) -> int:
        return int(
            self.client._request(
                "POST",
                f"{self.path}/factor-tables",
                {"variables": variables, "values": values},
            )["id"]
        )

    def add_cpd(self, variable_id: int, parent_variables: list[int], values: list[float]) -> int:
        return int(
            self.client._request(
                "POST",
                f"{self.path}/cpds",
                {"variable_id": variable_id, "parent_variables": parent_variables, "values": values},
            )["id"]
        )

    def add_evidence(self, variable_id: int, payload: dict[str, Any] | None = None) -> int:
        return int(
            self.client._request(
                "POST",
                f"{self.path}/evidence",
                {"variable_id": variable_id, "payload": payload},
            )["id"]
        )

    def get_evidence(self, evidence_id: int) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/evidence/{evidence_id}")["evidence"]

    def add_trace(self, payload: dict[str, Any] | None = None) -> int:
        return int(self.client._request("POST", f"{self.path}/traces", {"payload": payload})["id"])

    def get_trace(self, trace_id: int) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/traces/{trace_id}")["trace"]

    def compile_active_subgraph(
        self,
        query_variables: list[int],
        evidence: dict[int, str] | None = None,
        radius: int = 2,
        max_nodes: int = 10000,
        max_factors: int = 50000,
    ) -> dict[str, Any]:
        return self.client._request(
            "POST",
            f"{self.path}/inference/active-subgraph",
            {
                "query_variables": query_variables,
                "evidence": evidence,
                "radius": radius,
                "max_nodes": max_nodes,
                "max_factors": max_factors,
            },
        )["active_subgraph"]

    def belief_propagation(
        self,
        query_variables: list[int] | None = None,
        evidence: dict[int, str] | None = None,
        radius: int = 2,
        max_iters: int = 1000,
        tolerance: float = 1e-6,
        damping: float = 0.2,
        persist: bool = False,
    ) -> dict[str, Any]:
        return self.client._request(
            "POST",
            f"{self.path}/belief-propagation",
            {
                "query_variables": query_variables,
                "evidence": evidence,
                "radius": radius,
                "max_iters": max_iters,
                "tolerance": tolerance,
                "damping": damping,
                "persist": persist,
            },
        )["result"]



class RemoteSnapshot:
    """Read-only remote snapshot resource."""

    def __init__(self, graph: RemoteGraph, snapshot_id: str, metadata: dict[str, Any] | None = None) -> None:
        self.graph = graph
        self.client = graph.client
        self.snapshot_id = snapshot_id
        self.metadata = dict(metadata or {})
        self.path = f"{graph.path}/snapshots/{_quote(snapshot_id)}"

    def delete(self) -> bool:
        return self.graph.delete_snapshot(self.snapshot_id)

    def stats(self) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/stats")["stats"]

    def schema(self) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/schema")["schema"]

    def node_count(self) -> int:
        return int(self.client._request("GET", f"{self.path}/nodes/count")["count"])

    def edge_count(self) -> int:
        return int(self.client._request("GET", f"{self.path}/edges/count")["count"])

    def node_ids(self) -> list[int]:
        return self.client._request("GET", f"{self.path}/nodes")["ids"]

    def edge_ids(self) -> list[int]:
        return self.client._request("GET", f"{self.path}/edges")["ids"]

    def get_node(self, node_id: int) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/nodes/{node_id}")["node"]

    def get_edge(self, edge_id: int) -> dict[str, Any]:
        return self.client._request("GET", f"{self.path}/edges/{edge_id}")["edge"]

    def query(self, spec: dict[str, Any], profile: bool = False) -> Any:
        return self.client._request("POST", f"{self.path}/query", {"spec": spec, "profile": profile})["result"]

    def cypher(
        self,
        query: str,
        parameters: dict[str, Any] | None = None,
        profile: bool = False,
    ) -> dict[str, Any]:
        return self.client._request(
            "POST", f"{self.path}/cypher", {"query": query, "parameters": parameters, "profile": profile}
        )["result"]

    def compute_batch(self, jobs: list[dict[str, Any]]) -> list[Any]:
        return self.client._request("POST", f"{self.path}/compute/batch", {"jobs": jobs})["results"]

    def search_vectors(
        self,
        index: str,
        query_vectors: list[list[float]],
        labels: list[str] | None = None,
        edge_type: str | None = None,
        properties: dict[str, Any] | None = None,
        min_score: float | None = None,
        limit: int = 20,
        offset: int = 0,
    ) -> list[list[dict[str, Any]]]:
        return self.client._request(
            "POST",
            f"{self.path}/vector/{_quote(index)}/search-batch",
            {
                "query_vectors": query_vectors,
                "labels": labels,
                "edge_type": edge_type,
                "properties": properties,
                "min_score": min_score,
                "limit": limit,
                "offset": offset,
            },
        )["results"]


def _quote(value: str) -> str:
    return urllib.parse.quote(str(value), safe="")


def _clean_query(query: dict[str, Any]) -> dict[str, Any]:
    cleaned: dict[str, Any] = {}
    for key, value in query.items():
        if value is None:
            continue
        if isinstance(value, bool):
            cleaned[key] = "true" if value else "false"
        else:
            cleaned[key] = value
    return cleaned


def _decode_response(data: bytes) -> Any:
    if not data:
        return None
    return json.loads(data.decode("utf-8"))


def _error_from_http(exc: urllib.error.HTTPError) -> TongGraphServerError:
    try:
        payload = _decode_response(exc.read())
    except Exception:
        payload = None
    error = payload.get("error") if isinstance(payload, dict) else None
    if isinstance(error, dict):
        return TongGraphServerError(
            str(error.get("code") or "server_error"),
            str(error.get("message") or exc.reason),
            status_code=exc.code,
            graph=error.get("graph"),
            request_id=error.get("request_id"),
        )
    return TongGraphServerError("server_error", str(exc.reason), status_code=exc.code)


def _vector_search_body(
    query_vector: list[float],
    labels: list[str] | None,
    edge_type: str | None,
    properties: dict[str, Any] | None,
    min_score: float | None,
    limit: int,
    offset: int,
) -> dict[str, Any]:
    return {
        "query_vector": query_vector,
        "labels": labels,
        "edge_type": edge_type,
        "properties": properties,
        "min_score": min_score,
        "limit": limit,
        "offset": offset,
    }


__all__ = ["TongGraphClient", "TongGraphServerError", "RemoteGraph", "RemoteSnapshot"]
