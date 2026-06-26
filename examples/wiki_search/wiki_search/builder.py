from __future__ import annotations

import json
import shutil
from dataclasses import asdict, dataclass
from pathlib import Path

from tonggraph import Graph

from .data import (
    WikiChunk,
    WikiEntity,
    chunks_from_entities,
    load_entities,
    load_entities_from_wikidata,
    write_entities_jsonl,
)
from .embeddings import create_embedding_provider


SAMPLE_PATH = Path(__file__).resolve().parents[1] / "data" / "sample_wikidata.jsonl"


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
    graph = Graph(str(config.db_path))
    graph.create_fulltext_index(
        "wiki_text",
        ["title", "text"],
        target="node",
        tokenizer="unicode61",
    )
    graph.create_vector_index(
        "wiki_chunks",
        provider.dimensions,
        target="node",
        metric="cosine",
        model=provider.model_name,
        model_version=provider.model_version,
    )

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
            "embedding_dimensions": config.embedding_dimensions,
            "embedding_model": config.embedding_model,
            "replace": config.replace,
            "normalized_jsonl": str(config.normalized_jsonl)
            if config.normalized_jsonl
            else None,
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


def _add_entities(graph: Graph, entities: list[WikiEntity]) -> dict[str, int]:
    ids = {}
    for entity in entities:
        node_id = graph.add_node(
            entity.qid,
            labels=["WikiEntity"],
            properties={
                "qid": entity.qid,
                "title": entity.label,
                "description": entity.description,
                "enwiki_title": entity.enwiki_title,
                "kind": "entity",
            },
        )
        ids[entity.qid] = node_id
    return ids


def _add_chunks(
    graph: Graph,
    chunks: list[WikiChunk],
    entity_ids: dict[str, int],
) -> dict[str, int]:
    chunk_ids = {}
    for chunk in chunks:
        node_id = graph.add_node(
            chunk.chunk_id,
            labels=["WikiChunk"],
            properties={
                "chunk_id": chunk.chunk_id,
                "entity_qid": chunk.entity_qid,
                "title": chunk.title,
                "text": chunk.text,
                "source": chunk.source,
                "ordinal": chunk.ordinal,
                "kind": "chunk",
            },
        )
        chunk_ids[chunk.chunk_id] = node_id
        entity_id = entity_ids.get(chunk.entity_qid)
        if entity_id is not None:
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


def _replace_db(path: Path, *, enabled: bool) -> None:
    if not enabled:
        return
    if path.exists():
        path.unlink()
    segments = Path(f"{path}.segments")
    if segments.exists():
        shutil.rmtree(segments)
