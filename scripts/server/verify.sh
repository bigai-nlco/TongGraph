#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="${TONGGRAPH_ENV_FILE:-$ROOT_DIR/deploy/tonggraph-server.env}"

if [ -f "$ENV_FILE" ]; then
  set -a
  . "$ENV_FILE"
  set +a
fi

CONFIG_FILE="${TONGGRAPH_CONFIG:-$ROOT_DIR/deploy/tonggraph-server.yml}"
if [ ! -f "$CONFIG_FILE" ]; then
  echo "TongGraph config not found: $CONFIG_FILE" >&2
  exit 1
fi

if [ -n "${TONGGRAPH_PYTHON:-}" ]; then
  PYTHON_BIN="$TONGGRAPH_PYTHON"
elif [ -x "$ROOT_DIR/.venv/bin/python" ]; then
  PYTHON_BIN="$ROOT_DIR/.venv/bin/python"
else
  PYTHON_BIN="python3"
fi

cd "$ROOT_DIR"
exec "$PYTHON_BIN" - "$CONFIG_FILE" <<'PYVERIFY'
from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid
from typing import Any

from tonggraph.server.config import load_config

config_path = sys.argv[1]
config = load_config(config_path)
base_url = os.environ.get("TONGGRAPH_BASE_URL") or f"http://127.0.0.1:{config.port}"
base_url = base_url.rstrip("/")


def access_rank(access: str | None) -> int:
    return {None: 0, "read": 1, "write": 2}.get(access, 0)


def grant_for(user: Any, graph: str) -> str | None:
    return user.graphs.get(graph) or user.graphs.get("*")


def first_token(*, admin: bool | None = None, graph: str | None = None, min_access: str | None = None, exact_access: str | None = None) -> tuple[str | None, str | None]:
    required = access_rank(min_access)
    for user_id, user in config.users.items():
        if admin is not None and bool(user.admin) != admin:
            continue
        if not user.token:
            continue
        if graph is not None:
            access = grant_for(user, graph)
            if exact_access is not None and access != exact_access:
                continue
            if min_access is not None and access_rank(access) < required:
                continue
        return user_id, user.token
    return None, None


def choose_graph() -> str:
    requested = os.environ.get("TONGGRAPH_VERIFY_GRAPH")
    if requested:
        return requested
    for name, enabled in config.graph_logical_graphs.items():
        if enabled:
            return name
    if config.graphs:
        return next(iter(config.graphs))
    raise SystemExit("No graph configured; set TONGGRAPH_VERIFY_GRAPH")


graph = choose_graph()
logical_enabled = bool(config.graph_logical_graphs.get(graph, False))
logical_graph_id = os.environ.get("TONGGRAPH_VERIFY_LOGICAL_GRAPH") or f"verify_{uuid.uuid4().hex[:12]}"
admin_user, admin_token = first_token(admin=True)
writer_user, writer_token = first_token(admin=False, graph=graph, min_access="write")
reader_user, reader_token = first_token(admin=False, graph=graph, exact_access="read")
admin_token = os.environ.get("TONGGRAPH_VERIFY_ADMIN_TOKEN") or admin_token
writer_token = os.environ.get("TONGGRAPH_VERIFY_WRITER_TOKEN") or writer_token or admin_token
reader_token = os.environ.get("TONGGRAPH_VERIFY_READER_TOKEN") or reader_token

if not admin_token:
    raise SystemExit("No admin token found. Set TONGGRAPH_ADMIN_TOKEN or TONGGRAPH_VERIFY_ADMIN_TOKEN.")
if not writer_token:
    raise SystemExit("No writer token found. Set a write user token or TONGGRAPH_VERIFY_WRITER_TOKEN.")

summary: list[dict[str, Any]] = []
created_nodes: list[int] = []
created_edges: list[int] = []


def log(step: str, **payload: Any) -> None:
    item = {"step": step, **payload}
    summary.append(item)
    print(json.dumps(item, ensure_ascii=False, sort_keys=True))


def request(method: str, path: str, *, token: str | None = None, body: dict[str, Any] | None = None, query: dict[str, Any] | None = None, expected: tuple[int, ...] = (200,)) -> Any:
    url = f"{base_url}{path}"
    if query:
        clean = {key: value for key, value in query.items() if value is not None}
        if clean:
            url = f"{url}?{urllib.parse.urlencode(clean, doseq=True)}"
    data = None if body is None else json.dumps(body).encode("utf-8")
    headers = {"Accept": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if body is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=float(os.environ.get("TONGGRAPH_VERIFY_TIMEOUT", "15"))) as response:
            raw = response.read().decode("utf-8")
            status = response.status
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode("utf-8")
        status = exc.code
    except urllib.error.URLError as exc:
        raise SystemExit(f"{method} {url} failed to connect: {exc}") from exc
    if status not in expected:
        raise SystemExit(f"{method} {url} expected {expected}, got {status}: {raw}")
    return json.loads(raw) if raw else None


def expect_error(method: str, path: str, code: str, *, token: str | None = None, body: dict[str, Any] | None = None, query: dict[str, Any] | None = None, statuses: tuple[int, ...] = (400, 401, 403, 404, 409)) -> None:
    payload = request(method, path, token=token, body=body, query=query, expected=statuses)
    got = payload.get("error", {}).get("code") if isinstance(payload, dict) else None
    if got != code:
        raise SystemExit(f"{method} {path} expected error {code!r}, got {payload!r}")


def scoped_body(body: dict[str, Any]) -> dict[str, Any]:
    if logical_enabled:
        return {"logical_graph_id": logical_graph_id, **body}
    return body


def scoped_query(query: dict[str, Any] | None = None) -> dict[str, Any]:
    out = dict(query or {})
    if logical_enabled:
        out["logical_graph_id"] = logical_graph_id
    return out


health = request("GET", "/health")
if health.get("status") != "ok":
    raise SystemExit(f"unexpected health payload: {health!r}")
log("health", status=health.get("status"), base_url=base_url, graph=graph, logical_graphs=logical_enabled)

request("GET", "/graphs", token=writer_token)
request("GET", "/admin/graphs", token=admin_token)
log("auth", admin_user=admin_user, writer_user=writer_user, reader_user=reader_user)

if logical_enabled:
    request("POST", f"/graphs/{urllib.parse.quote(graph)}/logical-graphs", token=writer_token, body={"logical_graph_id": logical_graph_id}, expected=(200, 409))
    log("logical_graph", logical_graph_id=logical_graph_id, response="created_or_exists")
    expect_error("GET", f"/graphs/{urllib.parse.quote(graph)}/nodes/count", "logical_graph_required", token=writer_token, statuses=(400,))

external_a = f"verify:{logical_graph_id}:alice:{uuid.uuid4().hex}"
external_b = f"verify:{logical_graph_id}:bob:{uuid.uuid4().hex}"
node_a = request(
    "POST",
    f"/graphs/{urllib.parse.quote(graph)}/nodes",
    token=writer_token,
    body=scoped_body({"external_id": external_a, "labels": ["Verify", "Person"], "properties": {"name": "Alice", "text": "alpha verification memory"}}),
)["id"]
node_b = request(
    "POST",
    f"/graphs/{urllib.parse.quote(graph)}/nodes",
    token=writer_token,
    body=scoped_body({"external_id": external_b, "labels": ["Verify", "Person"], "properties": {"name": "Bob", "text": "beta verification memory"}}),
)["id"]
created_nodes.extend([node_a, node_b])
edge = request(
    "POST",
    f"/graphs/{urllib.parse.quote(graph)}/edges",
    token=writer_token,
    body=scoped_body({"source": node_a, "target": node_b, "edge_type": "VERIFY_LINK", "properties": {"weight": 1.0, "note": "verification edge"}}),
)["id"]
created_edges.append(edge)
log("records", nodes=created_nodes, edge=edge)

count = request("GET", f"/graphs/{urllib.parse.quote(graph)}/nodes/count", token=writer_token, query=scoped_query())
if count["count"] < 2:
    raise SystemExit(f"unexpected scoped node count: {count}")
node = request("GET", f"/graphs/{urllib.parse.quote(graph)}/nodes/{node_a}", token=writer_token, query=scoped_query())["node"]
if node["external_id"] != external_a:
    raise SystemExit(f"unexpected node payload: {node!r}")
lookup = request("GET", f"/graphs/{urllib.parse.quote(graph)}/nodes/by-external-id/{urllib.parse.quote(external_a)}", token=writer_token, query=scoped_query())
if lookup["id"] != node_a:
    raise SystemExit(f"external_id lookup failed: {lookup!r}")

request("POST", f"/graphs/{urllib.parse.quote(graph)}/fulltext/indexes", token=writer_token, body={"name": "verify_text", "properties": ["text", "name"], "target": "node"}, expected=(200, 400, 409))
text_rows = request("POST", f"/graphs/{urllib.parse.quote(graph)}/fulltext/verify_text/search", token=writer_token, body=scoped_body({"query": "alpha verification", "labels": ["Verify"], "limit": 5}))["results"]
if not any(row["id"] == node_a for row in text_rows):
    raise SystemExit(f"fulltext search did not find node {node_a}: {text_rows!r}")

request("POST", f"/graphs/{urllib.parse.quote(graph)}/vector/indexes", token=writer_token, body={"name": "verify_vec", "dimensions": 3, "target": "node", "metric": "cosine"}, expected=(200, 400, 409))
request("PUT", f"/graphs/{urllib.parse.quote(graph)}/vector/verify_vec/batch", token=writer_token, body=scoped_body({"vectors": {str(node_a): [1.0, 0.0, 0.0], str(node_b): [0.7, 0.2, 0.0]}}))
vec_rows = request("POST", f"/graphs/{urllib.parse.quote(graph)}/vector/verify_vec/search", token=writer_token, body=scoped_body({"query_vector": [1.0, 0.0, 0.0], "labels": ["Verify"], "limit": 2}))["results"]
if not vec_rows or vec_rows[0]["id"] != node_a:
    raise SystemExit(f"vector search unexpected results: {vec_rows!r}")
context = request("POST", f"/graphs/{urllib.parse.quote(graph)}/retrieve/context", token=writer_token, body=scoped_body({"text_index": "verify_text", "text_query": "alpha verification", "vector_index": "verify_vec", "vector_query": [1.0, 0.0, 0.0], "labels": ["Verify"], "radius": 1, "limit": 5}))["results"]
if not any(row.get("record", {}).get("id") == node_a for row in context):
    raise SystemExit(f"retrieve_context missing node {node_a}: {context!r}")
log("retrieval", fulltext=len(text_rows), vector=len(vec_rows), context=len(context))

query_rows = request("POST", f"/graphs/{urllib.parse.quote(graph)}/query", token=writer_token, body=scoped_body({"spec": {"match": [{"node": "n", "external_id": external_a}], "return": ["n"]}}))["result"]
if query_rows != [{"n": node_a}]:
    raise SystemExit(f"query failed: {query_rows!r}")
neighbors = request("GET", f"/graphs/{urllib.parse.quote(graph)}/traversal/neighbors/{node_a}", token=writer_token, query=scoped_query({"direction": "out"}))["ids"]
if node_b not in neighbors:
    raise SystemExit(f"neighbors missing node {node_b}: {neighbors!r}")
bfs = request("GET", f"/graphs/{urllib.parse.quote(graph)}/algorithms/bfs", token=writer_token, query=scoped_query({"start": node_a, "max_depth": 1}))["ids"]
if bfs[:2] != [node_a, node_b]:
    raise SystemExit(f"bfs unexpected: {bfs!r}")
log("query_compute", query_rows=len(query_rows), neighbors=len(neighbors), bfs=len(bfs))

snapshot = request("POST", f"/graphs/{urllib.parse.quote(graph)}/snapshots", token=writer_token, body=scoped_body({"ttl_seconds": 120}))["snapshot"]
request("POST", f"/graphs/{urllib.parse.quote(graph)}/nodes", token=writer_token, body=scoped_body({"external_id": f"verify:{logical_graph_id}:later:{uuid.uuid4().hex}", "labels": ["Verify"], "properties": {"name": "Later"}}))
snapshot_count = request("GET", f"/graphs/{urllib.parse.quote(graph)}/snapshots/{snapshot['snapshot_id']}/nodes/count", token=writer_token)["count"]
if snapshot_count < 2:
    raise SystemExit(f"snapshot count too small: {snapshot_count}")
request("DELETE", f"/graphs/{urllib.parse.quote(graph)}/snapshots/{snapshot['snapshot_id']}", token=writer_token)
log("snapshot", snapshot_id=snapshot["snapshot_id"], node_count=snapshot_count)

if reader_token:
    request("GET", f"/graphs/{urllib.parse.quote(graph)}/nodes/{node_a}", token=reader_token, query=scoped_query())
    expect_error("POST", f"/graphs/{urllib.parse.quote(graph)}/nodes", "permission_denied", token=reader_token, body=scoped_body({"external_id": f"verify:blocked:{uuid.uuid4().hex}"}), statuses=(403,))
    log("read_only", checked=True)
else:
    log("read_only", checked=False, reason="no read-only token found")

metrics = request("GET", "/metrics", token=admin_token)
if "requests" not in metrics:
    raise SystemExit(f"unexpected metrics payload: {metrics!r}")
log("metrics", ok=True)

if logical_enabled and os.environ.get("TONGGRAPH_VERIFY_KEEP_DATA", "0") != "1":
    request("DELETE", f"/graphs/{urllib.parse.quote(graph)}/logical-graphs/{urllib.parse.quote(logical_graph_id)}", token=writer_token)
    log("cleanup", logical_graph_id=logical_graph_id, deleted=True)
else:
    log("cleanup", kept=True)

print(json.dumps({"status": "ok", "base_url": base_url, "graph": graph, "logical_graph_id": logical_graph_id, "steps": len(summary)}, ensure_ascii=False, sort_keys=True))
PYVERIFY
