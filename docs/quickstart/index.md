# Quickstart

TongGraph is a local graph engine for Python applications. For source-tree
development, start with:

```bash
uv sync --dev
uv run python scripts/build_python_extension.py
uv run python -c "from tonggraph import Graph; print(Graph().node_count())"
```

Then choose the quickstart that matches your use case:

- [Property graph basics](property-graph-basics.md)
- [GraphRAG retrieval](graphrag-retrieval.md)
- [AI memory/session graph](ai-memory-session-graph.md)
- [Local persistence and reopen](local-persistence-reopen.md)
- [Structured query from natural language compiler](structured-query-nl.md)
- [Probabilistic inference](probabilistic-inference.md)

For longer examples, see the [examples section](../examples/index.md).
