from __future__ import annotations

import gzip
import json
import re
import urllib.request
from collections.abc import Iterable, Iterator
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


WIKIDATA_ENTITY_URL = "https://www.wikidata.org/wiki/Special:EntityData/{qid}.json"


@dataclass
class WikiEntity:
    qid: str
    label: str
    description: str = ""
    aliases: list[str] = field(default_factory=list)
    enwiki_title: str = ""
    wiki_text: str = ""
    claims: list[tuple[str, str]] = field(default_factory=list)

    @property
    def search_text(self) -> str:
        parts = [self.label, self.description, self.enwiki_title, self.wiki_text]
        parts.extend(self.aliases[:8])
        return "\n".join(part for part in parts if part)


@dataclass
class WikiChunk:
    chunk_id: str
    entity_qid: str
    title: str
    text: str
    source: str = ""
    ordinal: int = 0

    @property
    def search_text(self) -> str:
        return f"{self.title}\n{self.text}".strip()


def load_entities(
    paths: Iterable[Path],
    *,
    language: str = "en",
    max_entities: int | None = None,
) -> list[WikiEntity]:
    entities: dict[str, WikiEntity] = {}
    for path in paths:
        for record in iter_json_records(path):
            entity = normalize_entity_record(record, language=language)
            if entity is None:
                continue
            current = entities.get(entity.qid)
            if current is None:
                entities[entity.qid] = entity
            else:
                entities[entity.qid] = merge_entity(current, entity)
            if max_entities is not None and len(entities) >= max_entities:
                return list(entities.values())
    return list(entities.values())


def load_entities_from_wikidata(
    qids: Iterable[str],
    *,
    language: str = "en",
) -> list[WikiEntity]:
    entities = []
    for qid in qids:
        url = WIKIDATA_ENTITY_URL.format(qid=qid)
        with urllib.request.urlopen(url, timeout=30) as response:
            payload = json.loads(response.read().decode("utf-8"))
        entity = normalize_entity_record(payload, language=language)
        if entity is not None:
            entities.append(entity)
    return entities


def chunks_from_entities(entities: Iterable[WikiEntity], *, max_chars: int = 900) -> list[WikiChunk]:
    chunks = []
    for entity in entities:
        text = entity.search_text
        if not text:
            continue
        for ordinal, chunk in enumerate(split_text(text, max_chars=max_chars)):
            chunks.append(
                WikiChunk(
                    chunk_id=f"{entity.qid}:chunk:{ordinal}",
                    entity_qid=entity.qid,
                    title=entity.label or entity.qid,
                    text=chunk,
                    source="wikidata+wiki_text",
                    ordinal=ordinal,
                )
            )
    return chunks


def iter_json_records(path: Path) -> Iterator[dict[str, Any]]:
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt", encoding="utf-8") as handle:
        stripped = handle.read(1)
        handle.seek(0)
        if stripped == "[":
            yield from _iter_wikidata_array_dump(handle)
            return
        for line in handle:
            line = line.strip()
            if not line:
                continue
            if line.endswith(","):
                line = line[:-1]
            yield json.loads(line)


def normalize_entity_record(record: dict[str, Any], *, language: str = "en") -> WikiEntity | None:
    if "entities" in record:
        for entity in record["entities"].values():
            return normalize_entity_record(entity, language=language)
        return None

    if "qid" in record:
        qid = str(record["qid"])
        label = str(record.get("label") or qid)
        claims = [
            (str(claim["property"]), str(claim["target"]))
            for claim in record.get("claims", [])
            if claim.get("property") and claim.get("target")
        ]
        return WikiEntity(
            qid=qid,
            label=label,
            description=str(record.get("description") or ""),
            aliases=[str(alias) for alias in record.get("aliases", [])],
            enwiki_title=str(record.get("enwiki_title") or record.get("title") or label),
            wiki_text=str(record.get("wiki_text") or record.get("text") or ""),
            claims=claims,
        )

    qid = str(record.get("id") or "")
    if not qid.startswith("Q"):
        return None
    label = _language_value(record.get("labels", {}), language) or qid
    aliases = [
        str(alias.get("value"))
        for alias in record.get("aliases", {}).get(language, [])
        if alias.get("value")
    ]
    sitelinks = record.get("sitelinks", {})
    enwiki_title = sitelinks.get(f"{language}wiki", {}).get("title", "")
    return WikiEntity(
        qid=qid,
        label=label,
        description=_language_value(record.get("descriptions", {}), language),
        aliases=aliases,
        enwiki_title=str(enwiki_title),
        wiki_text=str(record.get("wiki_text") or record.get("text") or ""),
        claims=_extract_claim_edges(record.get("claims", {})),
    )


def merge_entity(left: WikiEntity, right: WikiEntity) -> WikiEntity:
    aliases = list(dict.fromkeys([*left.aliases, *right.aliases]))
    claims = list(dict.fromkeys([*left.claims, *right.claims]))
    return WikiEntity(
        qid=left.qid,
        label=right.label if right.label != right.qid else left.label,
        description=right.description or left.description,
        aliases=aliases,
        enwiki_title=right.enwiki_title or left.enwiki_title,
        wiki_text=right.wiki_text or left.wiki_text,
        claims=claims,
    )


def write_entities_jsonl(path: Path, entities: Iterable[WikiEntity]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for entity in entities:
            payload = {
                "qid": entity.qid,
                "label": entity.label,
                "description": entity.description,
                "aliases": entity.aliases,
                "enwiki_title": entity.enwiki_title,
                "wiki_text": entity.wiki_text,
                "claims": [
                    {"property": prop, "target": target} for prop, target in entity.claims
                ],
            }
            handle.write(json.dumps(payload, ensure_ascii=False, sort_keys=True) + "\n")


def split_text(text: str, *, max_chars: int) -> list[str]:
    text = re.sub(r"\s+", " ", text).strip()
    if len(text) <= max_chars:
        return [text] if text else []
    sentences = re.split(r"(?<=[.!?])\s+", text)
    chunks: list[str] = []
    current = ""
    for sentence in sentences:
        if len(current) + len(sentence) + 1 <= max_chars:
            current = f"{current} {sentence}".strip()
            continue
        if current:
            chunks.append(current)
        current = sentence
    if current:
        chunks.append(current)
    return chunks


def _language_value(values: dict[str, Any], language: str) -> str:
    if language in values and values[language].get("value"):
        return str(values[language]["value"])
    if "en" in values and values["en"].get("value"):
        return str(values["en"]["value"])
    return ""


def _extract_claim_edges(claims: dict[str, Any]) -> list[tuple[str, str]]:
    edges: list[tuple[str, str]] = []
    for property_id, statements in claims.items():
        for statement in statements:
            mainsnak = statement.get("mainsnak", {})
            datavalue = mainsnak.get("datavalue", {})
            value = datavalue.get("value")
            if isinstance(value, dict) and value.get("entity-type") == "item":
                target = value.get("id") or f"Q{value.get('numeric-id')}"
                if target and str(target).startswith("Q"):
                    edges.append((str(property_id), str(target)))
    return edges


def _iter_wikidata_array_dump(handle: Any) -> Iterator[dict[str, Any]]:
    for line in handle:
        line = line.strip()
        if line in {"[", "]"} or not line:
            continue
        if line.endswith(","):
            line = line[:-1]
        yield json.loads(line)
