# Server Agent Graph Client 中文指南

本文面向 **客户端 Agent** 或上层应用：它们已经产出了类似
`tmp_data/q450/trajectory/final_graph.json` 的图数据，希望通过 TongGraph
Server 进行图数据存取、检索、查询和稳定读取。

这里的客户端不直接打开 SQLite，也不直接创建 `Graph(path)`，而是通过
HTTP 或 `TongGraphClient` 访问已经部署好的 TongGraph Server。

## 目录 { #toc }

- [安装 TongGraph](#install)
- [数据形态](#data-shape)
- [推荐映射](#mapping)
- [连接服务](#connect)
- [管理 graph 和授权](#manage-graph)
- [写入 final_graph.json](#ingest-final-graph)
- [多个逻辑图上的操作](#multi-logical-graphs)
- [批量写入的 HTTP 形式](#http-batch-write)
- [检索服务](#retrieval)
- [查询和图遍历](#query-traversal)
- [Snapshot 稳定读取](#snapshot)
- [权限和错误处理](#auth-errors)
- [建议的客户端工作流](#workflow)

## 安装 TongGraph { #install }

客户端如果使用 Python HTTP client，需要安装 server extra，因为
`TongGraphClient` 位于 `tonggraph.server.client`：

```bash
pip install 'tonggraph[server]'
```

如果是在本仓库开发环境中运行客户端或服务端：

```bash
uv sync --extra server --dev
```

服务端部署同样需要安装 `tonggraph[server]`，这样才会包含 FastAPI、Uvicorn
和 `tonggraph-server` 启动入口。只使用 curl 或其他语言直接调用 HTTP API 的客户端
不需要安装 Python 包，但仍需要拿到服务地址、token、物理 graph 名和
`logical_graph_id`。

```python
from tonggraph.server.client import TongGraphClient

client = TongGraphClient("http://127.0.0.1:8719", token="client-token")
```

## 数据形态 { #data-shape }

`final_graph.json` 当前是一种 agent 运行轨迹图，顶层结构类似：

```json
{
  "generated_at": "2026-06-25T12:52:28.020451+00:00",
  "item_id": "trajectory",
  "nodes": [...],
  "beliefs": [...],
  "decisions": [...],
  "relations": [...],
  "merges": [...],
  "sessions": []
}
```

其中节点记录通常包含：

```json
{
  "id": 0,
  "node_type": "belief",
  "belief": "...",
  "stance": "asserted",
  "entities": ["..."],
  "event_time": null,
  "time_text": "1-9-2020",
  "source": {...},
  "evidence": [...],
  "confidence": 0.72
}
```

关系记录通常包含：

```json
{
  "from_id": 1,
  "to_id": 0,
  "type": "depends_on",
  "note": "..."
}
```

## 推荐映射 { #mapping }

把 agent 图写入 TongGraph Server 时，推荐保持一套稳定映射：

| Agent JSON 字段 | TongGraph 映射 |
|---|---|
| `item_id` | 推荐作为 `logical_graph_id`，也写入每个 node 的 properties |
| node `id` | `external_id="agent:{logical_graph_id}:node:{id}"` |
| node `node_type` | label，例如 `AgentNode`、`Belief`、`Decision` |
| node `belief` / `decision` | properties 中的 `text` / `belief` / `decision` |
| node `stance` | property `stance` |
| node `entities` | property `entities` |
| node `source` / `evidence` | properties 中保留原 JSON |
| relation `type` | edge type，建议转成大写安全字符串 |
| relation `note` | edge property `note` |
| relation `from_id/to_id` | 通过 external_id 映射到 TongGraph 内部 node id |

这种做法有两个好处：

- Agent 仍然可以用自己的 `id` 进行幂等查找和跨系统引用。
- TongGraph 内部 ID 可由服务端生成，客户端不用假设内部 ID 等于 JSON 里的 `id`。

## 连接服务 { #connect }

客户端需要三类信息：

```text
TONGGRAPH_BASE_URL=http://127.0.0.1:8719
TONGGRAPH_TOKEN=...
TONGGRAPH_GRAPH=agent_workspace
TONGGRAPH_LOGICAL_GRAPH_ID=q450
```

Python client：

```python
from tonggraph.server.client import TongGraphClient

client = TongGraphClient(
    "http://127.0.0.1:8719",
    token="client-token",
)
graph = client.graph("agent_workspace").logical("q450")
```

curl 请求需要带 Bearer token：

```bash
curl http://127.0.0.1:8719/graphs \
  -H "Authorization: Bearer $TONGGRAPH_TOKEN"
```

## 管理 graph 和授权 { #manage-graph }

管理员只需要创建一个启用逻辑图命名空间的物理 graph，并授予客户端用户写权限。
之后客户端可以在同一个 db 文件里自助创建多个 `logical_graph_id`，不用再找管理员
为每个 agent run 新建 db 文件。

```python
admin = TongGraphClient("http://127.0.0.1:8719", token="admin-token")
admin.create_graph("agent_workspace", grants={"agent_writer": "write"}, logical_graphs=True)
```

如果用户已经存在，也可以只授予 graph 权限：

```python
admin.grant_graph("agent_workspace", "agent_writer", "write")
admin.grant_graph("agent_workspace", "agent_reader", "read")
```

对应 curl：

```bash
curl -X POST http://127.0.0.1:8719/admin/graphs \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"agent_workspace","logical_graphs":true,"grants":{"agent_writer":"write"}}'
```

客户端创建或复用自己的逻辑图：

```python
client = TongGraphClient("http://127.0.0.1:8719", token="client-token")
workspace = client.graph("agent_workspace")
workspace.create_logical_graph("q450")
graph = workspace.logical("q450")
```

HTTP 也可以直接创建逻辑图：

```bash
curl -X POST http://127.0.0.1:8719/graphs/agent_workspace/logical-graphs \
  -H "Authorization: Bearer $TONGGRAPH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"logical_graph_id":"q450"}'
```

## 写入 final_graph.json { #ingest-final-graph }

下面的示例会：

1. 读取 `final_graph.json`。
2. 批量写入 nodes。
3. 根据 `relations` 批量写入 edges。
4. 创建全文索引，用于按 belief/evidence 文本检索。

```python
from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

from tonggraph.server.client import TongGraphClient

BASE_URL = "http://127.0.0.1:8719"
TOKEN = "client-token"
GRAPH = "agent_workspace"
LOGICAL_GRAPH_ID = "q450"
INPUT = Path("tmp_data/q450/trajectory/final_graph.json")


def edge_type(value: str) -> str:
    normalized = re.sub(r"[^A-Za-z0-9_]+", "_", value.strip()).upper()
    return normalized or "RELATED_TO"


def labels_for(node: dict[str, Any]) -> list[str]:
    labels = ["AgentNode"]
    kind = str(node.get("node_type") or "").strip().lower()
    if kind == "belief":
        labels.append("Belief")
    elif kind == "decision":
        labels.append("Decision")
    else:
        labels.append("AgentRecord")
    return labels


def properties_for(node: dict[str, Any], *, item_id: str, generated_at: str | None) -> dict[str, Any]:
    text = node.get("belief") or node.get("decision") or ""
    return {
        "agent_id": node.get("id"),
        "item_id": item_id,
        "generated_at": generated_at,
        "node_type": node.get("node_type"),
        "belief": node.get("belief"),
        "decision": node.get("decision"),
        "text": text,
        "stance": node.get("stance"),
        "entities": node.get("entities") or [],
        "event_time": node.get("event_time"),
        "time_text": node.get("time_text"),
        "source": node.get("source") or {},
        "evidence": node.get("evidence") or [],
        "supporting_excerpts": node.get("supporting_excerpts") or [],
        "confidence": node.get("confidence"),
        "initial_confidence": node.get("initial_confidence"),
        "confidence_history": node.get("confidence_history") or [],
    }


def ingest_final_graph(path: Path) -> dict[int, int]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    item_id = str(payload.get("item_id") or path.parent.name)
    generated_at = payload.get("generated_at")

    client = TongGraphClient(BASE_URL, token=TOKEN)
    graph = client.graph(GRAPH).logical(LOGICAL_GRAPH_ID)
    graph.create()

    records = []
    for node in payload.get("nodes", []):
        agent_id = node["id"]
        records.append(
            {
                "external_id": f"agent:{LOGICAL_GRAPH_ID}:node:{agent_id}",
                "labels": labels_for(node),
                "properties": properties_for(node, item_id=item_id, generated_at=generated_at),
            }
        )

    internal_ids = graph.add_nodes(records)
    id_map = {
        node["id"]: internal_id
        for node, internal_id in zip(payload.get("nodes", []), internal_ids, strict=True)
    }

    edge_records = []
    for relation in payload.get("relations", []):
        source = id_map.get(relation.get("from_id"))
        target = id_map.get(relation.get("to_id"))
        if source is None or target is None:
            continue
        edge_records.append(
            {
                "source": source,
                "target": target,
                "edge_type": edge_type(str(relation.get("type") or "related_to")),
                "properties": {
                    "type": relation.get("type"),
                    "note": relation.get("note"),
                    "item_id": item_id,
                },
            }
        )

    if edge_records:
        graph.add_edges(edge_records)

    # 索引可重复创建时请在业务侧捕获 conflict/invalid_request；这里展示首次初始化。
    graph.create_fulltext_index("agent_text", ["text", "belief", "decision"], target="node")
    return id_map


id_map = ingest_final_graph(INPUT)
print("ingested", len(id_map), "nodes")
```

如果需要幂等导入，不要直接重复 `add_nodes()`。建议先用
`get_node_id(external_id)` 判断是否已存在，或把每次 agent run 写入不同
`logical_graph_id` namespace。

启用逻辑图模式后，非管理员调用节点、边、检索、查询和图算法 API 时必须带
`logical_graph_id`。Python 的 `graph = client.graph("agent_workspace").logical("q450")`
会自动帮你带上这个字段；HTTP 客户端需要显式传。

### 多个逻辑图上的操作 { #multi-logical-graphs }

当前 server 的一次数据请求只作用于一个 `logical_graph_id`。如果客户端需要在
多个逻辑图上检索、查询或统计，需要在客户端循环调用多个逻辑图，再合并结果。
这样做的好处是每次请求的边界明确，不会意外把一个逻辑图的 traversal / query
扩展到另一个逻辑图。

Python client 推荐写法：

```python
workspace = client.graph("agent_workspace")
logical_graph_ids = ["q450", "q451", "q452"]

all_rows = []
for logical_graph_id in logical_graph_ids:
    graph = workspace.logical(logical_graph_id)
    rows = graph.search_vector(
        "agent_embedding",
        query_vector,
        labels=["Belief"],
        limit=10,
    )
    for row in rows:
        row["logical_graph_id"] = logical_graph_id
    all_rows.extend(rows)

# 多个逻辑图的结果需要由客户端决定如何排序、截断和去重。
all_rows.sort(key=lambda row: row.get("score", 0.0), reverse=True)
top_rows = all_rows[:10]
```

全文检索、`retrieve_context()`、结构化 `query()`、`node_count()`、`neighbors()`
等方法也使用同样模式：先 `workspace.logical(logical_graph_id)`，再调用对应方法。
如果使用 HTTP，则对每个逻辑图分别发送请求：

```bash
for logical_graph_id in q450 q451 q452; do
  curl -X POST "$TONGGRAPH_BASE_URL/graphs/$TONGGRAPH_GRAPH/vector/agent_embedding/search"     -H "Authorization: Bearer $TONGGRAPH_TOKEN"     -H "Content-Type: application/json"     -d "{"logical_graph_id":"$logical_graph_id","query_vector":[0.1,0.2,0.3],"labels":["Belief"],"limit":10}"
done
```

注意：多个逻辑图之间不会自动做跨图 traversal、shortest path 或 PageRank；如果需要
跨逻辑图的全局图算法，应把这些数据写入同一个 `logical_graph_id`，或由客户端先合并
候选结果后再做业务层计算。

## 批量写入的 HTTP 形式 { #http-batch-write }

Python client 推荐用于业务代码；非 Python 客户端可以直接调用 HTTP。

批量写 nodes：

```bash
curl -X POST "$TONGGRAPH_BASE_URL/graphs/$TONGGRAPH_GRAPH/nodes/batch" \
  -H "Authorization: Bearer $TONGGRAPH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"logical_graph_id":"q450","records":[{"external_id":"agent:q450:node:0","labels":["AgentNode","Belief"],"properties":{"text":"...","stance":"asserted"}}]}'
```

批量写 edges：

```bash
curl -X POST "$TONGGRAPH_BASE_URL/graphs/$TONGGRAPH_GRAPH/edges/batch" \
  -H "Authorization: Bearer $TONGGRAPH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"logical_graph_id":"q450","records":[{"source":0,"target":1,"edge_type":"DEPENDS_ON","properties":{"note":"..."}}]}'
```

## 检索服务 { #retrieval }

### 全文检索 { #fulltext }

导入后可以按 agent belief / decision 文本检索：

```python
rows = graph.search_text(
    "agent_text",
    "places of worship essential services",
    labels=["Belief"],
    limit=10,
)
for row in rows:
    print(row["score"], row["record"]["properties"]["text"])
```

HTTP：

```bash
curl -X POST "$TONGGRAPH_BASE_URL/graphs/$TONGGRAPH_GRAPH/fulltext/agent_text/search" \
  -H "Authorization: Bearer $TONGGRAPH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"logical_graph_id":"q450","query":"places of worship essential services","labels":["Belief"],"limit":10}'
```

### Embedding 索引存取与检索 { #embedding-index }

TongGraph Server 不负责生成 embedding。客户端 Agent 需要自己调用 embedding
模型，然后把向量写入 server。TongGraph 负责保存向量、按 index 管理向量，
并提供 exact vector search。

推荐把 agent 节点写入后得到的 TongGraph 内部 node id 作为 embedding 的 key：

```python
# internal_ids 来自 graph.add_nodes(...) 的返回值。
# embeddings: dict[int, list[float]]，key 是 TongGraph 内部 node id。
embeddings = {
    internal_ids[0]: embedding_for_node_0,
    internal_ids[1]: embedding_for_node_1,
}
```

创建 embedding index：

```python
graph.create_vector_index(
    "agent_embedding",
    dimensions=1536,
    target="node",
    metric="cosine",
    model="your-embedding-model",
    model_version="optional-version",
)
```

查看已有 index：

```python
indexes = graph.vector_indexes()
```

写入或覆盖 embedding：

```python
# 单条写入或覆盖。
graph.upsert_vector("agent_embedding", internal_ids[0], embedding_for_node_0)

# 批量写入或覆盖，适合 agent 一次产出很多节点。
graph.upsert_vectors("agent_embedding", embeddings)
```

读取已保存的 embedding：

```python
vector = graph.get_vector("agent_embedding", internal_ids[0])
```

删除 embedding 或整个 index：

```python
graph.delete_vector("agent_embedding", internal_ids[0])
graph.delete_vectors("agent_embedding", [internal_ids[1], internal_ids[2]])
graph.drop_vector_index("agent_embedding")
```

检索相似节点：

```python
nearest = graph.search_vector(
    "agent_embedding",
    query_vector,
    labels=["Belief"],
    limit=10,
)

batch_nearest = graph.search_vectors(
    "agent_embedding",
    [query_vector_1, query_vector_2],
    labels=["Belief"],
    limit=10,
)
```

HTTP API 对应关系：

| 操作 | HTTP |
|---|---|
| 列出 index | `GET /graphs/{graph}/vector/indexes` |
| 创建 index | `POST /graphs/{graph}/vector/indexes` |
| 删除 index | `DELETE /graphs/{graph}/vector/indexes/{index}` |
| 写入单条 vector | `PUT /graphs/{graph}/vector/{index}/{entity_id}` |
| 写入多条 vector | `PUT /graphs/{graph}/vector/{index}/batch` |
| 读取 vector | `GET /graphs/{graph}/vector/{index}/{entity_id}` |
| 删除单条 vector | `DELETE /graphs/{graph}/vector/{index}/{entity_id}` |
| 删除多条 vector | `POST /graphs/{graph}/vector/{index}/delete-batch` |
| 单 query 检索 | `POST /graphs/{graph}/vector/{index}/search` |
| 多 query 检索 | `POST /graphs/{graph}/vector/{index}/search-batch` |

curl 示例。这里为了让示例短一些使用 3 维向量；真实 embedding 要把
`dimensions` 和向量长度改成模型实际输出维度，例如 768、1024 或 1536：

```bash
curl -X POST "$TONGGRAPH_BASE_URL/graphs/$TONGGRAPH_GRAPH/vector/indexes" \
  -H "Authorization: Bearer $TONGGRAPH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"agent_embedding","dimensions":3,"target":"node","metric":"cosine"}'

curl -X PUT "$TONGGRAPH_BASE_URL/graphs/$TONGGRAPH_GRAPH/vector/agent_embedding/batch" \
  -H "Authorization: Bearer $TONGGRAPH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"logical_graph_id":"q450","vectors":{"0":[0.1,0.2,0.3],"1":[0.2,0.1,0.4]}}'

curl -X POST "$TONGGRAPH_BASE_URL/graphs/$TONGGRAPH_GRAPH/vector/agent_embedding/search" \
  -H "Authorization: Bearer $TONGGRAPH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"logical_graph_id":"q450","query_vector":[0.1,0.2,0.3],"labels":["Belief"],"limit":10}'
```

注意事项：

- `dimensions` 必须和写入的 embedding 长度一致。
- `metric="cosine"` 时不能写入零向量。
- `target="node"` 表示 vector 绑定到 node id；如果要给 edge 建 embedding，使用 `target="edge"`。
- `upsert_vector()` / `upsert_vectors()` 是覆盖写；同一个 entity id 再写一次会替换旧向量。
- 删除 node/edge 后，相关向量会从兼容 index 中移除；如果只是更新 embedding，直接 upsert 即可。
- 当前实现是 exact scan，适合中小规模本地/内网检索；大规模 ANN/HNSW 不是当前 server 能力。

### 组合检索 retrieve_context { #retrieve-context }

`retrieve_context()` 会把全文/向量候选和图邻域扩展合在一起，适合 Agent
做上下文召回。

```python
context = graph.retrieve_context(
    text_index="agent_text",
    text_query="Trump places of worship essential services",
    vector_index="agent_embedding",
    vector_query=query_vector,
    labels=["Belief"],
    radius=1,
    direction="both",
    limit=20,
)
```

返回结果是 JSON-compatible dict/list，核心字段包括：

```text
kind, id, score, distance, source_scores, record
```

其中 `record` 是序列化后的 node 或 edge。

## 查询和图遍历 { #query-traversal }

按 `external_id` 找回内部 node id：

```python
node_id = graph.get_node_id("agent:trajectory:node:0")
```

结构化查询：

```python
rows = graph.query(
    {
        "match": [
            {"node": "a", "labels": ["Belief"], "properties": {"stance": "asserted"}},
            {"edge": "r", "type": "DEPENDS_ON", "direction": "out"},
            {"node": "b"},
        ],
        "return": ["a", "b"],
        "limit": 20,
    }
)
```

遍历邻居：

```python
neighbors = graph.neighbors(node_id, direction="both")
frontier = graph.k_hop(node_id, hops=2, direction="both")
```

## Snapshot 稳定读取 { #snapshot }

如果客户端需要在一轮 Agent 推理中使用稳定视图，可以先创建 snapshot：

```python
snapshot = graph.create_snapshot(ttl_seconds=600)

# 后续 live graph 即使继续写入，snapshot 结果也保持创建时状态。
rows = snapshot.search_text("agent_text", "essential services", labels=["Belief"])
stable = snapshot.query({"match": [{"node": "n", "labels": ["Belief"]}], "limit": 10})

snapshot.delete()
```

Snapshot 是内存 TTL 资源，不会跨 server restart 保留。

## 权限和错误处理 { #auth-errors }

- `read` 用户可以读取、查询、检索、创建 snapshot。
- `write` 用户包含 `read` 权限，并可以写 node/edge、索引和向量。
- `admin` 用户可以创建 graph、创建用户、授权、备份恢复。

Python client 会把服务端错误映射成 `TongGraphServerError`：

```python
from tonggraph.server.client import TongGraphClient, TongGraphServerError

try:
    graph.add_node("blocked")
except TongGraphServerError as exc:
    print(exc.code, exc.status_code, exc.graph, exc.request_id)
```

常见错误：

| HTTP | code | 含义 |
|---:|---|---|
| 401 | `unauthenticated` | token 缺失或无效 |
| 403 | `permission_denied` / `admin_required` | 用户无 graph 权限或不是管理员 |
| 404 | `not_found` / `graph_not_found` | graph、node、edge、snapshot 不存在 |
| 409 | `conflict` | graph 或用户已存在 |
| 422 | `invalid_request` | 请求体字段不合法 |

排查问题时优先记录 `request_id`，服务端日志也会带同一个 request id。

## 建议的客户端工作流 { #workflow }

1. 管理员为项目或 Agent 创建 graph，例如 `agent_memory`。
2. 管理员创建 writer/reader token，并给 graph 授权。
3. Agent 每次产出 `final_graph.json` 后，把 agent node id 映射成稳定 external_id。
4. 使用 `add_nodes()` / `add_edges()` 批量写入。
5. 创建全文索引和可选向量索引。
6. 业务查询优先使用 `search_text()`、`search_vector()`、`retrieve_context()` 和 `query()`。
7. 长推理流程使用 snapshot 获取稳定视图。
8. 定期由管理员调用 backup API 备份 graph。
