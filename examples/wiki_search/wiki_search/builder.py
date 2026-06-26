from __future__ import annotations

import json
import shutil
import sqlite3
from collections.abc import Iterable, Iterator
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, TypeVar

from tonggraph import Graph

from .data import (
    WikiChunk,
    WikiEntity,
    chunks_from_entities,
    iter_hf_wikipedia_entities,
    load_entities,
    load_entities_from_wikidata,
    write_entities_jsonl,
)
from .embeddings import create_embedding_provider


SAMPLE_PATH = Path(__file__).resolve().parents[1] / "data" / "sample_wikidata.jsonl"
T = TypeVar("T")


@dataclass
class BuildConfig:
    db_path: Path
    source_paths: list[Path]
    downloaded_qids: list[str]
    use_sample: bool = True
    language: str = "en"
    max_entities: int | None = None
    chunk_chars: int = 900
    batch_size: int = 128
    embedding_backend: str = "hash"
    embedding_dimensions: int = 128
    embedding_model: str | None = None
    replace: bool = True
    normalized_jsonl: Path | None = None


@dataclass
class HFWikipediaBuildConfig:
    db_path: Path
    dataset_name: str = "wikimedia/wikipedia"
    dataset_config: str = "20231101.en"
    split: str = "train"
    start: int | None = None
    max_records: int | None = 10000
    resume_state: Path | None = None
    chunk_chars: int = 900
    batch_size: int = 128
    embedding_backend: str = "sentence-transformers"
    embedding_dimensions: int = 128
    embedding_model: str = "intfloat/e5-base-v2"
    replace: bool = True
    normalized_jsonl: Path | None = None
    progress_every: int = 0
    article_chunks: bool = False


@dataclass
class BuildSummary:
    db_path: str
    normalized_jsonl: str | None
    entities: int
    chunks: int
    edges: int
    claim_edges: int
    vector_index: str
    text_index: str
    embedding_backend: str
    embedding_model: str
    embedding_dimensions: int


@dataclass
class StreamingState:
    dataset_name: str
    dataset_config: str
    split: str
    next_record: int
    indexed_records: int
    indexed_chunks: int
    db_path: str
    vector_index: str
    embedding_backend: str
    embedding_model: str
    embedding_dimensions: int


def build_wiki_graph(config: BuildConfig) -> BuildSummary:
    provider = create_embedding_provider(
        config.embedding_backend,
        dimensions=config.embedding_dimensions,
        model_name=config.embedding_model,
    )
    entities = _load_all_entities(config)
    if not entities:
        raise ValueError("no wiki entities were loaded")

    chunks = chunks_from_entities(entities, max_chars=config.chunk_chars)
    if not chunks:
        raise ValueError("loaded entities produced no searchable chunks")

    if config.normalized_jsonl is not None:
        write_entities_jsonl(config.normalized_jsonl, entities)

    _replace_db(config.db_path, enabled=config.replace)
    config.db_path.parent.mkdir(parents=True, exist_ok=True)
    _prepare_sqlite_compat_db(config.db_path)
    graph = Graph(str(config.db_path))
    _ensure_fulltext_index(graph, "wiki_text")
    _ensure_vector_index(graph, "wiki_chunks", provider)

    entity_ids = _add_entities(graph, entities)
    chunk_ids = _add_chunks(graph, chunks, entity_ids)
    claim_edge_count = _add_claim_edges(graph, entities, entity_ids)
    _embed_chunks(graph, chunks, chunk_ids, provider=provider, batch_size=config.batch_size)
    graph.compact()

    return BuildSummary(
        db_path=str(config.db_path),
        normalized_jsonl=str(config.normalized_jsonl) if config.normalized_jsonl else None,
        entities=len(entities),
        chunks=len(chunks),
        edges=graph.edge_count(),
        claim_edges=claim_edge_count,
        vector_index="wiki_chunks",
        text_index="wiki_text",
        embedding_backend=provider.backend,
        embedding_model=provider.model_name,
        embedding_dimensions=provider.dimensions,
    )


def build_hf_wikipedia_graph(config: HFWikipediaBuildConfig) -> BuildSummary:
    provider = create_embedding_provider(
        config.embedding_backend,
        dimensions=config.embedding_dimensions,
        model_name=config.embedding_model,
    )
    if config.batch_size <= 0:
        raise ValueError("batch_size must be greater than zero")
    if config.max_records is not None and config.max_records < 0:
        raise ValueError("max_records must be zero or greater")

    _replace_db(config.db_path, enabled=config.replace)
    config.db_path.parent.mkdir(parents=True, exist_ok=True)
    _prepare_sqlite_compat_db(config.db_path)
    if config.normalized_jsonl is not None and config.replace:
        config.normalized_jsonl.unlink(missing_ok=True)

    graph = Graph(str(config.db_path))
    _ensure_fulltext_index(graph, "wiki_text")
    _ensure_vector_index(graph, "wiki_chunks", provider)

    previous_state = _load_streaming_state(config.resume_state)
    start = _stream_start(config, previous_state)
    base_state = previous_state if not config.replace and config.start is None else None
    indexed_records = 0
    indexed_chunks = 0
    next_progress = config.progress_every

    entity_stream = iter_hf_wikipedia_entities(
        dataset_name=config.dataset_name,
        dataset_config=config.dataset_config,
        split=config.split,
        start=start,
        limit=config.max_records,
    )
    for batch_records in _batched(entity_stream, config.batch_size):
        entities = [entity for _, entity in batch_records]
        chunks = (
            _article_chunks_from_entities(entities)
            if config.article_chunks
            else chunks_from_entities(entities, max_chars=config.chunk_chars)
        )
        entity_ids = _add_entities(graph, entities, update_existing=True)
        chunk_ids = _add_chunks(graph, chunks, entity_ids, update_existing=True)
        _embed_chunks(graph, chunks, chunk_ids, provider=provider, batch_size=config.batch_size)

        if config.normalized_jsonl is not None:
            write_entities_jsonl(config.normalized_jsonl, entities, append=True)

        indexed_records += len(entities)
        indexed_chunks += len(chunks)
        _write_streaming_state(
            config.resume_state,
            StreamingState(
                dataset_name=config.dataset_name,
                dataset_config=config.dataset_config,
                split=config.split,
                next_record=batch_records[-1][0] + 1,
                indexed_records=(base_state.indexed_records if base_state else 0) + indexed_records,
                indexed_chunks=(base_state.indexed_chunks if base_state else 0) + indexed_chunks,
                db_path=str(config.db_path),
                vector_index="wiki_chunks",
                embedding_backend=provider.backend,
                embedding_model=provider.model_name,
                embedding_dimensions=provider.dimensions,
            ),
        )
        if config.progress_every > 0 and indexed_records >= next_progress:
            print(
                f"Indexed {indexed_records} records this run "
                f"({indexed_chunks} chunks); next row {batch_records[-1][0] + 1}",
                flush=True,
            )
            while indexed_records >= next_progress:
                next_progress += config.progress_every

    graph.compact()
    if indexed_records == 0:
        raise ValueError("no Wikipedia records were indexed")

    return BuildSummary(
        db_path=str(config.db_path),
        normalized_jsonl=str(config.normalized_jsonl) if config.normalized_jsonl else None,
        entities=indexed_records,
        chunks=indexed_chunks,
        edges=graph.edge_count(),
        claim_edges=0,
        vector_index="wiki_chunks",
        text_index="wiki_text",
        embedding_backend=provider.backend,
        embedding_model=provider.model_name,
        embedding_dimensions=provider.dimensions,
    )


def write_summary(path: Path, summary: BuildSummary, *, config: BuildConfig) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "summary": asdict(summary),
        "config": {
            "db_path": str(config.db_path),
            "source_paths": [str(path) for path in config.source_paths],
            "downloaded_qids": config.downloaded_qids,
            "use_sample": config.use_sample,
            "language": config.language,
            "max_entities": config.max_entities,
            "chunk_chars": config.chunk_chars,
            "batch_size": config.batch_size,
            "embedding_backend": config.embedding_backend,
            "embedding_dimensions": summary.embedding_dimensions,
            "embedding_model": config.embedding_model,
            "replace": config.replace,
            "normalized_jsonl": str(config.normalized_jsonl)
            if config.normalized_jsonl
            else None,
        },
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_hf_summary(
    path: Path,
    summary: BuildSummary,
    *,
    config: HFWikipediaBuildConfig,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "summary": asdict(summary),
        "config": {
            "db_path": str(config.db_path),
            "dataset_name": config.dataset_name,
            "dataset_config": config.dataset_config,
            "split": config.split,
            "start": config.start,
            "max_records": config.max_records,
            "resume_state": str(config.resume_state) if config.resume_state else None,
            "chunk_chars": config.chunk_chars,
            "batch_size": config.batch_size,
            "embedding_backend": config.embedding_backend,
            "embedding_dimensions": config.embedding_dimensions,
            "embedding_model": config.embedding_model,
            "replace": config.replace,
            "normalized_jsonl": str(config.normalized_jsonl)
            if config.normalized_jsonl
            else None,
            "progress_every": config.progress_every,
            "article_chunks": config.article_chunks,
        },
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _load_all_entities(config: BuildConfig) -> list[WikiEntity]:
    paths = list(config.source_paths)
    if config.use_sample and not paths and not config.downloaded_qids:
        paths.append(SAMPLE_PATH)

    entities = load_entities(
        paths,
        language=config.language,
        max_entities=config.max_entities,
    )
    if config.downloaded_qids:
        entities.extend(
            load_entities_from_wikidata(
                config.downloaded_qids,
                language=config.language,
            )
        )

    deduped: dict[str, WikiEntity] = {}
    for entity in entities:
        deduped[entity.qid] = entity
        if config.max_entities is not None and len(deduped) >= config.max_entities:
            break
    return list(deduped.values())


def _add_entities(
    graph: Graph,
    entities: list[WikiEntity],
    *,
    update_existing: bool = False,
) -> dict[str, int]:
    ids = {}
    for entity in entities:
        properties = {
            "qid": entity.qid,
            "title": entity.label,
            "description": entity.description,
            "enwiki_title": entity.enwiki_title,
            "url": entity.url,
            "kind": "entity",
        }
        node_id = graph.get_node_id(entity.qid)
        if node_id is not None and update_existing:
            graph.update_node(
                node_id,
                add_labels=["WikiEntity"],
                set_properties=properties,
            )
        elif node_id is None:
            node_id = graph.add_node(
                entity.qid,
                labels=["WikiEntity"],
                properties=properties,
            )
        else:
            raise ValueError(f"entity node {entity.qid!r} already exists")
        ids[entity.qid] = node_id
    return ids


def _add_chunks(
    graph: Graph,
    chunks: list[WikiChunk],
    entity_ids: dict[str, int],
    *,
    update_existing: bool = False,
) -> dict[str, int]:
    chunk_ids = {}
    for chunk in chunks:
        properties = {
            "chunk_id": chunk.chunk_id,
            "entity_qid": chunk.entity_qid,
            "title": chunk.title,
            "text": chunk.text,
            "source": chunk.source,
            "ordinal": chunk.ordinal,
            "kind": "chunk",
        }
        existing_id = graph.get_node_id(chunk.chunk_id)
        created = existing_id is None
        if existing_id is not None and update_existing:
            node_id = existing_id
            graph.update_node(
                node_id,
                add_labels=["WikiChunk"],
                set_properties=properties,
            )
        elif existing_id is None:
            node_id = graph.add_node(
                chunk.chunk_id,
                labels=["WikiChunk"],
                properties=properties,
            )
        else:
            raise ValueError(f"chunk node {chunk.chunk_id!r} already exists")
        chunk_ids[chunk.chunk_id] = node_id
        entity_id = entity_ids.get(chunk.entity_qid)
        if entity_id is not None and created:
            graph.add_edge(node_id, entity_id, "ABOUT", properties={"source": "chunk"})
            graph.add_edge(entity_id, node_id, "HAS_CHUNK", properties={"source": "chunk"})
    return chunk_ids


def _add_claim_edges(
    graph: Graph,
    entities: list[WikiEntity],
    entity_ids: dict[str, int],
) -> int:
    edge_count = 0
    seen: set[tuple[int, int, str]] = set()
    for entity in entities:
        source = entity_ids.get(entity.qid)
        if source is None:
            continue
        for property_id, target_qid in entity.claims:
            target = entity_ids.get(target_qid)
            if target is None:
                continue
            edge_type = f"WDT_{property_id}"
            key = (source, target, edge_type)
            if key in seen:
                continue
            seen.add(key)
            graph.add_edge(
                source,
                target,
                edge_type,
                properties={"property": property_id, "source": "wikidata"},
            )
            edge_count += 1
    return edge_count


def _embed_chunks(
    graph: Graph,
    chunks: list[WikiChunk],
    chunk_ids: dict[str, int],
    *,
    provider,
    batch_size: int,
) -> None:
    if batch_size <= 0:
        raise ValueError("batch_size must be greater than zero")
    for start in range(0, len(chunks), batch_size):
        batch = chunks[start : start + batch_size]
        texts = [chunk.search_text for chunk in batch]
        vectors = provider.embed_texts(texts, role="document")
        graph.upsert_vectors(
            "wiki_chunks",
            {
                chunk_ids[chunk.chunk_id]: vector
                for chunk, vector in zip(batch, vectors, strict=True)
            },
        )


def _article_chunks_from_entities(entities: Iterable[WikiEntity]) -> list[WikiChunk]:
    chunks = []
    for entity in entities:
        text = entity.search_text
        if not text:
            continue
        chunks.append(
            WikiChunk(
                chunk_id=f"{entity.qid}:chunk:0",
                entity_qid=entity.qid,
                title=entity.label or entity.qid,
                text=text,
                source=entity.url or "huggingface:wikipedia",
                ordinal=0,
            )
        )
    return chunks


def _ensure_fulltext_index(graph: Graph, name: str) -> None:
    desired = {
        "name": name,
        "target": "node",
        "properties": ["title", "text"],
        "tokenizer": "unicode61",
    }
    for definition in graph.fulltext_indexes():
        if definition["name"] != name:
            continue
        if definition != desired:
            raise ValueError(f"full-text index {name!r} exists with incompatible definition")
        return
    graph.create_fulltext_index(
        name,
        ["title", "text"],
        target="node",
        tokenizer="unicode61",
    )


def _ensure_vector_index(graph: Graph, name: str, provider) -> None:
    desired = {
        "name": name,
        "target": "node",
        "dimensions": provider.dimensions,
        "metric": "cosine",
        "model": provider.model_name,
        "model_version": provider.model_version,
    }
    for definition in graph.vector_indexes():
        if definition["name"] != name:
            continue
        if definition != desired:
            raise ValueError(f"vector index {name!r} exists with incompatible definition")
        return
    graph.create_vector_index(
        name,
        provider.dimensions,
        target="node",
        metric="cosine",
        model=provider.model_name,
        model_version=provider.model_version,
    )


def _replace_db(path: Path, *, enabled: bool) -> None:
    if not enabled:
        return
    path.unlink(missing_ok=True)
    shutil.rmtree(Path(f"{path}.segments"), ignore_errors=True)


def _prepare_sqlite_compat_db(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with sqlite3.connect(path) as connection:
        connection.execute(
            """
            CREATE TABLE IF NOT EXISTS op_log (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                op TEXT NOT NULL,
                object_id INTEGER NOT NULL,
                payload TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            """
        )
        row = connection.execute(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'op_log';"
        ).fetchone()
        if row is not None and "DEFAULT (unixepoch())" in str(row[0]):
            _rewrite_op_log_default(connection)


def _rewrite_op_log_default(connection: sqlite3.Connection) -> None:
    old = "DEFAULT (unixepoch())"
    new = "DEFAULT (strftime('%s','now'))"
    version = int(connection.execute("PRAGMA schema_version;").fetchone()[0])
    connection.execute("PRAGMA writable_schema = ON;")
    try:
        connection.execute(
            "UPDATE sqlite_master SET sql = replace(sql, ?, ?) "
            "WHERE type = 'table' AND name = 'op_log';",
            (old, new),
        )
        connection.execute(f"PRAGMA schema_version = {version + 1};")
    finally:
        connection.execute("PRAGMA writable_schema = OFF;")


def _batched(items: Iterable[T], batch_size: int) -> Iterator[list[T]]:
    batch: list[T] = []
    for item in items:
        batch.append(item)
        if len(batch) >= batch_size:
            yield batch
            batch = []
    if batch:
        yield batch


def _load_streaming_state(path: Path | None) -> StreamingState | None:
    if path is None or not path.exists():
        return None
    payload = json.loads(path.read_text(encoding="utf-8"))
    return StreamingState(**payload)


def _write_streaming_state(path: Path | None, state: StreamingState) -> None:
    if path is None:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(asdict(state), indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _stream_start(
    config: HFWikipediaBuildConfig,
    state: StreamingState | None,
) -> int:
    if config.start is not None:
        if config.start < 0:
            raise ValueError("start must be zero or greater")
        return config.start
    if state is None or config.replace:
        return 0
    _validate_streaming_state(config, state)
    return state.next_record


def _validate_streaming_state(
    config: HFWikipediaBuildConfig,
    state: StreamingState,
) -> None:
    expected: dict[str, Any] = {
        "dataset_name": config.dataset_name,
        "dataset_config": config.dataset_config,
        "split": config.split,
        "db_path": str(config.db_path),
        "embedding_backend": config.embedding_backend,
        "embedding_model": config.embedding_model,
    }
    for key, value in expected.items():
        if getattr(state, key) != value:
            raise ValueError(
                f"resume state {config.resume_state} has {key}={getattr(state, key)!r}; "
                f"expected {value!r}"
            )
