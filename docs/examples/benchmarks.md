# Benchmarks

TongGraph keeps the v0.2.0 benchmark runner under `tests/benchmark` so benchmark
smoke coverage can run with the normal test suite while still producing JSON
artifacts for local comparison.

```bash
uv run python scripts/build_python_extension.py
uv run python tests/benchmark/gbench.py \
  --nodes 100 \
  --degree 3 \
  --repeat 3 \
  --output /tmp/gbench.json
```

The JSON artifact includes environment metadata, commit hash, workload
configuration, per-workload timings, and result checksums. Initial workloads
cover traversal, structured/Cypher query execution, hybrid GraphRAG retrieval,
persistence/reopen, and belief propagation.

Use these numbers as local reproducibility evidence. Public speed claims should
only compare runs with the same commit, hardware, Python version, and benchmark
configuration.
