# AI Memory Session Graph

Model sessions, messages, entities, claims, tool calls, and evidence as graph
records. This keeps memory inspectable and allows retrieval by both structure
and properties.

```python
from tonggraph import Graph

memory = Graph()

session = memory.add_node(
    "session:2026-06-22",
    labels=["Session"],
    properties={"user": "demo", "topic": "graph memory"},
)
message = memory.add_node(
    "message:1",
    labels=["Message"],
    properties={"role": "user", "text": "Remember that Alice likes graph RAG."},
)
alice = memory.add_node(
    "entity:alice",
    labels=["Entity", "Person"],
    properties={"name": "Alice"},
)
claim = memory.add_node(
    "claim:1",
    labels=["Claim"],
    properties={"text": "Alice likes graph RAG.", "confidence": 0.92},
)

memory.add_edge(session, message, "HAS_MESSAGE")
memory.add_edge(message, claim, "ASSERTS")
memory.add_edge(claim, alice, "ABOUT")

rows = memory.query(
    {
        "match": [
            {"node": "s", "labels": ["Session"], "properties": {"user": "demo"}},
            {"edge": "hm", "type": "HAS_MESSAGE", "direction": "out"},
            {"node": "m", "labels": ["Message"]},
            {"edge": "asserts", "type": "ASSERTS", "direction": "out"},
            {"node": "c", "labels": ["Claim"]},
        ],
        "return": ["m", "c"],
        "limit": 20,
    }
)

for row in rows:
    print(memory.get_node(row["c"]).properties["text"])
```

Persist the memory graph when sessions should survive process restarts.
