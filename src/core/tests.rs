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

fn cleanup_db(path: &std::path::Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_extension("db-shm"));
    let _ = fs::remove_file(path.with_extension("db-wal"));
    let _ = fs::remove_dir_all(std::path::PathBuf::from(format!(
        "{}.segments",
        path.to_string_lossy()
    )));
}
