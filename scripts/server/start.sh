#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG_FILE="${TONGGRAPH_CONFIG:-$ROOT_DIR/deploy/tonggraph-server.yml}"

if [ ! -f "$CONFIG_FILE" ]; then
  echo "TongGraph config not found: $CONFIG_FILE" >&2
  exit 1
fi

args=(--config "$CONFIG_FILE")
if [ -n "${TONGGRAPH_HOST:-}" ]; then
  args+=(--host "$TONGGRAPH_HOST")
fi
if [ -n "${TONGGRAPH_PORT:-}" ]; then
  args+=(--port "$TONGGRAPH_PORT")
fi

cd "$ROOT_DIR"
if [ "${TONGGRAPH_USE_UV:-auto}" != "0" ] && command -v uv >/dev/null 2>&1 && [ -f "$ROOT_DIR/uv.lock" ]; then
  exec uv run tonggraph-server "${args[@]}"
fi
exec "${TONGGRAPH_SERVER_BIN:-tonggraph-server}" "${args[@]}"
