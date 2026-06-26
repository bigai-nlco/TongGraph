#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

from wiki_search.builder import (
    BuildConfig,
    HFWikipediaBuildConfig,
    build_hf_wikipedia_graph,
    build_wiki_graph,
    write_hf_summary,
    write_summary,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a TongGraph wiki vector-search database.",
    )
    parser.add_argument(
        "--db-path",
        type=Path,
        default=Path("search_data/wiki_graph.db"),
        help="SQLite-backed TongGraph database to create.",
    )
    parser.add_argument(
        "--source",
        action="append",
        type=Path,
        default=[],
        help=(
            "Local JSON/JSONL/.gz Wikidata-style source. Can be repeated. "
            "Use records shaped like data/sample_wikidata.jsonl or Wikidata "
            "entity JSON dump records."
        ),
    )
    parser.add_argument(
        "--hf-wikipedia",
        action="store_true",
        help="Stream Wikipedia articles from the Hugging Face wikimedia/wikipedia dataset.",
    )
    parser.add_argument(
        "--hf-dataset",
        default="wikimedia/wikipedia",
        help="Hugging Face dataset id for --hf-wikipedia.",
    )
    parser.add_argument(
        "--hf-config",
        default="20231101.en",
        help="Hugging Face dataset config for --hf-wikipedia.",
    )
    parser.add_argument(
        "--hf-split",
        default="train",
        help="Hugging Face dataset split for --hf-wikipedia.",
    )
    parser.add_argument(
        "--hf-start",
        type=int,
        help="Zero-based streaming row offset. Overrides --resume-state when set.",
    )
    parser.add_argument(
        "--hf-max-records",
        type=int,
        default=10000,
        help="Maximum Hugging Face streaming rows to process in this run.",
    )
    parser.add_argument(
        "--resume-state",
        type=Path,
        default=Path("search_data/hf_wikipedia_state.json"),
        help="JSON cursor used to continue Hugging Face streaming builds.",
    )
    parser.add_argument(
        "--progress-every",
        type=int,
        default=1000,
        help="Print progress every N Hugging Face records. Use 0 to disable.",
    )
    parser.add_argument(
        "--hf-article-chunks",
        action="store_true",
        help="Store and embed one full-text chunk per streamed Wikipedia row.",
    )
    parser.add_argument(
        "--download-qid",
        action="append",
        default=[],
        help="Fetch one entity from Special:EntityData, for example Q42.",
    )
    parser.add_argument("--no-sample", action="store_true", help="Do not use bundled sample data.")
    parser.add_argument("--language", default="en", help="Preferred Wikidata language code.")
    parser.add_argument("--max-entities", type=int, help="Stop after this many unique entities.")
    parser.add_argument("--chunk-chars", type=int, default=900, help="Maximum text characters per chunk.")
    parser.add_argument("--batch-size", type=int, default=128, help="Embedding/vector upsert batch size.")
    parser.add_argument(
        "--embedding-backend",
        choices=["hash", "sentence-transformers"],
        default="hash",
        help="Embedding backend owned by this example, not TongGraph core.",
    )
    parser.add_argument(
        "--embedding-dimensions",
        type=int,
        default=128,
        help="Dimensions for the deterministic hash backend.",
    )
    parser.add_argument(
        "--embedding-model",
        help="sentence-transformers model name, for example intfloat/e5-small-v2.",
    )
    parser.add_argument(
        "--append",
        action="store_true",
        help="Open an existing DB instead of replacing it. Usually leave disabled.",
    )
    parser.add_argument(
        "--normalized-jsonl",
        type=Path,
        default=Path("search_data/normalized_wiki_entities.jsonl"),
        help="Write normalized input records for inspection/reuse.",
    )
    parser.add_argument(
        "--summary",
        type=Path,
        default=Path("search_data/build_summary.json"),
        help="Write build metadata JSON.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.hf_wikipedia:
        config = HFWikipediaBuildConfig(
            db_path=args.db_path,
            dataset_name=args.hf_dataset,
            dataset_config=args.hf_config,
            split=args.hf_split,
            start=args.hf_start,
            max_records=args.hf_max_records,
            resume_state=args.resume_state,
            chunk_chars=args.chunk_chars,
            batch_size=args.batch_size,
            embedding_backend=args.embedding_backend,
            embedding_dimensions=args.embedding_dimensions,
            embedding_model=args.embedding_model or "intfloat/e5-base-v2",
            replace=not args.append,
            normalized_jsonl=args.normalized_jsonl,
            progress_every=args.progress_every,
            article_chunks=args.hf_article_chunks,
        )
        summary = build_hf_wikipedia_graph(config)
        write_hf_summary(args.summary, summary, config=config)
        print(f"Built TongGraph wiki DB from Hugging Face Wikipedia: {summary.db_path}")
        print(
            f"Indexed records this run: {summary.entities}, chunks: {summary.chunks}, "
            f"total edges: {summary.edges}"
        )
        print(f"Vector index: {summary.vector_index} ({summary.embedding_dimensions} dims)")
        print(f"Resume state: {args.resume_state}")
        print(f"Summary: {args.summary}")
        return

    config = BuildConfig(
        db_path=args.db_path,
        source_paths=args.source,
        downloaded_qids=args.download_qid,
        use_sample=not args.no_sample,
        language=args.language,
        max_entities=args.max_entities,
        chunk_chars=args.chunk_chars,
        batch_size=args.batch_size,
        embedding_backend=args.embedding_backend,
        embedding_dimensions=args.embedding_dimensions,
        embedding_model=args.embedding_model,
        replace=not args.append,
        normalized_jsonl=args.normalized_jsonl,
    )
    summary = build_wiki_graph(config)
    write_summary(args.summary, summary, config=config)
    print(f"Built TongGraph wiki DB: {summary.db_path}")
    print(
        f"Entities: {summary.entities}, chunks: {summary.chunks}, "
        f"edges: {summary.edges} ({summary.claim_edges} Wikidata claim edges)"
    )
    print(f"Vector index: {summary.vector_index} ({summary.embedding_dimensions} dims)")
    print(f"Summary: {args.summary}")


if __name__ == "__main__":
    main()
