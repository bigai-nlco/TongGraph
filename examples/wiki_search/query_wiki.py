#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

from wiki_search.retriever import WikiGraphRetriever


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Query a TongGraph wiki vector DB.")
    parser.add_argument("query", help="Natural-language search query.")
    parser.add_argument("--db-path", type=Path, default=Path("search_data/wiki_graph.db"))
    parser.add_argument(
        "--embedding-backend",
        choices=["hash", "sentence-transformers"],
        default="hash",
    )
    parser.add_argument("--embedding-model", help="Optional sentence-transformers model name.")
    parser.add_argument("--top-k", type=int, default=5)
    parser.add_argument("--expand-hops", type=int, default=1)
    parser.add_argument(
        "--max-text-chars",
        type=int,
        default=1200,
        help="Trim each result text for CLI output. Use 0 for full text.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    retriever = WikiGraphRetriever(
        args.db_path,
        embedding_backend=args.embedding_backend,
        embedding_model=args.embedding_model,
    )
    results = retriever.search(
        args.query,
        top_k=args.top_k,
        expand_hops=args.expand_hops,
    )
    if args.max_text_chars > 0:
        for result in results:
            text = str(result.get("text", ""))
            if len(text) > args.max_text_chars:
                result["text"] = text[: args.max_text_chars].rstrip() + "..."

    payload = {
        "query": args.query,
        "results": results,
    }
    print(json.dumps(payload, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
