# Local Server

TongGraph Server is an optional local or internal-network HTTP wrapper around the
embedded `Graph` API.

## Install

```bash
uv sync --extra server --dev
```

or install the package with the server extra:

```bash
pip install 'tonggraph[server]'
```

## Minimal Config

```yaml
host: 127.0.0.1
port: 8719
data_dir: .tonggraph
graphs:
  shared_kg: shared.db
operations:
  request_logging: true
  request_timeout_seconds: 30
  metrics: true
auth:
  mode: token
  users:
    admin:
      admin: true
      token: admin-dev-token
      graphs:
        "*": write
    alice:
      token: alice-dev-token
      graphs:
        shared_kg: write
```

## Start

```bash
tonggraph-server --config tonggraph-server.yml
```

## Use With Curl

```bash
curl -H 'Authorization: Bearer admin-dev-token' http://127.0.0.1:8719/health

curl -X POST http://127.0.0.1:8719/admin/graphs \
  -H 'Authorization: Bearer admin-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"name":"alice_memory","grants":{"alice":"write"}}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/nodes \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"external_id":"alice","labels":["Person"],"properties":{"name":"Alice"}}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/query \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"spec":{"match":[{"node":"n","external_id":"alice"}]}}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/vector/embeddings/search-batch \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"query_vectors":[[1.0,0.0],[0.5,0.5]],"limit":3}'
```

## Python Client

```python
from tonggraph.server.client import TongGraphClient

admin = TongGraphClient("http://127.0.0.1:8719", token="admin-dev-token")
admin.create_graph("alice_memory", grants={"alice": "write"})

client = TongGraphClient("http://127.0.0.1:8719", token="alice-dev-token")
graph = client.graph("alice_memory")

alice = graph.add_node(
    "alice",
    labels=["Person"],
    properties={"name": "Alice"},
)
bob = graph.add_node("bob", labels=["Person"], properties={"name": "Bob"})
graph.add_edge(alice, bob, "KNOWS", properties={"weight": 1.0})

rows = graph.query({"match": [{"node": "n", "external_id": "alice"}]})

graph.create_vector_index("embeddings", 2, target="node", metric="cosine")
graph.upsert_vector("embeddings", alice, [1.0, 0.0])
graph.upsert_vector("embeddings", bob, [0.5, 0.5])
nearest = graph.search_vectors("embeddings", [[1.0, 0.0], [0.5, 0.5]], limit=1)

snapshot = graph.create_snapshot(ttl_seconds=600)
graph.add_node("later")
stable_count = snapshot.node_count()

print(rows, nearest, stable_count)
```

The first Python client returns JSON-compatible dict/list values. It does not
start the server process; point it at an already running `tonggraph-server`.

## Bulk Ingest And Context Retrieval

Use batch writes for remote ingest, then combine text/vector candidates with graph
expansion through `retrieve_context()`:

```bash
curl -X POST http://127.0.0.1:8719/graphs/alice_memory/nodes/batch \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"records":[{"external_id":"doc:1","labels":["Document"],"properties":{"text":"graph memory retrieval"}},{"external_id":"chunk:1","labels":["Chunk"],"properties":{"text":"retrieval context"}}]}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/edges/batch \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"records":[{"source":0,"target":1,"edge_type":"HAS_CHUNK"}]}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/retrieve/context \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"text_index":"chunks","text_query":"graph retrieval","vector_index":"chunks","vector_query":[1.0,0.2,0.4],"radius":1,"limit":5}'
```

```python
ids = graph.add_nodes([
    {"external_id": "doc:1", "labels": ["Document"], "properties": {"text": "graph memory retrieval"}},
    {"external_id": "chunk:1", "labels": ["Chunk"], "properties": {"text": "retrieval context"}},
])
graph.add_edges([{"source": ids[0], "target": ids[1], "edge_type": "HAS_CHUNK"}])

graph.create_fulltext_index("chunks", ["text"])
graph.create_vector_index("chunks", 3)
graph.upsert_vectors("chunks", {ids[0]: [1.0, 0.2, 0.4], ids[1]: [0.9, 0.1, 0.3]})

rows = graph.retrieve_context(
    text_index="chunks",
    text_query="graph retrieval",
    vector_index="chunks",
    vector_query=[1.0, 0.2, 0.4],
    radius=1,
    limit=5,
)
```

## Controlled Import And Export

Server-side import paths are resolved under `<data_dir>/imports/`; export paths
are resolved under `<data_dir>/exports/`. Absolute paths and `..` escapes are
rejected.

```bash
curl -X POST http://127.0.0.1:8719/graphs/alice_memory/import/nodes/jsonl \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"path":"nodes.jsonl"}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/export/nodes/jsonl \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"path":"exports/nodes.jsonl"}'
```

```python
imported = graph.import_nodes_jsonl("nodes.jsonl")
graph.export_nodes_jsonl("exports/nodes.jsonl")
graph.export_query_rows_jsonl("exports/rows.jsonl", rows=[{"node": imported[0]}])
```

## Traversal And Compute

```bash
curl 'http://127.0.0.1:8719/graphs/alice_memory/traversal/neighbors/0?direction=out' \
  -H 'Authorization: Bearer alice-dev-token'

curl 'http://127.0.0.1:8719/graphs/alice_memory/algorithms/shortest-path?start=0&target=1&weight_property=weight' \
  -H 'Authorization: Bearer alice-dev-token'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/compute/batch \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"jobs":[{"op":"bfs","start":0,"max_depth":2},{"op":"pagerank","iterations":5}]}'
```

## Snapshots

Create a read-only snapshot before later writes:

```bash
SNAPSHOT_ID=$(curl -s -X POST http://127.0.0.1:8719/graphs/alice_memory/snapshots \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"ttl_seconds":600}' | python -c 'import json,sys; print(json.load(sys.stdin)["snapshot"]["snapshot_id"])')

curl http://127.0.0.1:8719/graphs/alice_memory/snapshots/$SNAPSHOT_ID/nodes/count \
  -H 'Authorization: Bearer alice-dev-token'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/snapshots/$SNAPSHOT_ID/query \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"spec":{"match":[{"node":"n","external_id":"alice"}]}}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/snapshots/$SNAPSHOT_ID/fulltext/people/search \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"query":"Alice","limit":3}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/snapshots/$SNAPSHOT_ID/vector/embeddings/search \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"query_vector":[1.0,0.0],"limit":3}'

curl -X POST http://127.0.0.1:8719/graphs/alice_memory/snapshots/$SNAPSHOT_ID/vector/embeddings/search-batch \
  -H 'Authorization: Bearer alice-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"query_vectors":[[1.0,0.0],[0.5,0.5]],"limit":3}'

curl -X DELETE http://127.0.0.1:8719/graphs/alice_memory/snapshots/$SNAPSHOT_ID \
  -H 'Authorization: Bearer alice-dev-token'
```

Snapshots are in-memory, read-only, TTL-bound resources. They are not persisted
across server restart.


## Inference

The Python client also exposes probability transfer and finite-discrete belief
propagation endpoints:

```python
from tonggraph.server.client import TongGraphClient

client = TongGraphClient("http://127.0.0.1:8719", token="alice-dev-token")
graph = client.graph("alice_memory")

source = graph.add_node("source")
target = graph.add_node("target")
graph.add_edge(source, target, "P", properties={"probability": "0.5"})

scores = graph.propagate({source: 1.0}, steps=1, edge_type="P")

parent = graph.add_variable("binary", owner_id=source, prior={"p": 0.6})
child = graph.add_variable("binary", owner_id=target)
graph.add_cpd(child, [parent], [0.9, 0.1, 0.2, 0.8])

result = graph.belief_propagation(
    [child],
    evidence={parent: "true"},
    damping=0.0,
    persist=True,
)
posterior = graph.posterior(child)

print(scores, result["beliefs"], posterior)
```

`persist=false` belief propagation is a read operation. `persist=true` stores the
posterior and trace, so it requires graph write access.

## Auth Management

Administrators can create users, grant graph access, rotate tokens, and disable
users without restarting the server:

```bash
curl -X POST http://127.0.0.1:8719/admin/users \
  -H 'Authorization: Bearer admin-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"user_id":"bob","token":"bob-dev-token","graphs":{"alice_memory":"read"}}'

curl -X POST http://127.0.0.1:8719/admin/users/bob/token \
  -H 'Authorization: Bearer admin-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{}'

curl -X PATCH http://127.0.0.1:8719/admin/users/bob \
  -H 'Authorization: Bearer admin-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"disabled":true}'
```

The generated token is returned only by the rotation response. User list and get
responses report `has_token` but do not expose token values. Dynamic users and
token overrides are stored in `<data_dir>/server-state.json`; do not commit real
tokens.

```python
admin = TongGraphClient("http://127.0.0.1:8719", token="admin-dev-token")
admin.create_user("bob", token="bob-dev-token", graphs={"alice_memory": "read"})
rotated = admin.rotate_user_token("bob")
admin.update_user("bob", disabled=True)
```

## Backup And Restore

Administrators can create local `.tar.gz` graph backups and restore them as new
or existing graphs:

```bash
curl -X POST http://127.0.0.1:8719/admin/graphs/alice_memory/backup \
  -H 'Authorization: Bearer admin-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"note":"before migration"}'

curl http://127.0.0.1:8719/admin/backups \
  -H 'Authorization: Bearer admin-dev-token'

curl -X POST http://127.0.0.1:8719/admin/backups/$BACKUP_ID/restore \
  -H 'Authorization: Bearer admin-dev-token' \
  -H 'Content-Type: application/json' \
  -d '{"graph":"alice_memory_copy","grants":{"alice":"write"}}'
```

Backups are stored under `<data_dir>/backups/` and include the graph SQLite file,
optional WAL/SHM files, `.segments/` sidecar files, and graph metadata. In-memory
snapshots are not backed up.

```python
admin = TongGraphClient("http://127.0.0.1:8719", token="admin-dev-token")
backup = admin.backup_graph("alice_memory", note="before migration")
restored = admin.restore_backup(
    backup["backup_id"],
    "alice_memory_copy",
    grants={"alice": "write"},
)
admin.delete_backup(backup["backup_id"])
```


## Bare-Metal Deployment

The repository includes deployment assets for a single-node local or internal
network service:

```text
deploy/tonggraph-server.yml
deploy/tonggraph-server.env.example
deploy/systemd/tonggraph-server.service
scripts/server/start.sh
scripts/server/health.sh
scripts/server/smoke.sh
```

Use the template config and environment file as a starting point. The template
binds to `127.0.0.1:8719` and reads tokens from environment variables. The
start script automatically loads `deploy/tonggraph-server.env` when it exists.

```bash
cp deploy/tonggraph-server.env.example deploy/tonggraph-server.env
# Edit deploy/tonggraph-server.env and set private token values.
./scripts/server/start.sh
```

The start script reads `deploy/tonggraph-server.yml` by default. Override it for
a copied production config:

```bash
TONGGRAPH_CONFIG=/etc/tonggraph/tonggraph-server.yml \
TONGGRAPH_HOST=127.0.0.1 \
TONGGRAPH_PORT=8719 \
./scripts/server/start.sh
```

Check health and run a minimal smoke test against an already running server:

```bash
TONGGRAPH_BASE_URL=http://127.0.0.1:8719 ./scripts/server/health.sh

TONGGRAPH_BASE_URL=http://127.0.0.1:8719 \
TONGGRAPH_ADMIN_TOKEN="$TONGGRAPH_ADMIN_TOKEN" \
./scripts/server/smoke.sh
```

For systemd, copy the config and env file to `/etc/tonggraph/`, install the
unit from `deploy/systemd/tonggraph-server.service`, then adjust
`WorkingDirectory`, `ExecStart`, and the service user for your installation
path.

```bash
sudo install -d /etc/tonggraph
sudo cp deploy/tonggraph-server.yml /etc/tonggraph/tonggraph-server.yml
sudo cp deploy/tonggraph-server.env.example /etc/tonggraph/tonggraph-server.env
sudo cp deploy/systemd/tonggraph-server.service /etc/systemd/system/tonggraph-server.service
sudo systemctl daemon-reload
sudo systemctl enable --now tonggraph-server
```

The deployment assets are intentionally bare-metal first. Docker, Compose,
Kubernetes, TLS termination, and public-network hardening are left to later
deployment work.

## Operations

```bash
curl -H 'Authorization: Bearer admin-dev-token' \
  http://127.0.0.1:8719/metrics
```

`/metrics` returns JSON request counters, latency totals, status and route
counts, uptime, and graph summaries. In token auth mode it requires an admin
token.

## Vector Benchmark

Run the local exact vector benchmark when sizing server deployments:

```bash
uv run python -m tests.benchmark.gbench.vector \
  --vectors 10000 \
  --dimensions 128 \
  --queries 20 \
  --batch-size 8 \
  --repeat 3 \
  --output tests/benchmark/.gbench/results/vector-exact-10k.json
```

The benchmark reports embedded and HTTP server exact-search latency in JSON.
Generated results live under `tests/benchmark/.gbench/`, which is gitignored.

## Current Boundaries

The current server is single-node and internal-network oriented. Snapshot
resources are in-memory and expire by TTL. The server does not provide
distributed storage, public multi-tenant hosting, Docker/Compose deployment
assets, TLS termination, or fine-grained node and edge permissions.
