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
distributed storage, public multi-tenant hosting, or fine-grained node and edge
permissions.
