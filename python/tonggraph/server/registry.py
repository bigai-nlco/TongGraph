"""Graph registry and server control-plane persistence."""

from __future__ import annotations

import json
import os
import queue
import threading
import time
import uuid
from concurrent.futures import Future
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, TypeVar

from tonggraph import Graph

from .config import ServerConfig, default_graph_path, validate_graph_name
from .errors import ServerError

T = TypeVar("T")
DEFAULT_SNAPSHOT_TTL_SECONDS = 600.0
MAX_SNAPSHOT_TTL_SECONDS = 3600.0


@dataclass
class SnapshotEntry:
    snapshot_id: str
    owner_user_id: str
    created_at: float
    expires_at: float
    snapshot: Any

    def metadata(self) -> dict[str, Any]:
        return {
            "snapshot_id": self.snapshot_id,
            "owner_user_id": self.owner_user_id,
            "created_at": self.created_at,
            "expires_at": self.expires_at,
            "ttl_seconds": max(0.0, self.expires_at - time.time()),
        }


@dataclass
class GraphWorkerContext:
    graph: Graph
    snapshots: dict[str, SnapshotEntry]


class GraphWorker:
    """Own one PyO3 Graph on one dedicated Python thread."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self._tasks: queue.Queue[tuple[Callable[[GraphWorkerContext], Any], Future[Any]] | None] = queue.Queue()
        self._ready: Future[None] = Future()
        self._thread = threading.Thread(target=self._run, name=f"tonggraph:{path.name}", daemon=True)
        self._thread.start()
        self._ready.result()

    def call(self, func: Callable[[Graph], T]) -> T:
        return self.call_context(lambda context: func(context.graph))

    def call_context(self, func: Callable[[GraphWorkerContext], T]) -> T:
        future: Future[T] = Future()
        self._tasks.put((func, future))
        return future.result()

    def close(self) -> None:
        self._tasks.put(None)
        self._thread.join(timeout=5)

    def __del__(self) -> None:
        if getattr(self, "_thread", None) is not None and self._thread.is_alive():
            try:
                self.close()
            except Exception:
                pass

    def _run(self) -> None:
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            graph = Graph(str(self.path))
            context = GraphWorkerContext(graph=graph, snapshots={})
            self._ready.set_result(None)
        except Exception as exc:  # pragma: no cover - startup failure path.
            self._ready.set_exception(exc)
            return
        while True:
            task = self._tasks.get()
            if task is None:
                context.snapshots.clear()
                break
            func, future = task
            try:
                future.set_result(func(context))
            except BaseException as exc:
                future.set_exception(exc.with_traceback(None))


@dataclass
class GraphEntry:
    name: str
    path: Path
    worker: GraphWorker | None = None
    created_by: str | None = None


class GraphRegistry:
    def __init__(self, config: ServerConfig) -> None:
        self.config = config
        self._lock = threading.RLock()
        self.state_path = config.data_dir / "server-state.json"
        self.graphs: dict[str, GraphEntry] = {
            name: GraphEntry(name=name, path=path) for name, path in config.graphs.items()
        }
        self.grants: dict[str, dict[str, str]] = _config_grants(config)
        self._load_state()

    def list_graphs(self) -> list[dict[str, Any]]:
        with self._lock:
            return [
                {
                    "name": entry.name,
                    "path": str(entry.path),
                    "open": entry.worker is not None,
                    "created_by": entry.created_by,
                }
                for entry in sorted(self.graphs.values(), key=lambda item: item.name)
            ]

    def visible_graphs(self, user_id: str, *, admin: bool = False) -> list[dict[str, Any]]:
        if admin:
            return self.list_graphs()
        with self._lock:
            allowed = set(self.grants.get(user_id, {}))
            return [item for item in self.list_graphs() if item["name"] in allowed]

    def access_for(self, user_id: str, graph: str) -> str | None:
        with self._lock:
            user_grants = self.grants.get(user_id, {})
            return user_grants.get(graph) or user_grants.get("*")

    def grant(self, user_id: str, graph: str, access: str) -> None:
        validate_graph_name(graph)
        if access not in {"read", "write"}:
            raise ServerError("invalid_request", "grant access must be 'read' or 'write'")
        with self._lock:
            if graph not in self.graphs:
                raise ServerError("graph_not_found", f"graph {graph!r} not found", status_code=404, graph=graph)
            self.grants.setdefault(user_id, {})[graph] = access
            self._save_state()

    def revoke(self, user_id: str, graph: str) -> None:
        validate_graph_name(graph)
        with self._lock:
            self.grants.setdefault(user_id, {}).pop(graph, None)
            self._save_state()

    def create_graph(self, name: str, *, created_by: str | None = None, grants: dict[str, str] | None = None) -> GraphEntry:
        name = validate_graph_name(name)
        with self._lock:
            if name in self.graphs:
                raise ServerError("graph_already_exists", f"graph {name!r} already exists", status_code=409, graph=name)
            entry = GraphEntry(name=name, path=default_graph_path(self.config.data_dir, name), created_by=created_by)
            entry.worker = GraphWorker(entry.path)
            self.graphs[name] = entry
            for user_id, access in (grants or {}).items():
                if access not in {"read", "write"}:
                    raise ServerError("invalid_request", "grant access must be 'read' or 'write'")
                self.grants.setdefault(str(user_id), {})[name] = access
            if created_by:
                self.grants.setdefault(created_by, {})[name] = "write"
            self._save_state()
            return entry

    def get_entry(self, name: str) -> GraphEntry:
        name = validate_graph_name(name)
        with self._lock:
            entry = self.graphs.get(name)
            if entry is None:
                raise ServerError("graph_not_found", f"graph {name!r} not found", status_code=404, graph=name)
            return entry

    def open_graph(self, name: str) -> GraphEntry:
        entry = self.get_entry(name)
        with self._lock:
            if entry.worker is None:
                entry.worker = GraphWorker(entry.path)
            return entry

    def call(self, name: str, func: Callable[[Graph], T]) -> T:
        entry = self.open_graph(name)
        assert entry.worker is not None
        return entry.worker.call(func)

    def call_context(self, name: str, func: Callable[[GraphWorkerContext], T]) -> T:
        entry = self.open_graph(name)
        assert entry.worker is not None
        return entry.worker.call_context(func)

    def create_snapshot(self, name: str, owner_user_id: str, ttl_seconds: float | None = None) -> dict[str, Any]:
        ttl = DEFAULT_SNAPSHOT_TTL_SECONDS if ttl_seconds is None else ttl_seconds
        if ttl <= 0:
            raise ServerError("invalid_request", "snapshot ttl_seconds must be positive")
        ttl = min(ttl, MAX_SNAPSHOT_TTL_SECONDS)

        def op(context: GraphWorkerContext) -> dict[str, Any]:
            _prune_snapshots(context)
            now = time.time()
            snapshot_id = str(uuid.uuid4())
            entry = SnapshotEntry(
                snapshot_id=snapshot_id,
                owner_user_id=owner_user_id,
                created_at=now,
                expires_at=now + ttl,
                snapshot=context.graph.snapshot(),
            )
            context.snapshots[snapshot_id] = entry
            return entry.metadata()

        return self.call_context(name, op)

    def list_snapshots(self, name: str, user_id: str, *, admin: bool = False) -> list[dict[str, Any]]:
        def op(context: GraphWorkerContext) -> list[dict[str, Any]]:
            _prune_snapshots(context)
            return [
                entry.metadata()
                for entry in sorted(context.snapshots.values(), key=lambda item: item.created_at)
                if admin or entry.owner_user_id == user_id
            ]

        return self.call_context(name, op)

    def delete_snapshot(self, name: str, snapshot_id: str, user_id: str, *, admin: bool = False) -> None:
        def op(context: GraphWorkerContext) -> None:
            _prune_snapshots(context)
            entry = _get_snapshot(context, snapshot_id)
            if not admin and entry.owner_user_id != user_id:
                raise ServerError("permission_denied", "snapshot belongs to another user", status_code=403, graph=name)
            context.snapshots.pop(snapshot_id, None)

        self.call_context(name, op)

    def call_snapshot(
        self,
        name: str,
        snapshot_id: str,
        user_id: str,
        *,
        admin: bool = False,
        func: Callable[[Any], T],
    ) -> T:
        def op(context: GraphWorkerContext) -> T:
            _prune_snapshots(context)
            entry = _get_snapshot(context, snapshot_id)
            if not admin and entry.owner_user_id != user_id:
                raise ServerError("permission_denied", "snapshot belongs to another user", status_code=403, graph=name)
            return func(entry.snapshot)

        return self.call_context(name, op)

    def close_all(self) -> None:
        with self._lock:
            workers = [entry.worker for entry in self.graphs.values() if entry.worker is not None]
            for entry in self.graphs.values():
                entry.worker = None
        for worker in workers:
            worker.close()

    def __del__(self) -> None:
        try:
            self.close_all()
        except Exception:
            pass

    def compact(self, name: str) -> None:
        self.call(name, lambda graph: graph.compact())

    def refresh(self, name: str) -> None:
        self.call(name, lambda graph: graph.refresh())

    def _load_state(self) -> None:
        if not self.state_path.exists():
            return
        with self.state_path.open("r", encoding="utf-8") as handle:
            state = json.load(handle)
        for name, payload in dict(state.get("graphs") or {}).items():
            graph_name = validate_graph_name(str(name))
            if graph_name not in self.graphs:
                self.graphs[graph_name] = GraphEntry(
                    name=graph_name,
                    path=(self.config.data_dir / payload.get("path", f"{graph_name}.db")).resolve(),
                    created_by=payload.get("created_by"),
                )
        for user_id, grants in dict(state.get("grants") or {}).items():
            self.grants.setdefault(str(user_id), {}).update({str(name): str(access) for name, access in dict(grants).items()})

    def _save_state(self) -> None:
        self.config.data_dir.mkdir(parents=True, exist_ok=True)
        dynamic_graphs = {
            name: {
                "path": _relative_path(entry.path, self.config.data_dir),
                "created_by": entry.created_by,
            }
            for name, entry in self.graphs.items()
            if name not in self.config.graphs
        }
        state = {"graphs": dynamic_graphs, "grants": self.grants}
        tmp = self.state_path.with_suffix(".json.tmp")
        with tmp.open("w", encoding="utf-8") as handle:
            json.dump(state, handle, indent=2, sort_keys=True)
            handle.write("\n")
        os.replace(tmp, self.state_path)


def _config_grants(config: ServerConfig) -> dict[str, dict[str, str]]:
    return {user_id: dict(user.graphs) for user_id, user in config.users.items()}


def _relative_path(path: Path, data_dir: Path) -> str:
    try:
        return str(path.resolve().relative_to(data_dir.resolve()))
    except ValueError:
        return path.name


def _prune_snapshots(context: GraphWorkerContext) -> None:
    now = time.time()
    expired = [snapshot_id for snapshot_id, entry in context.snapshots.items() if entry.expires_at <= now]
    for snapshot_id in expired:
        context.snapshots.pop(snapshot_id, None)


def _get_snapshot(context: GraphWorkerContext, snapshot_id: str) -> SnapshotEntry:
    entry = context.snapshots.get(snapshot_id)
    if entry is None:
        raise ServerError("snapshot_not_found", f"snapshot {snapshot_id!r} not found", status_code=404)
    return entry
