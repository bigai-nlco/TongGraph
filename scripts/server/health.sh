#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${TONGGRAPH_BASE_URL:-http://127.0.0.1:8719}"
URL="${BASE_URL%/}/health"

python3 - "$URL" <<'PY'
import json
import sys
import urllib.request

url = sys.argv[1]
with urllib.request.urlopen(url, timeout=5) as response:
    payload = json.loads(response.read().decode("utf-8"))
if payload.get("status") != "ok":
    raise SystemExit(f"unexpected health payload: {payload!r}")
print(json.dumps(payload, sort_keys=True))
PY
