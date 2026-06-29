from __future__ import annotations

from pathlib import Path

from fastapi import APIRouter, Request

from ..access import require_graph_access
from ..errors import ServerError
from ..schemas import ExportEdgesRequest, ExportNodesRequest, ExportRowsRequest, ImportPathRequest

router = APIRouter(prefix="/graphs/{graph}")


def _scoped_path(request: Request, scope: str, raw_path: str) -> Path:
    path = Path(raw_path)
    if path.is_absolute():
        raise ServerError("invalid_request", "path must be relative")
    root = (request.app.state.config.data_dir / scope).resolve()
    candidate = (root / path).resolve()
    if root != candidate and root not in candidate.parents:
        raise ServerError("invalid_request", f"path must stay under data_dir/{scope}")
    return candidate


def _import_path(request: Request, raw_path: str) -> Path:
    path = _scoped_path(request, "imports", raw_path)
    if not path.exists():
        raise ServerError("invalid_request", f"import path {raw_path!r} does not exist")
    if not path.is_file():
        raise ServerError("invalid_request", f"import path {raw_path!r} is not a file")
    return path


def _export_path(request: Request, raw_path: str) -> Path:
    path = _scoped_path(request, "exports", raw_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    return path


@router.post("/import/nodes/csv")
async def import_nodes_csv(request: Request, graph: str, payload: ImportPathRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    path = _import_path(request, payload.path)
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.import_nodes_csv(path))}


@router.post("/import/edges/csv")
async def import_edges_csv(request: Request, graph: str, payload: ImportPathRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    path = _import_path(request, payload.path)
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.import_edges_csv(path))}


@router.post("/import/nodes/jsonl")
async def import_nodes_jsonl(request: Request, graph: str, payload: ImportPathRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    path = _import_path(request, payload.path)
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.import_nodes_jsonl(path))}


@router.post("/import/edges/jsonl")
async def import_edges_jsonl(request: Request, graph: str, payload: ImportPathRequest) -> dict[str, object]:
    require_graph_access(request, graph, "write")
    path = _import_path(request, payload.path)
    return {"ids": request.app.state.registry.call(graph, lambda graph_obj: graph_obj.import_edges_jsonl(path))}


@router.post("/export/nodes/jsonl")
async def export_nodes_jsonl(request: Request, graph: str, payload: ExportNodesRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    path = _export_path(request, payload.path)
    request.app.state.registry.call(graph, lambda graph_obj: graph_obj.export_nodes_jsonl(path, payload.nodes))
    return {"exported": True, "path": payload.path}


@router.post("/export/edges/jsonl")
async def export_edges_jsonl(request: Request, graph: str, payload: ExportEdgesRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    path = _export_path(request, payload.path)
    request.app.state.registry.call(graph, lambda graph_obj: graph_obj.export_edges_jsonl(path, payload.edges))
    return {"exported": True, "path": payload.path}


@router.post("/export/query-rows/jsonl")
async def export_query_rows_jsonl(request: Request, graph: str, payload: ExportRowsRequest) -> dict[str, object]:
    require_graph_access(request, graph, "read")
    path = _export_path(request, payload.path)
    request.app.state.registry.call(graph, lambda graph_obj: graph_obj.export_query_rows_jsonl(path, payload.rows))
    return {"exported": True, "path": payload.path}
