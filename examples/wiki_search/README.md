# Wiki Search

This example builds a local graph database for Wikidata/Wikipedia-style search.
It stores graph structure and vectors in TongGraph, then serves natural-language
retrieval through FastAPI.

The bundled path is small and offline. It uses `data/sample_wikidata.jsonl`,
which contains real Wikidata QIDs and property IDs in a normalized JSONL shape.
For a larger build, point the builder at local Wikidata JSON dump slices,
Wikidata `Special:EntityData` records, or a converted wiki corpus such as
wiki-18.

## Boundary

- TongGraph core owns graph entities, edges, SQLite persistence, vector indexes,
  exact vector search, and batch vector search.
- This example owns Wikidata/wiki parsing, corpus chunking, embedding model
  choice, and FastAPI request/response schemas.
- TongGraph does not download corpora, call embedding providers, or depend on
  FastAPI/sentence-transformers.

## Data Plan

Recommended scalable path:

1. Read Wikidata JSON entity dumps or `Special:EntityData` JSON records for
   labels, descriptions, aliases, sitelinks, and item-to-item claims.
2. Convert each Wikidata item to a `WikiEntity` node.
3. Convert item claims with item targets to graph edges, using edge types like
   `WDT_P31` for `instance of`.
4. Attach wiki article text or corpus passages to `WikiChunk` nodes.
5. Embed each chunk in the example layer and store the vectors in TongGraph's
   `wiki_chunks` vector index.

Useful upstream references:

- Wikidata database downloads:
  <https://www.wikidata.org/wiki/Wikidata:Database_download>
- Wikibase JSON entity format:
  <https://doc.wikimedia.org/Wikibase/master/php/docs_topics_json.html>
- Wikibase RDF dump format and truthy direct claims:
  <https://www.mediawiki.org/wiki/Wikibase/Indexing/RDF_Dump_Format>
- Hugging Face Wikipedia dataset:
  <https://huggingface.co/datasets/wikimedia/wikipedia>

## Setup

From this directory:

```bash
uv sync
```

The example `pyproject.toml` depends on the outer TongGraph package through an
editable path dependency. Install optional embedding dependencies only when you
want sentence-transformers:

```bash
uv sync --extra embeddings
```

Install the Hugging Face streaming dataset path together with embeddings:

```bash
uv sync --extra embeddings --extra wikipedia
```

## Build The Sample DB

```bash
uv run python download_search_data.py
```

This creates:

- `search_data/wiki_graph.db`
- `search_data/wiki_graph.db.segments/`
- `search_data/normalized_wiki_entities.jsonl`
- `search_data/build_summary.json`

Try a small live Wikidata fetch:

```bash
uv run python download_search_data.py \
  --no-sample \
  --download-qid Q42 \
  --download-qid Q937
```

Build from local JSON/JSONL files:

```bash
uv run python download_search_data.py \
  --source /path/to/wikidata_slice.jsonl \
  --max-entities 10000 \
  --embedding-backend sentence-transformers \
  --embedding-model intfloat/e5-small-v2
```

The parser accepts either normalized records shaped like
`data/sample_wikidata.jsonl` or Wikidata entity JSON records. Full Wikidata dump
files are very large, so start with a small slice before running a full build.

## Build From Hugging Face Wikipedia Streaming

Stream the first 10,000 records from the Hugging Face `wikimedia/wikipedia`
dataset config `20231101.en`, embed one article chunk per row with
`intfloat/e5-base-v2`, and store them in TongGraph:

```bash
SENTENCE_TRANSFORMERS_DEVICE=cpu uv run python download_search_data.py \
  --hf-wikipedia \
  --hf-config 20231101.en \
  --hf-max-records 10000 \
  --embedding-backend sentence-transformers \
  --embedding-model intfloat/e5-base-v2 \
  --batch-size 64 \
  --progress-every 1000 \
  --hf-article-chunks
```

The build writes `search_data/hf_wikipedia_state.json` after each committed
batch. Continue with the next streaming rows later by appending to the same DB:

```bash
SENTENCE_TRANSFORMERS_DEVICE=cpu uv run python download_search_data.py \
  --hf-wikipedia \
  --append \
  --hf-max-records 10000 \
  --embedding-backend sentence-transformers \
  --embedding-model intfloat/e5-base-v2 \
  --batch-size 64 \
  --progress-every 1000 \
  --hf-article-chunks
```

Query the DB directly without starting the server:

```bash
SENTENCE_TRANSFORMERS_DEVICE=cpu uv run python query_wiki.py "early programming languages and compilers" \
  --embedding-backend sentence-transformers \
  --embedding-model intfloat/e5-base-v2 \
  --top-k 3
```

## Serve Retrieval

```bash
uv run python rag_server.py --db-path search_data/wiki_graph.db --port 9002
```

Retrieve one query:

```bash
curl -s http://127.0.0.1:9002/retrieve \
  -H 'content-type: application/json' \
  -d '{"query":"science fiction comedy by Douglas Adams","top_k":3}'
```

Retrieve a batch through TongGraph's batch vector API:

```bash
curl -s http://127.0.0.1:9002/retrieve_batch \
  -H 'content-type: application/json' \
  -d '{"queries":["relativity physicist","hitchhiker comedy"],"top_k":2}'
```

You can also run with `uvicorn` directly:

```bash
WIKI_GRAPH_DB=search_data/wiki_graph.db \
  uv run uvicorn rag_server:app --host 127.0.0.1 --port 9002
```
