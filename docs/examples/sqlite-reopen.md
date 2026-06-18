# SQLite Reopen

This example writes graph records to SQLite, compacts adjacency data into a
segment sidecar, reopens the database, and checks that indexes and snapshots
still behave as expected.

=== "Python"

    ```python
    import json
    from pathlib import Path
    from tempfile import TemporaryDirectory
    from tonggraph import Graph

    with TemporaryDirectory() as tmp:
        db_path = Path(tmp) / "tonggraph.db"
        graph = Graph(str(db_path))
        source = graph.add_node("source", labels=["Entity"], properties={"name": "A"})
        target = graph.add_node("target", labels=["Entity"], properties={"name": "B"})
        graph.add_edge(source, target, "LINKS", properties={"probability": 0.75})

        snapshot = graph.snapshot()
        graph.compact()
        del graph

        reopened = Graph(str(db_path))
        result = {
            "source_id": source,
            "snapshot_node_count": snapshot.node_count(),
            "reopened_counts": {
                "nodes": reopened.node_count(),
                "edges": reopened.edge_count(),
            },
            "entity_nodes": reopened.nodes_with_label("Entity"),
            "links_neighbors": reopened.neighbors(source, edge_type="LINKS"),
            "segment_manifest_exists": Path(f"{db_path}.segments/manifest.txt").exists(),
        }

    print(json.dumps(result, indent=2, sort_keys=True))
    ```

Output:

```json
{
  "entity_nodes": [
    0,
    1
  ],
  "links_neighbors": [
    1
  ],
  "reopened_counts": {
    "edges": 1,
    "nodes": 2
  },
  "segment_manifest_exists": true,
  "snapshot_node_count": 2,
  "source_id": 0
}
```
