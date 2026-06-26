from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from threading import RLock
from typing import Any

from tonggraph import Graph

from .embeddings import EmbeddingProvider, create_embedding_provider


@dataclass
class RetrievalHit:
    id: str
    score: float
    title: str
    text: str
    entity: dict[str, Any] | None
    related_entities: list[dict[str, Any]]


class WikiGraphRetriever:
    def __init__(
        self,
        db_path: Path,
        *,
        embedding_backend: str = "hash",
        embedding_model: str | None = None,
        vector_index: str = "wiki_chunks",
        text_index: str = "wiki_text",
    ) -> None:
        self.db_path = Path(db_path)
        self.graph = Graph(str(self.db_path))
        self.vector_index = vector_index
        self.text_index = text_index
        self._lock = RLock()

        definition = self._vector_definition(vector_index)
        self.embedding_provider = create_embedding_provider(
            embedding_backend,
            dimensions=int(definition["dimensions"]),
            model_name=embedding_model,
        )
        if self.embedding_provider.dimensions != int(definition["dimensions"]):
            raise ValueError(
                "embedding dimensions do not match stored vector index "
                f"{definition['dimensions']}"
            )

    def search(
        self,
        query: str,
        *,
        top_k: int = 5,
        labels: list[str] | None = None,
        expand_hops: int = 1,
    ) -> list[dict[str, Any]]:
        return self.search_batch(
            [query],
            top_k=top_k,
            labels=labels,
            expand_hops=expand_hops,
        )[0]

    def search_batch(
        self,
        queries: list[str],
        *,
        top_k: int = 5,
        labels: list[str] | None = None,
        expand_hops: int = 1,
    ) -> list[list[dict[str, Any]]]:
        if top_k <= 0:
            raise ValueError("top_k must be greater than zero")
        vectors = self.embedding_provider.embed_texts(queries, role="query")
        with self._lock:
            batches = self.graph.search_vectors(
                self.vector_index,
                vectors,
                labels=labels or ["WikiChunk"],
                limit=top_k,
            )
            return [
                [
                    self._format_hit(row, expand_hops=expand_hops)
                    for row in batch
                ]
                for batch in batches
            ]

    def stats(self) -> dict[str, Any]:
        with self._lock:
            graph_stats = self.graph.stats()
            return {
                "db_path": str(self.db_path),
                "nodes": graph_stats["nodes"],
                "edges": graph_stats["edges"],
                "vector_indexes": self.graph.vector_indexes(),
                "fulltext_indexes": self.graph.fulltext_indexes(),
                "embedding_backend": self.embedding_provider.backend,
                "embedding_model": self.embedding_provider.model_name,
                "embedding_dimensions": self.embedding_provider.dimensions,
            }

    def _format_hit(self, row: dict[str, Any], *, expand_hops: int) -> dict[str, Any]:
        chunk = self.graph.get_node(int(row["id"]))
        entity = self._entity_for_chunk(chunk.id)
        related = self._related_entities(entity["internal_id"], expand_hops) if entity else []
        return {
            "id": chunk.external_id,
            "score": float(row["score"]),
            "title": str(chunk.properties.get("title", "")),
            "text": str(chunk.properties.get("text", "")),
            "entity": entity,
            "related_entities": related,
        }

    def _entity_for_chunk(self, chunk_id: int) -> dict[str, Any] | None:
        for neighbor_id in self.graph.neighbors(chunk_id, direction="out", edge_type="ABOUT"):
            node = self.graph.get_node(neighbor_id)
            if "WikiEntity" in node.labels:
                return {
                    "internal_id": node.id,
                    "qid": node.properties.get("qid", node.external_id),
                    "title": node.properties.get("title", node.external_id),
                    "description": node.properties.get("description", ""),
                    "enwiki_title": node.properties.get("enwiki_title", ""),
                }
        return None

    def _related_entities(self, entity_id: int, hops: int) -> list[dict[str, Any]]:
        if hops <= 0:
            return []
        related = []
        seen = {entity_id}
        frontier = [entity_id]
        for _ in range(hops):
            next_frontier = []
            for current in frontier:
                for neighbor_id in self.graph.neighbors(current, direction="both"):
                    if neighbor_id in seen:
                        continue
                    seen.add(neighbor_id)
                    node = self.graph.get_node(neighbor_id)
                    if "WikiEntity" not in node.labels:
                        continue
                    related.append(
                        {
                            "qid": node.properties.get("qid", node.external_id),
                            "title": node.properties.get("title", node.external_id),
                            "description": node.properties.get("description", ""),
                        }
                    )
                    next_frontier.append(neighbor_id)
            frontier = next_frontier
        return related[:20]

    def _vector_definition(self, name: str) -> dict[str, Any]:
        for definition in self.graph.vector_indexes():
            if definition["name"] == name:
                return definition
        raise ValueError(f"vector index {name!r} not found in {self.db_path}")
