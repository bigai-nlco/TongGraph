# Benchmarks

TongGraph keeps the v0.2.0 benchmark runner under `tests/benchmark` so benchmark
smoke coverage can run with the normal test suite while still producing JSON
artifacts for local comparison.

```bash
uv run python scripts/build_python_extension.py
uv run python -m tests.benchmark.gbench \
  --nodes 100 \
  --degree 3 \
  --repeat 3 \
  --output /tmp/gbench.json
```

The JSON artifact includes environment metadata, commit hash, workload
configuration, per-workload timings, and result checksums. Initial workloads
cover traversal, structured/Cypher query execution, hybrid GraphRAG retrieval,
persistence/reopen, and belief propagation.

## Exact Vector Search

Run the exact vector benchmark for embedded `Graph.search_vector()` and TongGraph
Server HTTP vector search:

```bash
uv run python -m tests.benchmark.gbench.vector \
  --vectors 10000 \
  --dimensions 128 \
  --queries 20 \
  --batch-size 8 \
  --repeat 3 \
  --output tests/benchmark/.gbench/results/vector-exact-10k.json

uv run python -m tests.benchmark.gbench.vector \
  --vectors 100000 \
  --dimensions 128 \
  --queries 20 \
  --batch-size 8 \
  --repeat 3 \
  --output tests/benchmark/.gbench/results/vector-exact-100k.json
```

The benchmark builds one deterministic SQLite graph, measures embedded exact
search, then starts a local TongGraph Server over the same graph and measures
HTTP search. Results below are local reference numbers for commit
`400558ee980140105053938088727ab5b1da9bfe` on Linux 6.8 / Python 3.13.12 with
cosine search, 128 dimensions, `limit=10`, 20 query vectors, and 3 repeats.

| Vectors | Workload | Mean | P50 | P95 | Throughput |
|---:|---|---:|---:|---:|---:|
| 10k | embedded `search_vector` | 56.890 ms | 56.588 ms | 59.938 ms | 17.58 ops/s |
| 10k | server `search_vector` | 61.245 ms | 60.119 ms | 65.039 ms | 16.33 ops/s |
| 10k | embedded `search_vectors` batch | 379.354 ms | 455.020 ms | 455.704 ms | 2.64 batch ops/s |
| 10k | server `search_vectors` batch | 386.750 ms | 461.768 ms | 468.994 ms | 2.59 batch ops/s |
| 100k | embedded `search_vector` | 596.288 ms | 595.818 ms | 601.011 ms | 1.68 ops/s |
| 100k | server `search_vector` | 615.215 ms | 613.285 ms | 630.238 ms | 1.63 ops/s |
| 100k | embedded `search_vectors` batch | 4028.329 ms | 4835.346 ms | 4841.639 ms | 0.25 batch ops/s |
| 100k | server `search_vectors` batch | 4088.130 ms | 4893.004 ms | 4918.222 ms | 0.24 batch ops/s |

The batch rows measure one `search_vectors()` call as one operation. With
`batch_size=8` and 20 query vectors, the final batch has 4 vectors, so mean and
percentiles reflect mixed batch sizes.

These numbers match the expected exact-scan profile: latency grows roughly
linearly with vector count. The server wrapper adds only small overhead compared
with embedded search; the scan dominates at 100k vectors. For interactive local
or internal services, 10k vectors per index is comfortable, while 100k vectors is
best treated as a batch/offline or low-QPS tier unless higher latency is
acceptable. Larger or latency-sensitive deployments should wait for a future ANN
backend.

Use these numbers as local reproducibility evidence. Public speed claims should
only compare runs with the same commit, hardware, Python version, and benchmark
configuration.
