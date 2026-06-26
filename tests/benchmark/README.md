# GBench

GBench is the local TongGraph benchmark harness.

## Quick Start

Run the default offline smoke benchmark:

```bash
uv run python -m tests.benchmark.gbench --repeat 1
```

Write results to a JSON file:

```bash
uv run python -m tests.benchmark.gbench \
  --dataset synthetic-smoke \
  --repeat 3 \
  --output tests/benchmark/.gbench/results/synthetic-smoke.json
```

List workloads:

```bash
uv run python -m tests.benchmark.gbench --list-workloads
```

Run only selected workloads with shell-style globs:

```bash
uv run python -m tests.benchmark.gbench \
  --workload 'synthetic/query' \
  --workload 'synthetic/traversal'
```

## Datasets

`synthetic-smoke` is the default. It is generated locally, requires no network, and covers traversal, structured query/Cypher, GraphRAG retrieval, persistence, and probabilistic inference. Tune it with `--nodes` and `--degree`.

`pokec-small` downloads [Memgraph](https://github.com/memgraph/memgraph/tree/master/tests/mgbench)'s small Pokec import file on first use and caches it under `tests/benchmark/.gbench/cache/pokec-small/`. The importer parses only the known line-oriented subset used by that file:

- `CREATE (:User {id, completion_percentage, gender, age});`
- `MATCH (n:User {id}), (m:User {id}) CREATE (n)-[e: Friend]->(m);`

Bound local Pokec runs while developing:

```bash
uv run python -m tests.benchmark.gbench \
  --dataset pokec-small \
  --max-nodes 100 \
  --max-edges 500 \
  --repeat 1 \
  --output tests/benchmark/.gbench/results/pokec-smoke.json
```

Edges whose endpoints are outside the selected node set are skipped so TongGraph internal ids stay valid.

## Warm-Up Modes

`--warm-up cold` measures each workload without an extra pre-run.

`--warm-up hot` runs the workload once before measurement.

`--warm-up vulcanic` runs the workload for the configured repeat count before measuring it again.

## Output Shape

The JSON result has four top-level keys:

- `metadata`: git commit, Python/platform details, timestamp, dataset source, and cache status.
- `config`: dataset, repeat count, seed, warm-up mode, workload filters, limits, and cache directory.
- `dataset`: node/edge counts and dataset-specific import stats.
- `workloads`: one entry per measured workload with group, name, checksum, latency percentiles in ns, and throughput qps.

Checksums make it easier to spot accidentally empty workloads or behavior changes across benchmark revisions.

## Artifacts

All downloaded data, generated SQLite files, temp directories, and example result JSON files should live under:

```text
tests/benchmark/.gbench/
```

That directory is gitignored and should not be committed.

## Adding Workloads

Add a `Workload` entry in `tests/benchmark/gbench/workloads.py` with:

- `group`: broad category such as `read`, `query`, `aggregate`, or `analytical`.
- `name`: stable workload name.
- `datasets`: dataset names the workload supports.
- `run`: a function that accepts the loaded dataset artifact and benchmark config, performs deterministic work, and returns an integer checksum.

Prefer public TongGraph Python APIs. Keep workload parameter choices deterministic from the dataset metadata or configured seed so different runs are comparable.
