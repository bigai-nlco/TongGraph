"""Command-line entry point for TongGraph server."""

from __future__ import annotations

import argparse

import uvicorn

from .app import create_app
from .config import load_config


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="tonggraph-server")
    parser.add_argument("--config", "-c", help="Path to YAML or JSON server config")
    parser.add_argument("--host", help="Override configured host")
    parser.add_argument("--port", type=int, help="Override configured port")
    args = parser.parse_args(argv)
    config = load_config(args.config)
    host = args.host or config.host
    port = args.port or config.port
    uvicorn.run(create_app(config), host=host, port=port)
    return 0
