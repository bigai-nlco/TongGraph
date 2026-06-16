use super::*;
use crate::models::{PropertyMap, PropertyValue};
use std::collections::HashMap;
use std::fs;

#[test]
fn in_memory_graph_supports_retrieval_indexes_and_k_hop() {
    let mut graph = GraphCore::new();
    let a = graph
        .add_node(Some("a".to_string()), vec!["Person".to_string()], map([]))
        .unwrap();
    let b = graph
        .add_node(Some("b".to_string()), vec!["Person".to_string()], map([]))
        .unwrap();
    let c = graph
        .add_node(Some("c".to_string()), vec!["Claim".to_string()], map([]))
        .unwrap();

    graph
        .add_edge(a, b, "KNOWS".to_string(), map([("probability", "0.5")]))
        .unwrap();
    graph
        .add_edge(b, c, "SUPPORTS".to_string(), map([("probability", "0.25")]))
        .unwrap();

    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 2);
    assert_eq!(graph.get_node_id("a"), Some(a));
    assert_eq!(graph.nodes_with_label("Person"), vec![a, b]);
    assert_eq!(graph.edges_by_type("KNOWS"), vec![0]);
    assert_eq!(graph.neighbors(a, "out", None).unwrap(), vec![b]);
    assert_eq!(
        graph.neighbors(a, "out", Some("SUPPORTS")).unwrap(),
        Vec::<u64>::new()
    );
    assert_eq!(graph.k_hop(a, 2, "out", None).unwrap(), vec![b, c]);
}

#[test]
fn sqlite_store_persists_graph_and_rebuilds_indexes() {
    let path = temp_db_path("tonggraph-persist");
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        let a = graph
            .add_node(
                Some("a".to_string()),
                vec!["Entity".to_string()],
                map([("name", "Alice")]),
            )
            .unwrap();
        let b = graph
            .add_node(
                Some("b".to_string()),
                vec!["Entity".to_string()],
                map([("name", "Bob")]),
            )
            .unwrap();
        graph
            .add_edge(a, b, "LINKS".to_string(), map([("probability", "0.75")]))
            .unwrap();
    }

    let graph = GraphCore::open(path.to_str().unwrap()).unwrap();
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.get_node_id("a"), Some(0));
    assert_eq!(graph.nodes_with_label("Entity"), vec![0, 1]);
    assert_eq!(graph.neighbors(0, "out", Some("LINKS")).unwrap(), vec![1]);
    assert_eq!(
        graph.get_edge(0).unwrap().properties["probability"].encoded_value(),
        "0.75"
    );

    cleanup_db(&path);
}

#[test]
fn weighted_probability_propagation_uses_sparse_edges() {
    let mut graph = GraphCore::new();
    let a = graph.add_node(None, Vec::new(), map([])).unwrap();
    let b = graph.add_node(None, Vec::new(), map([])).unwrap();
    let c = graph.add_node(None, Vec::new(), map([])).unwrap();
    graph
        .add_edge(a, b, "P".to_string(), map([("probability", "0.5")]))
        .unwrap();
    graph
        .add_edge(b, c, "P".to_string(), map([("probability", "0.25")]))
        .unwrap();

    let result = graph
        .propagate(&HashMap::from([(a, 1.0)]), 2, "probability", 1.0)
        .unwrap();
    assert_eq!(result.get(&a), Some(&1.0));
    assert_eq!(result.get(&b), Some(&0.5));
    assert_eq!(result.get(&c), Some(&0.125));
}

#[test]
fn graph_compute_runtime_supports_v03_algorithms() {
    let mut graph = GraphCore::new();
    let a = graph.add_node(None, Vec::new(), map([])).unwrap();
    let b = graph.add_node(None, Vec::new(), map([])).unwrap();
    let c = graph.add_node(None, Vec::new(), map([])).unwrap();
    let d = graph.add_node(None, Vec::new(), map([])).unwrap();
    let e = graph.add_node(None, Vec::new(), map([])).unwrap();
    graph
        .add_edge(a, b, "LINK".to_string(), map([("weight", "2.0")]))
        .unwrap();
    graph
        .add_edge(a, c, "LINK".to_string(), map([("weight", "1.0")]))
        .unwrap();
    graph
        .add_edge(c, b, "LINK".to_string(), map([("weight", "0.5")]))
        .unwrap();
    graph
        .add_edge(b, d, "LINK".to_string(), map([("weight", "1.0")]))
        .unwrap();

    assert_eq!(graph.bfs(a, "out", None, Some(1)).unwrap(), vec![a, b, c]);
    let path = graph
        .shortest_path(a, b, "out", None, Some("weight"))
        .unwrap()
        .unwrap();
    assert_eq!(path.nodes, vec![a, c, b]);
    assert!((path.distance - 1.5).abs() < 1e-9);
    assert_eq!(
        graph.connected_components(None).unwrap(),
        vec![vec![a, b, c, d], vec![e]]
    );

    let ranks = graph.pagerank(20, 0.85, Some(1e-12), None).unwrap();
    assert_eq!(ranks.len(), 5);
    assert!((ranks.values().sum::<f64>() - 1.0).abs() < 1e-9);
    assert_eq!(
        graph.random_walk(a, 4, "out", None, Some(7)).unwrap(),
        graph.random_walk(a, 4, "out", None, Some(7)).unwrap()
    );

    let subgraph = graph.subgraph(&[a, b, c], None).unwrap();
    assert_eq!(subgraph.node_count(), 3);
    assert_eq!(subgraph.edge_count(), 3);
    assert!(subgraph.get_edge(2).is_some());
    assert!(subgraph.get_edge(3).is_none());
}

#[test]
fn compute_batch_returns_ordered_results() {
    let mut graph = GraphCore::new();
    let a = graph.add_node(None, Vec::new(), map([])).unwrap();
    let b = graph.add_node(None, Vec::new(), map([])).unwrap();
    graph
        .add_edge(a, b, "LINK".to_string(), map([("weight", "3.0")]))
        .unwrap();

    let results = graph
        .compute_batch(&[
            ComputeJob::Bfs {
                start: a,
                direction: "out".to_string(),
                edge_type: None,
                max_depth: None,
            },
            ComputeJob::ShortestPath {
                start: a,
                target: b,
                direction: "out".to_string(),
                edge_type: None,
                weight_property: Some("weight".to_string()),
            },
        ])
        .unwrap();

    match &results[0] {
        ComputeResult::Nodes(nodes) => assert_eq!(nodes, &vec![a, b]),
        _ => panic!("expected BFS node result"),
    }
    match &results[1] {
        ComputeResult::ShortestPath(Some(path)) => {
            assert_eq!(path.nodes, vec![a, b]);
            assert!((path.distance - 3.0).abs() < 1e-9);
        }
        _ => panic!("expected shortest-path result"),
    }
}

#[test]
fn sqlite_segments_persist_after_manual_compaction_and_reopen() {
    let path = temp_db_path("tonggraph-segment");
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        let a = graph.add_node(None, Vec::new(), map([])).unwrap();
        let b = graph.add_node(None, Vec::new(), map([])).unwrap();
        graph.add_edge(a, b, "LINK".to_string(), map([])).unwrap();
        graph.compact_segments().unwrap();
    }

    assert!(segment_manifest_path(&path).exists());
    let reopened = GraphCore::open(path.to_str().unwrap()).unwrap();
    assert_eq!(reopened.neighbors(0, "out", None).unwrap(), vec![1]);
    cleanup_db(&path);
}

#[test]
fn sqlite_graph_auto_compacts_large_delta_overlay() {
    let path = temp_db_path("tonggraph-auto-compact");
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        let a = graph.add_node(None, Vec::new(), map([])).unwrap();
        let b = graph.add_node(None, Vec::new(), map([])).unwrap();
        for _ in 0..1025 {
            graph.add_edge(a, b, "LINK".to_string(), map([])).unwrap();
        }
    }

    assert!(segment_manifest_path(&path).exists());
    let reopened = GraphCore::open(path.to_str().unwrap()).unwrap();
    assert_eq!(reopened.edge_count(), 1025);
    cleanup_db(&path);
}

fn map<const N: usize>(values: [(&str, &str); N]) -> PropertyMap {
    values
        .into_iter()
        .map(|(key, value)| (key.to_string(), PropertyValue::String(value.to_string())))
        .collect()
}

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let unique = format!(
        "{}-{}-{}.db",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

fn segment_manifest_path(path: &std::path::Path) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}.segments", path.to_string_lossy())).join("manifest.txt")
}

fn cleanup_db(path: &std::path::Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_extension("db-shm"));
    let _ = fs::remove_file(path.with_extension("db-wal"));
    let _ = fs::remove_dir_all(std::path::PathBuf::from(format!(
        "{}.segments",
        path.to_string_lossy()
    )));
}
