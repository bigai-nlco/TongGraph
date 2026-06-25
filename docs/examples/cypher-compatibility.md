# Cypher Compatibility

TongGraph exposes an embedded Cypher-compatible query surface through
`Graph.cypher()`. The current implementation targets the documented supported
subset, including `MATCH`, `OPTIONAL MATCH`, `WHERE`, `RETURN`, `ORDER BY`,
`SKIP`, `LIMIT`, `CREATE`, `MERGE`, `SET`, `REMOVE`, `DELETE`,
`DETACH DELETE`, `UNION`, parameters, result records, and local staged
transactions.

See [Cypher Compatibility](../design/cypher-compatibility.md) for the current
compatibility matrix and explicit non-goals.

## Run Cypher against a graph

```python
from tonggraph import Graph

graph = Graph()

created = graph.cypher(
    """
    CREATE (alice:Person {external_id: 'alice', name: 'Alice', rank: 3})
           -[:KNOWS {since: 2026}]->
           (bob:Person {external_id: 'bob', name: 'Bob', rank: 2})
    RETURN alice, bob
    """
)

assert created.keys == ["alice", "bob"]
assert created.records[0]["alice"].properties["name"] == "Alice"

rows = graph.cypher(
    """
    MATCH (a:Person)-[r:KNOWS]->(b:Person)
    WHERE a.name = $name AND b.rank IN [$rank]
    RETURN a.name AS source, type(r) AS relationship, b.name AS target
    ORDER BY target
    """,
    {"name": "Alice", "rank": 2},
)

assert rows.records == [
    {"source": "Alice", "relationship": "KNOWS", "target": "Bob"}
]
```

`Graph.cypher()` returns a `CypherResult` with `keys`, `records`, and `summary`.
Records are dictionaries keyed by the projected Cypher aliases.

## Update and delete graph data

`SET` can update properties, merge a map, add labels, and change a node's unique
`external_id`. `REMOVE` removes scalar properties or node labels.

```python
updated = graph.cypher(
    """
    MATCH (a:Person)-[r:KNOWS]->(b:Person)
    WHERE a.external_id = 'alice'
    SET a.name = 'Alicia', a += {active: true}, a:Researcher, r.strength = 0.9
    REMOVE a.rank
    RETURN a.name AS name, labels(a) AS labels, r.strength AS strength
    """
)

assert updated.summary["properties_set"] == 3
assert updated.summary["properties_removed"] == 1
assert updated.summary["labels_added"] == 1
```

Plain `DELETE` requires relationships to be removed first. `DETACH DELETE`
removes a node and all incident relationships. Delete statements return counters
through `summary` and do not support `RETURN` in the current subset.

```python
deleted = graph.cypher(
    """
    MATCH (a {external_id: 'alice'})-[r:KNOWS]->(b {external_id: 'bob'})
    DELETE r, b
    """
)

assert deleted.summary["nodes_deleted"] == 1
assert deleted.summary["relationships_deleted"] == 1
```

The same operations are available without Cypher through `Graph.update_node`,
`Graph.update_edge`, `Graph.delete_node`, and `Graph.delete_edge`.

## Stage writes in a transaction

Explicit transactions stage changes until commit. Reads through the transaction
see staged writes; reads through the graph see only committed data.

```python
with graph.transaction() as tx:
    tx.run("CREATE (carol:Person {external_id: 'carol', name: 'Carol'})")

    staged = tx.run(
        """
        MATCH (p:Person)
        RETURN p.name AS name
        ORDER BY name
        """
    )
    assert [row["name"] for row in staged.records] == ["Alice", "Bob", "Carol"]

    committed = graph.cypher("MATCH (p:Person) RETURN count(*) AS total")
    assert committed.records == [{"total": 2}]

after_commit = graph.cypher("MATCH (p:Person) RETURN count(*) AS total")
assert after_commit.records == [{"total": 3}]
```

## Query a snapshot

Snapshots support read-only Cypher. Write and schema clauses are rejected.

```python
snapshot = graph.snapshot()

result = snapshot.cypher(
    """
    MATCH (p:Person)
    RETURN p.name AS name
    ORDER BY name
    """
)

assert [row["name"] for row in result.records] == ["Alice", "Bob", "Carol"]
```
