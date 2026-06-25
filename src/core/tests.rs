use super::*;
use crate::models::{NewEdgeRecord, NewNodeRecord, PropertyMap, PropertyValue};
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
        .propagate(&HashMap::from([(a, 1.0)]), 2, None, "probability", 1.0)
        .unwrap();
    assert_eq!(result.get(&a), Some(&1.0));
    assert_eq!(result.get(&b), Some(&0.5));
    assert_eq!(result.get(&c), Some(&0.125));

    graph
        .add_edge(a, c, "Q".to_string(), map([("probability", "0.9")]))
        .unwrap();
    let filtered = graph
        .propagate(&HashMap::from([(a, 1.0)]), 2, Some("P"), "probability", 1.0)
        .unwrap();
    assert_eq!(filtered.get(&a), Some(&1.0));
    assert_eq!(filtered.get(&b), Some(&0.5));
    assert_eq!(filtered.get(&c), Some(&0.125));
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
fn structured_query_matches_path_patterns_and_filters() {
    let mut graph = GraphCore::new();
    let alice = graph
        .add_node(
            Some("alice".to_string()),
            vec!["Person".to_string()],
            typed_map([
                ("name", s("Alice")),
                ("rank", PropertyValue::Int(3)),
                ("active", PropertyValue::Bool(true)),
                ("group", s("ai")),
            ]),
        )
        .unwrap();
    let bob = graph
        .add_node(
            Some("bob".to_string()),
            vec!["Person".to_string()],
            typed_map([
                ("name", s("Bob")),
                ("rank", PropertyValue::Int(2)),
                ("active", PropertyValue::Bool(true)),
                ("group", s("research")),
            ]),
        )
        .unwrap();
    let carol = graph
        .add_node(
            Some("carol".to_string()),
            vec!["Person".to_string()],
            typed_map([
                ("name", s("Carol")),
                ("rank", PropertyValue::Int(4)),
                ("active", PropertyValue::Bool(false)),
                ("group", s("research")),
            ]),
        )
        .unwrap();
    let claim = graph
        .add_node(
            Some("claim".to_string()),
            vec!["Claim".to_string()],
            map([]),
        )
        .unwrap();

    let alice_knows_bob = graph
        .add_edge(
            alice,
            bob,
            "KNOWS".to_string(),
            typed_map([
                ("note", s("team alpha")),
                ("weight", PropertyValue::Float(0.8)),
            ]),
        )
        .unwrap();
    graph
        .add_edge(
            bob,
            carol,
            "KNOWS".to_string(),
            typed_map([
                ("note", s("team beta")),
                ("weight", PropertyValue::Float(0.6)),
            ]),
        )
        .unwrap();
    graph
        .add_edge(
            carol,
            alice,
            "KNOWS".to_string(),
            typed_map([("note", s("loop")), ("weight", PropertyValue::Float(0.3))]),
        )
        .unwrap();
    graph
        .add_edge(bob, claim, "SUPPORTS".to_string(), map([]))
        .unwrap();

    let rows = graph
        .query(&QuerySpec {
            elements: vec![
                QueryElement::Node(NodePattern {
                    alias: "a".to_string(),
                    id: None,
                    external_id: None,
                    labels: vec!["Person".to_string()],
                    properties: vec![
                        filter(
                            "rank",
                            PropertyOperator::Gte,
                            QueryValue::Scalar(PropertyValue::Int(2)),
                        ),
                        filter(
                            "group",
                            PropertyOperator::In,
                            QueryValue::List(vec![s("ai"), s("research")]),
                        ),
                    ],
                }),
                edge(
                    "rel",
                    Some("KNOWS"),
                    QueryDirection::Out,
                    vec![filter(
                        "note",
                        PropertyOperator::Contains,
                        QueryValue::Scalar(s("team")),
                    )],
                ),
                QueryElement::Node(NodePattern {
                    alias: "b".to_string(),
                    id: None,
                    external_id: None,
                    labels: vec!["Person".to_string()],
                    properties: vec![filter(
                        "active",
                        PropertyOperator::Eq,
                        QueryValue::Scalar(PropertyValue::Bool(true)),
                    )],
                }),
            ],
            returns: Some(vec!["a".to_string(), "rel".to_string(), "b".to_string()]),
            limit: Some(1),
        })
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("a"), Some(&alice));
    assert_eq!(rows[0].get("rel"), Some(&alice_knows_bob));
    assert_eq!(rows[0].get("b"), Some(&bob));
}

#[test]
fn structured_query_supports_directions_and_repeated_aliases() {
    let mut graph = GraphCore::new();
    let alice = graph
        .add_node(
            Some("alice".to_string()),
            vec!["Person".to_string()],
            map([]),
        )
        .unwrap();
    let bob = graph
        .add_node(Some("bob".to_string()), vec!["Person".to_string()], map([]))
        .unwrap();
    let carol = graph
        .add_node(
            Some("carol".to_string()),
            vec!["Person".to_string()],
            map([]),
        )
        .unwrap();
    let ab = graph
        .add_edge(alice, bob, "KNOWS".to_string(), map([]))
        .unwrap();
    let bc = graph
        .add_edge(bob, carol, "KNOWS".to_string(), map([]))
        .unwrap();
    let ca = graph
        .add_edge(carol, alice, "KNOWS".to_string(), map([]))
        .unwrap();

    let incoming = graph
        .query(&QuerySpec {
            elements: vec![
                node_with_id("target", alice),
                edge("rel", Some("KNOWS"), QueryDirection::In, vec![]),
                node("source"),
            ],
            returns: Some(vec!["source".to_string(), "rel".to_string()]),
            limit: None,
        })
        .unwrap();
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].get("source"), Some(&carol));
    assert_eq!(incoming[0].get("rel"), Some(&ca));

    let both = graph
        .query(&QuerySpec {
            elements: vec![
                node_with_id("center", bob),
                edge("rel", Some("KNOWS"), QueryDirection::Both, vec![]),
                node("other"),
            ],
            returns: Some(vec!["other".to_string(), "rel".to_string()]),
            limit: None,
        })
        .unwrap();
    assert_eq!(both.len(), 2);
    assert_eq!(both[0].get("other"), Some(&carol));
    assert_eq!(both[0].get("rel"), Some(&bc));
    assert_eq!(both[1].get("other"), Some(&alice));
    assert_eq!(both[1].get("rel"), Some(&ab));

    let cycle = graph
        .query(&QuerySpec {
            elements: vec![
                node_with_id("a", alice),
                edge("ab", Some("KNOWS"), QueryDirection::Out, vec![]),
                node("b"),
                edge("bc", Some("KNOWS"), QueryDirection::Out, vec![]),
                node("c"),
                edge("ca", Some("KNOWS"), QueryDirection::Out, vec![]),
                node("a"),
            ],
            returns: Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
            limit: None,
        })
        .unwrap();
    assert_eq!(cycle.len(), 1);
    assert_eq!(cycle[0].get("a"), Some(&alice));
    assert_eq!(cycle[0].get("b"), Some(&bob));
    assert_eq!(cycle[0].get("c"), Some(&carol));

    let repeated_edge = graph
        .query(&QuerySpec {
            elements: vec![
                node_with_id("a", alice),
                edge("e", Some("KNOWS"), QueryDirection::Out, vec![]),
                node_with_id("b", bob),
                edge("e", Some("KNOWS"), QueryDirection::In, vec![]),
                node_with_id("a", alice),
            ],
            returns: Some(vec!["e".to_string()]),
            limit: None,
        })
        .unwrap();
    assert_eq!(repeated_edge.len(), 1);
    assert_eq!(repeated_edge[0].get("e"), Some(&ab));
}

#[test]
fn structured_query_rejects_invalid_specs() {
    let mut graph = GraphCore::new();
    let node_id = graph.add_node(None, Vec::new(), map([])).unwrap();

    let unknown_return = graph.query(&QuerySpec {
        elements: vec![node_with_id("n", node_id)],
        returns: Some(vec!["missing".to_string()]),
        limit: None,
    });
    assert!(unknown_return
        .unwrap_err()
        .contains("return alias \"missing\" is not declared"));

    let invalid_pattern = graph.query(&QuerySpec {
        elements: vec![node("a"), node("b")],
        returns: None,
        limit: None,
    });
    assert!(invalid_pattern.unwrap_err().contains("alternate"));

    let alias_kind_conflict = graph.query(&QuerySpec {
        elements: vec![
            node("same"),
            edge("same", Some("LINK"), QueryDirection::Out, vec![]),
            node("other"),
        ],
        returns: None,
        limit: None,
    });
    assert!(alias_kind_conflict
        .unwrap_err()
        .contains("cannot refer to both nodes and edges"));
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

#[test]
fn sqlite_rebuilds_when_segment_manifest_is_bad() {
    let path = compacted_test_graph("tonggraph-bad-manifest");
    fs::write(segment_manifest_path(&path), "bad manifest").unwrap();

    let reopened = GraphCore::open(path.to_str().unwrap()).unwrap();
    assert_eq!(reopened.neighbors(0, "out", Some("LINK")).unwrap(), vec![1]);
    assert!(fs::read_to_string(segment_manifest_path(&path))
        .unwrap()
        .contains("version=tonggraph-segment-v1"));
    cleanup_db(&path);
}

#[test]
fn sqlite_rebuilds_when_segment_file_is_missing_or_corrupt() {
    let missing_path = compacted_test_graph("tonggraph-missing-segment");
    fs::remove_file(segment_file_path(&missing_path)).unwrap();
    let reopened = GraphCore::open(missing_path.to_str().unwrap()).unwrap();
    assert_eq!(reopened.neighbors(0, "out", Some("LINK")).unwrap(), vec![1]);
    assert!(segment_file_path(&missing_path).exists());
    cleanup_db(&missing_path);

    let corrupt_path = compacted_test_graph("tonggraph-corrupt-segment");
    fs::write(segment_file_path(&corrupt_path), b"not a segment").unwrap();
    let reopened = GraphCore::open(corrupt_path.to_str().unwrap()).unwrap();
    assert_eq!(reopened.neighbors(0, "out", Some("LINK")).unwrap(), vec![1]);
    cleanup_db(&corrupt_path);
}

#[test]
fn sqlite_stale_handle_requires_refresh_before_write() {
    let path = temp_db_path("tonggraph-stale-handle");
    let mut first = GraphCore::open(path.to_str().unwrap()).unwrap();
    first
        .add_node(Some("a".to_string()), Vec::new(), map([]))
        .unwrap();

    let mut second = GraphCore::open(path.to_str().unwrap()).unwrap();
    second
        .add_node(Some("b".to_string()), Vec::new(), map([]))
        .unwrap();

    let stale = first
        .add_node(Some("c".to_string()), Vec::new(), map([]))
        .unwrap_err();
    assert!(stale.contains("call refresh() before writing"));
    assert_eq!(first.get_node_id("b"), None);

    first.refresh().unwrap();
    assert_eq!(first.get_node_id("b"), Some(1));
    assert_eq!(
        first
            .add_node(Some("c".to_string()), Vec::new(), map([]))
            .unwrap(),
        2
    );
    cleanup_db(&path);
}

#[test]
fn bulk_add_nodes_edges_and_ordered_scans_work_in_core() {
    let mut graph = GraphCore::new();
    let nodes = graph
        .add_nodes(vec![
            NewNodeRecord {
                external_id: Some("a".to_string()),
                labels: vec!["Entity".to_string()],
                properties: map([("rank", "1")]),
            },
            NewNodeRecord {
                external_id: Some("b".to_string()),
                labels: vec!["Entity".to_string()],
                properties: map([("rank", "2")]),
            },
        ])
        .unwrap();
    assert_eq!(nodes, vec![0, 1]);
    let edges = graph
        .add_edges(vec![NewEdgeRecord {
            source: nodes[0],
            target: nodes[1],
            edge_type: "LINK".to_string(),
            properties: map([("probability", "0.5")]),
        }])
        .unwrap();
    assert_eq!(edges, vec![0]);
    assert_eq!(graph.node_ids(), vec![0, 1]);
    assert_eq!(graph.edge_ids(), vec![0]);
    assert_eq!(
        graph
            .nodes()
            .into_iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(
        graph
            .edges()
            .into_iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>(),
        vec![0]
    );
}

#[test]
fn belief_propagation_conditions_binary_chain_exactly() {
    let mut graph = GraphCore::new();
    let parent = graph
        .add_variable(
            None,
            "binary".to_string(),
            None,
            map([("p", "0.6")]),
            map([]),
        )
        .unwrap();
    let child = graph
        .add_variable(None, "binary".to_string(), None, map([]), map([]))
        .unwrap();
    graph
        .add_cpd(child, vec![parent], vec![0.9, 0.1, 0.2, 0.8])
        .unwrap();

    let result = graph
        .belief_propagation(
            Some(&[child]),
            &HashMap::from([(parent, "true".to_string())]),
            2,
            100,
            1e-12,
            0.0,
            false,
        )
        .unwrap();
    let child_belief = result.beliefs.get(&child).unwrap();
    assert_close(*child_belief.get("false").unwrap(), 0.2);
    assert_close(*child_belief.get("true").unwrap(), 0.8);
    assert_eq!(result.schedule, "residual_async");
    assert!(result.converged);
}

#[test]
fn categorical_cpd_uses_child_fastest_order() {
    let mut graph = GraphCore::new();
    let switch = graph
        .add_variable(None, "binary".to_string(), None, map([]), map([]))
        .unwrap();
    let weather = graph
        .add_variable(
            None,
            "categorical".to_string(),
            Some(vec![
                "sun".to_string(),
                "rain".to_string(),
                "snow".to_string(),
            ]),
            map([]),
            map([]),
        )
        .unwrap();
    graph
        .add_cpd(
            weather,
            vec![switch],
            vec![
                0.7, 0.2, 0.1, // switch=false
                0.1, 0.3, 0.6, // switch=true
            ],
        )
        .unwrap();

    let result = graph
        .belief_propagation(
            Some(&[weather]),
            &HashMap::from([(switch, "true".to_string())]),
            2,
            100,
            1e-12,
            0.0,
            false,
        )
        .unwrap();
    let weather_belief = result.beliefs.get(&weather).unwrap();
    assert_close(*weather_belief.get("sun").unwrap(), 0.1);
    assert_close(*weather_belief.get("rain").unwrap(), 0.3);
    assert_close(*weather_belief.get("snow").unwrap(), 0.6);
}

#[test]
fn loopy_belief_propagation_returns_normalized_metadata() {
    let mut graph = GraphCore::new();
    let a = graph
        .add_variable(
            None,
            "binary".to_string(),
            None,
            map([("p", "0.55")]),
            map([]),
        )
        .unwrap();
    let b = graph
        .add_variable(None, "binary".to_string(), None, map([]), map([]))
        .unwrap();
    let c = graph
        .add_variable(None, "binary".to_string(), None, map([]), map([]))
        .unwrap();
    graph
        .add_factor_table(vec![a, b], vec![2.0, 1.0, 1.0, 2.0])
        .unwrap();
    graph
        .add_factor_table(vec![b, c], vec![2.0, 1.0, 1.0, 2.0])
        .unwrap();
    graph
        .add_factor_table(vec![c, a], vec![2.0, 1.0, 1.0, 2.0])
        .unwrap();

    let result = graph
        .belief_propagation(None, &HashMap::new(), 2, 200, 1e-9, 0.2, false)
        .unwrap();
    assert_eq!(result.beliefs.len(), 3);
    assert_eq!(result.active.factors.len(), 3);
    assert_eq!(result.schedule, "residual_async");
    assert!(result.max_residual.is_finite());
    assert!(result.messages_updated <= result.iterations);
    for belief in result.beliefs.values() {
        assert_close(belief.values().sum::<f64>(), 1.0);
    }
}

#[test]
fn active_subgraph_closes_over_factors_and_caps_nodes() {
    let mut graph = GraphCore::new();
    let a = graph.add_node(None, Vec::new(), map([])).unwrap();
    let b = graph.add_node(None, Vec::new(), map([])).unwrap();
    let c = graph.add_node(None, Vec::new(), map([])).unwrap();
    graph
        .add_edge(a, b, "P".to_string(), map([("probability", "0.5")]))
        .unwrap();
    graph
        .add_edge(b, c, "P".to_string(), map([("probability", "0.5")]))
        .unwrap();
    let va = graph
        .add_variable(Some(a), "binary".to_string(), None, map([]), map([]))
        .unwrap();
    let vb = graph
        .add_variable(Some(b), "binary".to_string(), None, map([]), map([]))
        .unwrap();
    let vc = graph
        .add_variable(Some(c), "binary".to_string(), None, map([]), map([]))
        .unwrap();
    let factor = graph
        .add_factor_table(vec![vb, vc], vec![1.0, 0.5, 0.5, 1.0])
        .unwrap();

    let active = graph
        .compile_active_subgraph(&[va], &HashMap::new(), 1, 2, 10)
        .unwrap();
    assert_eq!(active.graph_nodes, vec![a, b]);
    assert_eq!(active.factors, vec![factor]);
    assert!(active.variables.contains(&vc));
    assert_eq!(active.boundary_variables, vec![vc]);
    assert!(active.truncated);

    let propagated = graph
        .local_propagate(
            &HashMap::from([(a, 1.0)]),
            1,
            None,
            Some("P"),
            "probability",
            1.0,
        )
        .unwrap();
    assert_eq!(propagated.get(&a), Some(&1.0));
    assert_eq!(propagated.get(&b), Some(&0.5));
    assert!(!propagated.contains_key(&c));

    assert!(graph
        .local_propagate(
            &HashMap::from([(a, -1.0)]),
            1,
            None,
            Some("P"),
            "probability",
            1.0,
        )
        .is_err());
    assert!(graph
        .local_propagate(
            &HashMap::from([(a, 1.0)]),
            1,
            None,
            Some("P"),
            "probability",
            1.5,
        )
        .is_err());
}

#[test]
fn posterior_persists_after_belief_propagation_and_reopen() {
    let path = temp_db_path("tonggraph-bp-posterior");
    let variable;
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        variable = graph
            .add_variable(
                None,
                "binary".to_string(),
                None,
                map([("p", "0.25")]),
                map([]),
            )
            .unwrap();
        let result = graph
            .belief_propagation(
                Some(&[variable]),
                &HashMap::from([(variable, "true".to_string())]),
                2,
                20,
                1e-9,
                0.0,
                true,
            )
            .unwrap();
        assert_eq!(result.trace_id, Some(0));
    }

    let reopened = GraphCore::open(path.to_str().unwrap()).unwrap();
    let posterior = reopened.posterior(variable).unwrap();
    assert_close(*posterior.get("false").unwrap(), 0.0);
    assert_close(*posterior.get("true").unwrap(), 1.0);
    assert_eq!(reopened.trace_count(), 1);
    cleanup_db(&path);
}

#[test]
fn invalid_domains_and_potentials_are_rejected() {
    let mut graph = GraphCore::new();
    assert!(graph
        .add_variable(None, "categorical".to_string(), None, map([]), map([]),)
        .is_err());
    let variable = graph
        .add_variable(None, "binary".to_string(), None, map([]), map([]))
        .unwrap();
    assert!(graph
        .add_factor_table(vec![variable], vec![0.0, 0.0])
        .is_err());
    assert!(graph
        .add_factor_table(vec![variable], vec![1.0, -1.0])
        .is_err());
    assert!(graph
        .add_factor_table(vec![variable, variable], vec![1.0, 0.0, 0.0, 1.0])
        .is_err());

    let parent = graph
        .add_variable(None, "binary".to_string(), None, map([]), map([]))
        .unwrap();
    let child = graph
        .add_variable(None, "binary".to_string(), None, map([]), map([]))
        .unwrap();
    assert!(graph
        .add_cpd(child, vec![parent], vec![90.0, 10.0, 2.0, 8.0])
        .is_err());
}

#[test]
fn graph_crud_updates_indexes_and_enforces_delete_rules() {
    let mut graph = GraphCore::new();
    let alice = graph
        .add_node(
            Some("alice".to_string()),
            vec!["Person".to_string()],
            typed_map([("name", s("Alice")), ("rank", PropertyValue::Int(1))]),
        )
        .unwrap();
    let bob = graph
        .add_node(Some("bob".to_string()), vec!["Person".to_string()], map([]))
        .unwrap();
    let edge = graph
        .add_edge(alice, bob, "KNOWS".to_string(), map([("since", "2024")]))
        .unwrap();

    let updated = graph
        .update_node(
            alice,
            Some("alice-2".to_string()),
            vec!["Researcher".to_string()],
            vec!["Person".to_string()],
            typed_map([("rank", PropertyValue::Int(2))]),
            vec!["name".to_string()],
        )
        .unwrap();
    assert_eq!(updated.external_id, "alice-2");
    assert_eq!(updated.labels, vec!["Researcher"]);
    assert!(!updated.properties.contains_key("name"));
    assert_eq!(graph.get_node_id("alice"), None);
    assert_eq!(graph.get_node_id("alice-2"), Some(alice));
    assert_eq!(graph.nodes_with_label("Person"), vec![bob]);
    assert_eq!(graph.nodes_with_label("Researcher"), vec![alice]);
    assert_eq!(
        graph.nodes_with_property("rank", Some(&PropertyValue::Int(2))),
        vec![alice]
    );
    assert!(graph
        .update_node(
            alice,
            Some("bob".to_string()),
            Vec::new(),
            Vec::new(),
            map([]),
            Vec::new(),
        )
        .unwrap_err()
        .contains("already exists"));
    assert!(graph.delete_node(alice, false).is_err());

    let updated_edge = graph
        .update_edge(
            edge,
            typed_map([("weight", PropertyValue::Float(0.5))]),
            vec!["since".to_string()],
        )
        .unwrap();
    assert!(!updated_edge.properties.contains_key("since"));
    assert_eq!(
        graph.edges_with_property("weight", Some(&PropertyValue::Float(0.5))),
        vec![edge]
    );

    graph.delete_edge(edge).unwrap();
    assert_eq!(
        graph.neighbors(alice, "out", None).unwrap(),
        Vec::<u64>::new()
    );
    graph.delete_node(alice, false).unwrap();
    assert!(graph.get_node(alice).is_none());
}

#[test]
fn node_delete_rejects_probabilistic_owner_and_detach_removes_edges() {
    let mut graph = GraphCore::new();
    let owner = graph.add_node(None, Vec::new(), map([])).unwrap();
    let other = graph.add_node(None, Vec::new(), map([])).unwrap();
    graph
        .add_edge(owner, other, "LINK".to_string(), map([]))
        .unwrap();
    graph
        .add_variable(Some(owner), "binary".to_string(), None, map([]), map([]))
        .unwrap();
    assert!(graph
        .delete_node(owner, true)
        .unwrap_err()
        .contains("probabilistic variables"));

    let disposable = graph.add_node(None, Vec::new(), map([])).unwrap();
    graph
        .add_edge(disposable, other, "LINK".to_string(), map([]))
        .unwrap();
    graph.delete_node(disposable, true).unwrap();
    assert!(graph.get_node(disposable).is_none());
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn graph_transaction_commits_consumed_ids_even_when_records_cancel_out() {
    let mut graph = GraphCore::new();
    let (mut staged, base_version) = graph.transaction_snapshot();
    let transient = staged.add_node(None, Vec::new(), map([])).unwrap();
    staged.delete_node(transient, false).unwrap();
    graph
        .commit_transaction_snapshot(&staged, base_version)
        .unwrap();
    let next = graph.add_node(None, Vec::new(), map([])).unwrap();
    assert_eq!(next, transient + 1);
}

#[test]
fn graph_transaction_rejects_parent_mutation_conflicts() {
    let mut graph = GraphCore::new();
    let node = graph
        .add_node(Some("node".to_string()), Vec::new(), map([("name", "old")]))
        .unwrap();
    let (mut staged, base_version) = graph.transaction_snapshot();
    staged
        .update_node_in_place(
            node,
            None,
            Vec::new(),
            Vec::new(),
            map([("name", "staged")]),
            Vec::new(),
        )
        .unwrap();
    graph.add_node(None, Vec::new(), map([])).unwrap();
    let error = graph
        .commit_transaction_snapshot(&staged, base_version)
        .unwrap_err();
    assert!(error.contains("graph changed since transaction started"));
    assert_eq!(
        graph.get_node(node).unwrap().properties["name"].encoded_value(),
        "old"
    );
}

#[test]
fn sqlite_crud_persists_updates_deletes_and_segments() {
    let path = temp_db_path("tonggraph-crud");
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        let a = graph
            .add_node(
                Some("a".to_string()),
                vec!["Old".to_string()],
                map([("name", "A")]),
            )
            .unwrap();
        let b = graph
            .add_node(Some("b".to_string()), Vec::new(), map([]))
            .unwrap();
        let edge = graph
            .add_edge(a, b, "LINK".to_string(), map([("old", "yes")]))
            .unwrap();
        graph
            .update_node(
                a,
                Some("a2".to_string()),
                vec!["New".to_string()],
                vec!["Old".to_string()],
                map([("name", "Alice")]),
                Vec::new(),
            )
            .unwrap();
        graph
            .update_edge(edge, map([("weight", "2")]), vec!["old".to_string()])
            .unwrap();
        graph.delete_node(b, true).unwrap();
    }

    let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
    assert_eq!(graph.node_count(), 1);
    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.get_node_id("a2"), Some(0));
    assert_eq!(graph.nodes_with_label("New"), vec![0]);
    assert_eq!(graph.nodes_with_label("Old"), Vec::<u64>::new());
    assert_eq!(graph.neighbors(0, "out", None).unwrap(), Vec::<u64>::new());
    assert_eq!(
        graph.get_node(0).unwrap().properties["name"].encoded_value(),
        "Alice"
    );
    let next = graph.add_node(None, Vec::new(), map([])).unwrap();
    assert_eq!(next, 2);
    cleanup_db(&path);
}

#[test]
fn in_memory_fulltext_search_supports_modes_filters_edges_and_snapshots() {
    let mut graph = GraphCore::new();
    let guide = graph
        .add_node(
            Some("guide".to_string()),
            vec!["Document".to_string()],
            typed_map([
                ("title", s("Graph Database Guide")),
                ("content", s("A local embedded graph engine")),
                ("published", PropertyValue::Bool(true)),
            ]),
        )
        .unwrap();
    let notes = graph
        .add_node(
            Some("notes".to_string()),
            vec!["Document".to_string()],
            typed_map([
                ("title", s("Database Notes")),
                ("content", s("Relational storage")),
                ("published", PropertyValue::Bool(false)),
            ]),
        )
        .unwrap();
    let other = graph
        .add_node(Some("other".to_string()), Vec::new(), map([]))
        .unwrap();
    let edge = graph
        .add_edge(
            guide,
            other,
            "CITES".to_string(),
            map([("note", "Graph research collaboration")]),
        )
        .unwrap();

    graph
        .create_fulltext_index(
            "documents".to_string(),
            "node".to_string(),
            vec!["title".to_string(), "content".to_string()],
            "unicode61".to_string(),
        )
        .unwrap();
    graph
        .create_fulltext_index(
            "relations".to_string(),
            "edge".to_string(),
            vec!["note".to_string()],
            "unicode61".to_string(),
        )
        .unwrap();

    let all = graph
        .search_text("documents", "graph database", "all", &fulltext_options())
        .unwrap();
    assert_eq!(
        all.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![guide]
    );
    assert_eq!(all[0].matched_fields, vec!["title", "content"]);

    let any = graph
        .search_text("documents", "graph relational", "any", &fulltext_options())
        .unwrap();
    assert_eq!(
        any.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![guide, notes]
    );
    assert_eq!(
        graph
            .search_text(
                "documents",
                "graph database guide",
                "phrase",
                &fulltext_options(),
            )
            .unwrap()[0]
            .id,
        guide
    );
    assert_eq!(
        graph
            .search_text("documents", "grap", "prefix", &fulltext_options())
            .unwrap()[0]
            .id,
        guide
    );

    let filtered = graph
        .search_text(
            "documents",
            "database",
            "all",
            &FullTextSearchOptions {
                labels: vec!["Document".to_string()],
                properties: typed_map([("published", PropertyValue::Bool(true))]),
                ..fulltext_options()
            },
        )
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, guide);

    let edge_results = graph
        .search_text(
            "relations",
            "research",
            "all",
            &FullTextSearchOptions {
                edge_type: Some("CITES".to_string()),
                ..fulltext_options()
            },
        )
        .unwrap();
    assert_eq!(edge_results[0].id, edge);
    assert_eq!(edge_results[0].kind, "edge");

    let snapshot = graph.snapshot();
    graph
        .update_node(
            guide,
            None,
            Vec::new(),
            Vec::new(),
            map([
                ("title", "Completely Different"),
                ("content", "No matching terms"),
            ]),
            Vec::new(),
        )
        .unwrap();
    assert_eq!(
        snapshot
            .search_text("documents", "graph", "all", &fulltext_options())
            .unwrap()[0]
            .id,
        guide
    );
    assert!(graph
        .search_text("documents", "graph", "all", &fulltext_options())
        .unwrap()
        .is_empty());
}

#[test]
fn trigram_fulltext_search_supports_chinese_substrings() {
    let mut graph = GraphCore::new();
    let document = graph
        .add_node(None, Vec::new(), map([("content", "本地图数据库全文检索")]))
        .unwrap();
    graph
        .create_fulltext_index(
            "chinese".to_string(),
            "node".to_string(),
            vec!["content".to_string()],
            "trigram".to_string(),
        )
        .unwrap();
    assert_eq!(
        graph
            .search_text("chinese", "图数据", "all", &fulltext_options())
            .unwrap()[0]
            .id,
        document
    );
    assert!(graph
        .search_text("chinese", "图数", "all", &fulltext_options())
        .is_err());
    assert!(graph
        .search_text("chinese", "图数据", "prefix", &fulltext_options())
        .is_err());
}

#[test]
fn sqlite_fulltext_indexes_persist_and_follow_crud() {
    let path = temp_db_path("tonggraph-fulltext");
    let node;
    let edge;
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        node = graph
            .add_node(
                Some("doc".to_string()),
                vec!["Document".to_string()],
                map([("title", "Embedded Graph Search")]),
            )
            .unwrap();
        let target = graph.add_node(None, Vec::new(), map([])).unwrap();
        edge = graph
            .add_edge(
                node,
                target,
                "REFERENCES".to_string(),
                map([("note", "initial citation")]),
            )
            .unwrap();
        graph
            .create_fulltext_index(
                "docs".to_string(),
                "node".to_string(),
                vec!["title".to_string()],
                "unicode61".to_string(),
            )
            .unwrap();
        graph
            .create_fulltext_index(
                "edges".to_string(),
                "edge".to_string(),
                vec!["note".to_string()],
                "unicode61".to_string(),
            )
            .unwrap();
    }

    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        assert_eq!(graph.fulltext_indexes().len(), 2);
        assert_eq!(
            graph
                .search_text("docs", "embedded", "all", &fulltext_options())
                .unwrap()[0]
                .id,
            node
        );
        graph
            .update_node(
                node,
                None,
                Vec::new(),
                Vec::new(),
                map([("title", "Updated Retrieval")]),
                Vec::new(),
            )
            .unwrap();
        assert!(graph
            .search_text("docs", "embedded", "all", &fulltext_options())
            .unwrap()
            .is_empty());
        assert_eq!(
            graph
                .search_text("docs", "updated", "all", &fulltext_options())
                .unwrap()[0]
                .id,
            node
        );
        assert_eq!(
            graph
                .search_text("docs", "Updated Retrieval", "phrase", &fulltext_options())
                .unwrap()[0]
                .id,
            node
        );
        assert_eq!(
            graph
                .search_text("docs", "retr", "prefix", &fulltext_options())
                .unwrap()[0]
                .id,
            node
        );
        graph.delete_edge(edge).unwrap();
        assert!(graph
            .search_text("edges", "citation", "all", &fulltext_options())
            .unwrap()
            .is_empty());
        graph.rebuild_fulltext_index(None).unwrap();
        graph.drop_fulltext_index("edges").unwrap();
        assert_eq!(
            graph
                .fulltext_indexes()
                .into_iter()
                .map(|definition| definition.name)
                .collect::<Vec<_>>(),
            vec!["docs"]
        );
    }

    let graph = GraphCore::open(path.to_str().unwrap()).unwrap();
    assert_eq!(
        graph
            .search_text("docs", "retrieval", "all", &fulltext_options())
            .unwrap()[0]
            .id,
        node
    );
    assert!(graph
        .search_text("edges", "citation", "all", &fulltext_options())
        .is_err());
    cleanup_db(&path);
}

#[test]
fn fulltext_validation_rejects_invalid_definitions_and_filters() {
    let mut graph = GraphCore::new();
    graph.add_node(None, Vec::new(), map([])).unwrap();
    assert!(graph
        .create_fulltext_index(
            "bad".to_string(),
            "mixed".to_string(),
            vec!["text".to_string()],
            "unicode61".to_string(),
        )
        .is_err());
    assert!(graph
        .create_fulltext_index(
            "bad".to_string(),
            "node".to_string(),
            vec![],
            "unicode61".to_string(),
        )
        .is_err());
    graph
        .create_fulltext_index(
            "nodes".to_string(),
            "node".to_string(),
            vec!["text".to_string()],
            "unicode61".to_string(),
        )
        .unwrap();
    assert!(graph
        .search_text(
            "nodes",
            "text",
            "all",
            &FullTextSearchOptions {
                edge_type: Some("LINK".to_string()),
                ..fulltext_options()
            },
        )
        .is_err());
    assert!(graph
        .search_text("nodes", "text", "bad", &fulltext_options(),)
        .is_err());
    assert!(graph
        .search_text(
            "nodes",
            "text",
            "all",
            &FullTextSearchOptions {
                limit: 0,
                ..fulltext_options()
            },
        )
        .is_err());
}

#[test]
fn in_memory_vector_search_supports_metrics_filters_snapshots_and_subgraphs() {
    let mut graph = GraphCore::new();
    let a = graph
        .add_node(
            Some("a".to_string()),
            vec!["Document".to_string()],
            typed_map([("published", PropertyValue::Bool(true))]),
        )
        .unwrap();
    let b = graph
        .add_node(
            Some("b".to_string()),
            vec!["Document".to_string()],
            typed_map([("published", PropertyValue::Bool(false))]),
        )
        .unwrap();
    let c = graph
        .add_node(Some("c".to_string()), Vec::new(), map([]))
        .unwrap();
    let edge = graph.add_edge(a, c, "CITES".to_string(), map([])).unwrap();

    graph
        .create_vector_index(
            "documents".to_string(),
            "node".to_string(),
            3,
            "cosine".to_string(),
            Some("embed".to_string()),
            Some("1".to_string()),
        )
        .unwrap();
    graph
        .upsert_vectors(
            "documents",
            vec![(a, vec![1.0, 0.0, 0.0]), (b, vec![0.5, 0.5, 0.0])],
        )
        .unwrap();
    graph
        .create_vector_index(
            "relations".to_string(),
            "edge".to_string(),
            2,
            "euclidean".to_string(),
            None,
            None,
        )
        .unwrap();
    graph
        .upsert_vector("relations", edge, vec![1.0, 1.0])
        .unwrap();

    let results = graph
        .search_vector(
            "documents",
            &[1.0, 0.0, 0.0],
            &VectorSearchOptions {
                labels: vec!["Document".to_string()],
                edge_type: None,
                properties: PropertyMap::new(),
                min_score: None,
                limit: 20,
                offset: 0,
            },
        )
        .unwrap();
    assert_eq!(
        results.iter().map(|result| result.id).collect::<Vec<_>>(),
        vec![a, b]
    );
    assert_close(results[0].score, 1.0);
    assert!(results[1].score < results[0].score);

    let filtered = graph
        .search_vector(
            "documents",
            &[1.0, 0.0, 0.0],
            &VectorSearchOptions {
                labels: vec!["Document".to_string()],
                edge_type: None,
                properties: typed_map([("published", PropertyValue::Bool(true))]),
                min_score: Some(0.9),
                limit: 1,
                offset: 0,
            },
        )
        .unwrap();
    assert_eq!(filtered[0].id, a);
    let edge_result = graph
        .search_vector(
            "relations",
            &[1.0, 1.0],
            &VectorSearchOptions {
                labels: Vec::new(),
                edge_type: Some("CITES".to_string()),
                properties: PropertyMap::new(),
                min_score: None,
                limit: 20,
                offset: 0,
            },
        )
        .unwrap();
    assert_eq!(edge_result[0].id, edge);
    assert_close(edge_result[0].score, 1.0);

    let snapshot = graph.snapshot();
    let subgraph = graph.subgraph(&[a, c], None).unwrap();
    graph
        .upsert_vector("documents", a, vec![0.0, 1.0, 0.0])
        .unwrap();
    assert_close(
        snapshot
            .search_vector("documents", &[1.0, 0.0, 0.0], &vector_options())
            .unwrap()[0]
            .score,
        1.0,
    );
    assert_eq!(
        subgraph.get_vector("documents", a).unwrap().unwrap(),
        vec![1.0, 0.0, 0.0]
    );
    assert!(subgraph.get_vector("documents", b).is_err());
}

#[test]
fn vector_search_validates_batches_and_supports_dot_ties() {
    let mut graph = GraphCore::new();
    let a = graph
        .add_node(Some("a".to_string()), Vec::new(), map([]))
        .unwrap();
    let b = graph
        .add_node(Some("b".to_string()), Vec::new(), map([]))
        .unwrap();
    graph
        .create_vector_index(
            "dot".to_string(),
            "node".to_string(),
            2,
            "dot".to_string(),
            None,
            None,
        )
        .unwrap();
    let error = graph
        .upsert_vectors("dot", vec![(a, vec![1.0, 0.0]), (b, vec![f64::NAN, 0.0])])
        .unwrap_err();
    assert!(error.contains("finite"));
    assert!(graph.get_vector("dot", a).unwrap().is_none());
    graph
        .upsert_vectors("dot", vec![(b, vec![1.0, 0.0]), (a, vec![1.0, 0.0])])
        .unwrap();
    let results = graph
        .search_vector("dot", &[2.0, 0.0], &vector_options())
        .unwrap();
    assert_eq!(
        results.iter().map(|result| result.id).collect::<Vec<_>>(),
        vec![a, b]
    );
    assert_close(results[0].score, 2.0);
    let paged = graph
        .search_vector(
            "dot",
            &[2.0, 0.0],
            &VectorSearchOptions {
                offset: 1,
                limit: 1,
                ..vector_options()
            },
        )
        .unwrap();
    assert_eq!(paged[0].id, b);
    assert!(graph.upsert_vector("dot", a, vec![1.0]).is_err());

    graph
        .create_vector_index(
            "cosine".to_string(),
            "node".to_string(),
            2,
            "cosine".to_string(),
            None,
            None,
        )
        .unwrap();
    assert!(graph
        .upsert_vector("cosine", a, vec![0.0, 0.0])
        .unwrap_err()
        .contains("zero"));
    assert!(graph
        .create_vector_index(
            "bad-model".to_string(),
            "node".to_string(),
            2,
            "cosine".to_string(),
            None,
            Some("1".to_string()),
        )
        .is_err());
    assert!(graph
        .create_vector_index(
            "bad-target".to_string(),
            "mixed".to_string(),
            2,
            "cosine".to_string(),
            None,
            None,
        )
        .is_err());
    assert!(graph
        .create_vector_index(
            "bad-dimensions".to_string(),
            "node".to_string(),
            0,
            "cosine".to_string(),
            None,
            None,
        )
        .is_err());
    assert!(graph
        .create_vector_index(
            "bad-metric".to_string(),
            "node".to_string(),
            2,
            "manhattan".to_string(),
            None,
            None,
        )
        .is_err());
    assert!(graph
        .search_vector(
            "dot",
            &[1.0, 0.0],
            &VectorSearchOptions {
                limit: 0,
                ..vector_options()
            },
        )
        .is_err());
    assert!(graph
        .search_vector(
            "dot",
            &[1.0, 0.0],
            &VectorSearchOptions {
                min_score: Some(f64::NAN),
                ..vector_options()
            },
        )
        .is_err());
    assert!(graph
        .search_vector(
            "dot",
            &[1.0, 0.0],
            &VectorSearchOptions {
                edge_type: Some("LINK".to_string()),
                ..vector_options()
            },
        )
        .is_err());
}

#[test]
fn sqlite_vectors_persist_and_entity_deletes_cascade_transactionally() {
    let path = temp_db_path("tonggraph-vectors");
    let node;
    let target;
    let edge;
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        node = graph
            .add_node(Some("node".to_string()), Vec::new(), map([]))
            .unwrap();
        target = graph
            .add_node(Some("target".to_string()), Vec::new(), map([]))
            .unwrap();
        edge = graph
            .add_edge(node, target, "LINK".to_string(), map([]))
            .unwrap();
        graph
            .create_vector_index(
                "nodes".to_string(),
                "node".to_string(),
                2,
                "cosine".to_string(),
                None,
                None,
            )
            .unwrap();
        graph
            .create_vector_index(
                "edges".to_string(),
                "edge".to_string(),
                2,
                "euclidean".to_string(),
                None,
                None,
            )
            .unwrap();
        graph.upsert_vector("nodes", node, vec![1.0, 0.0]).unwrap();
        graph.upsert_vector("edges", edge, vec![0.0, 1.0]).unwrap();
    }
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        assert_eq!(
            graph.get_vector("nodes", node).unwrap().unwrap(),
            vec![1.0, 0.0]
        );
        assert_eq!(
            graph.get_vector("edges", edge).unwrap().unwrap(),
            vec![0.0, 1.0]
        );
        let (mut staged, version) = graph.transaction_snapshot();
        staged.delete_node_in_place(target, true).unwrap();
        graph.commit_transaction_snapshot(&staged, version).unwrap();
        assert!(graph
            .search_vector("edges", &[0.0, 1.0], &vector_options())
            .unwrap()
            .is_empty());
    }
    let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
    assert!(graph
        .search_vector("edges", &[0.0, 1.0], &vector_options())
        .unwrap()
        .is_empty());
    graph.delete_vector("nodes", node).unwrap();
    graph.delete_vector("nodes", node).unwrap();
    assert!(graph.get_vector("nodes", node).unwrap().is_none());
    graph.drop_vector_index("edges").unwrap();
    assert!(graph.get_vector("edges", edge).is_err());
    cleanup_db(&path);
}

fn vector_options() -> VectorSearchOptions {
    VectorSearchOptions {
        labels: Vec::new(),
        edge_type: None,
        properties: PropertyMap::new(),
        min_score: None,
        limit: 20,
        offset: 0,
    }
}

fn fulltext_options() -> FullTextSearchOptions {
    FullTextSearchOptions {
        labels: Vec::new(),
        edge_type: None,
        properties: PropertyMap::new(),
        limit: 20,
        offset: 0,
    }
}

fn map<const N: usize>(values: [(&str, &str); N]) -> PropertyMap {
    values
        .into_iter()
        .map(|(key, value)| (key.to_string(), PropertyValue::String(value.to_string())))
        .collect()
}

fn typed_map<const N: usize>(values: [(&str, PropertyValue); N]) -> PropertyMap {
    values
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn s(value: &str) -> PropertyValue {
    PropertyValue::String(value.to_string())
}

fn node(alias: &str) -> QueryElement {
    QueryElement::Node(NodePattern {
        alias: alias.to_string(),
        id: None,
        external_id: None,
        labels: Vec::new(),
        properties: Vec::new(),
    })
}

fn node_with_id(alias: &str, id: u64) -> QueryElement {
    QueryElement::Node(NodePattern {
        alias: alias.to_string(),
        id: Some(id),
        external_id: None,
        labels: Vec::new(),
        properties: Vec::new(),
    })
}

fn edge(
    alias: &str,
    edge_type: Option<&str>,
    direction: QueryDirection,
    properties: Vec<PropertyConstraint>,
) -> QueryElement {
    QueryElement::Edge(EdgePattern {
        alias: Some(alias.to_string()),
        id: None,
        edge_type: edge_type.map(str::to_string),
        direction,
        properties,
    })
}

fn filter(key: &str, op: PropertyOperator, value: QueryValue) -> PropertyConstraint {
    PropertyConstraint {
        key: key.to_string(),
        op,
        value,
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {actual} to be close to {expected}"
    );
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

fn segment_file_path(path: &std::path::Path) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}.segments", path.to_string_lossy())).join("segment-v1.bin")
}

fn compacted_test_graph(name: &str) -> std::path::PathBuf {
    let path = temp_db_path(name);
    {
        let mut graph = GraphCore::open(path.to_str().unwrap()).unwrap();
        let a = graph
            .add_node(Some("a".to_string()), Vec::new(), map([]))
            .unwrap();
        let b = graph
            .add_node(Some("b".to_string()), Vec::new(), map([]))
            .unwrap();
        graph.add_edge(a, b, "LINK".to_string(), map([])).unwrap();
        graph.compact_segments().unwrap();
    }
    path
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
