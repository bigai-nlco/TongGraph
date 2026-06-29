#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="${TONGGRAPH_ENV_FILE:-$ROOT_DIR/deploy/tonggraph-server.env}"

if [ -f "$ENV_FILE" ]; then
  set -a
  . "$ENV_FILE"
  set +a
fi

BASE_URL="${TONGGRAPH_BASE_URL:-http://127.0.0.1:8719}"
ADMIN_TOKEN="${TONGGRAPH_ADMIN_TOKEN:?set TONGGRAPH_ADMIN_TOKEN before running smoke test}"
GRAPH="${TONGGRAPH_SMOKE_GRAPH:-smoke_test}"
READER="${TONGGRAPH_SMOKE_READER:-smoke_reader}"
READER_TOKEN="${TONGGRAPH_SMOKE_READER_TOKEN:-tonggraph-smoke-reader-token}"

python3 - "$BASE_URL" "$ADMIN_TOKEN" "$GRAPH" "$READER" "$READER_TOKEN" <<'PY'
import json
import sys
import urllib.error
import urllib.request
import uuid

base_url, admin_token, graph, reader, reader_token = sys.argv[1:]
base_url = base_url.rstrip("/")


def request(method, path, token=None, body=None, expected=(200,)):
    data = None if body is None else json.dumps(body).encode("utf-8")
    headers = {}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if body is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(f"{base_url}{path}", data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=10) as response:
            payload = response.read().decode("utf-8")
            status = response.status
    except urllib.error.HTTPError as exc:
        payload = exc.read().decode("utf-8")
        status = exc.code
    if status not in expected:
        raise SystemExit(f"{method} {path} expected {expected}, got {status}: {payload}")
    return json.loads(payload) if payload else None


health = request("GET", "/health")
if health.get("status") != "ok":
    raise SystemExit(f"unexpected health payload: {health!r}")

created_graph = request("POST", "/admin/graphs", admin_token, {"name": graph}, expected=(200, 409))
if created_graph and "error" in created_graph and created_graph["error"].get("code") != "conflict":
    raise SystemExit(f"unexpected graph create response: {created_graph!r}")
request("POST", "/admin/users", admin_token, {"user_id": reader, "token": reader_token, "graphs": {graph: "read"}}, expected=(200, 409))
request("PATCH", f"/admin/users/{reader}", admin_token, {"disabled": False, "graphs": {graph: "read"}})
request("POST", f"/admin/graphs/{graph}/grants", admin_token, {"user": reader, "access": "read"})

external_id = f"smoke-node-{uuid.uuid4().hex}"
node = request("POST", f"/graphs/{graph}/nodes", admin_token, {"external_id": external_id, "labels": ["Smoke"], "properties": {"ok": True}})
node_id = node["id"]
count = request("GET", f"/graphs/{graph}/nodes/count", reader_token)
if count["count"] < 1:
    raise SystemExit(f"unexpected node count: {count!r}")
request("GET", f"/graphs/{graph}/nodes/{node_id}", reader_token)
request("POST", f"/graphs/{graph}/nodes", reader_token, {"external_id": "blocked"}, expected=(403,))
print(json.dumps({"status": "ok", "graph": graph, "node_id": node_id}, sort_keys=True))
PY
