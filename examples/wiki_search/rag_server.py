#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
from pathlib import Path
from typing import Any

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

from wiki_search.retriever import WikiGraphRetriever


retriever: WikiGraphRetriever | None = None


class RetrieveRequest(BaseModel):
    query: str = Field(min_length=1)
    top_k: int = Field(default=5, ge=1, le=100)
    expand_hops: int = Field(default=1, ge=0, le=3)


class BatchRetrieveRequest(BaseModel):
    queries: list[str] = Field(min_length=1, max_length=256)
    top_k: int = Field(default=5, ge=1, le=100)
    expand_hops: int = Field(default=1, ge=0, le=3)


app = FastAPI(title="TongGraph Wiki RAG Server")


@app.on_event("startup")
async def startup_event() -> None:
    global retriever
    if retriever is not None:
        return
    db_path = os.environ.get("WIKI_GRAPH_DB")
    if not db_path:
        return
    retriever = WikiGraphRetriever(
        Path(db_path),
        embedding_backend=os.environ.get("WIKI_EMBEDDING_BACKEND", "hash"),
        embedding_model=os.environ.get("WIKI_EMBEDDING_MODEL") or None,
    )


@app.get("/health")
def health() -> dict[str, Any]:
    if retriever is None:
        raise HTTPException(status_code=503, detail="retriever is not initialized")
    stats = retriever.stats()
    return {
        "status": "healthy",
        "db_path": stats["db_path"],
        "nodes": stats["nodes"],
        "edges": stats["edges"],
        "embedding_backend": stats["embedding_backend"],
        "embedding_model": stats["embedding_model"],
        "embedding_dimensions": stats["embedding_dimensions"],
    }


@app.get("/stats")
def stats() -> dict[str, Any]:
    if retriever is None:
        raise HTTPException(status_code=503, detail="retriever is not initialized")
    return retriever.stats()


@app.post("/retrieve")
def retrieve(request: RetrieveRequest) -> dict[str, Any]:
    if retriever is None:
        raise HTTPException(status_code=503, detail="retriever is not initialized")
    try:
        results = retriever.search(
            request.query,
            top_k=request.top_k,
            expand_hops=request.expand_hops,
        )
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc)) from exc
    return {
        "query": request.query,
        "method": "tonggraph_vector",
        "results": results,
        "num_results": len(results),
    }


@app.post("/retrieve_batch")
def retrieve_batch(request: BatchRetrieveRequest) -> dict[str, Any]:
    if retriever is None:
        raise HTTPException(status_code=503, detail="retriever is not initialized")
    try:
        results = retriever.search_batch(
            request.queries,
            top_k=request.top_k,
            expand_hops=request.expand_hops,
        )
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc)) from exc
    return {
        "queries": request.queries,
        "method": "tonggraph_vector_batch",
        "results": results,
        "num_results": [len(batch) for batch in results],
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Serve a TongGraph wiki RAG database.")
    parser.add_argument("--db-path", type=Path, default=Path("search_data/wiki_graph.db"))
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=9002)
    parser.add_argument(
        "--embedding-backend",
        choices=["hash", "sentence-transformers"],
        default="hash",
    )
    parser.add_argument("--embedding-model", help="Optional sentence-transformers model name.")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    global retriever
    retriever = WikiGraphRetriever(
        args.db_path,
        embedding_backend=args.embedding_backend,
        embedding_model=args.embedding_model,
    )
    import uvicorn

    uvicorn.run(app, host=args.host, port=args.port, log_level="info")


if __name__ == "__main__":
    main()
