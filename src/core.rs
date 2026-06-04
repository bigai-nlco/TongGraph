use crate::models::{EdgeRecord, NodeRecord};
use crate::sqlite::SqliteStore;
use std::collections::{BTreeSet, HashMap, VecDeque};

pub(crate) struct GraphCore {
    nodes: Vec<Option<NodeRecord>>,
    edges: Vec<Option<EdgeRecord>>,
    out_adj: Vec<Vec<u64>>,
    in_adj: Vec<Vec<u64>>,
    node_by_external_id: HashMap<String, u64>,
    label_index: HashMap<String, BTreeSet<u64>>,
    edge_type_index: HashMap<String, BTreeSet<u64>>,
    next_node_id: u64,
    next_edge_id: u64,
    store: Option<SqliteStore>,
}

impl GraphCore {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            out_adj: Vec::new(),
            in_adj: Vec::new(),
            node_by_external_id: HashMap::new(),
            label_index: HashMap::new(),
            edge_type_index: HashMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
            store: None,
        }
    }

    pub(crate) fn open(path: &str) -> Result<Self, String> {
        let store = SqliteStore::open(path)?;
        let mut core = Self::new();
        for node in store.load_nodes()? {
            core.insert_loaded_node(node)?;
        }
        for edge in store.load_edges()? {
            core.insert_loaded_edge(edge)?;
        }
        core.store = Some(store);
        Ok(core)
    }

    pub(crate) fn add_node(
        &mut self,
        external_id: Option<String>,
        labels: Vec<String>,
        properties: HashMap<String, String>,
    ) -> Result<u64, String> {
        for label in &labels {
            validate_non_empty("label", label)?;
        }

        let id = self.next_node_id;
        let external_id = external_id.unwrap_or_else(|| format!("node:{id}"));
        validate_non_empty("external_id", &external_id)?;
        if self.node_by_external_id.contains_key(&external_id) {
            return Err(format!("external_id {external_id:?} already exists"));
        }

        let record = NodeRecord {
            id,
            external_id,
            labels,
            properties,
        };

        if let Some(store) = &self.store {
            store.insert_node(&record)?;
        }
        self.insert_node_record(record)?;
        Ok(id)
    }

    pub(crate) fn add_edge(
        &mut self,
        source: u64,
        target: u64,
        edge_type: String,
        properties: HashMap<String, String>,
    ) -> Result<u64, String> {
        validate_non_empty("edge_type", &edge_type)?;
        self.require_node(source)?;
        self.require_node(target)?;

        let id = self.next_edge_id;
        let record = EdgeRecord {
            id,
            source,
            target,
            edge_type,
            properties,
        };

        if let Some(store) = &self.store {
            store.insert_edge(&record)?;
        }
        self.insert_edge_record(record)?;
        Ok(id)
    }

    pub(crate) fn node_count(&self) -> usize {
        self.nodes.iter().flatten().count()
    }

    pub(crate) fn edge_count(&self) -> usize {
        self.edges.iter().flatten().count()
    }

    pub(crate) fn get_node(&self, node_id: u64) -> Option<NodeRecord> {
        self.nodes.get(node_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn get_edge(&self, edge_id: u64) -> Option<EdgeRecord> {
        self.edges.get(edge_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn get_node_id(&self, external_id: &str) -> Option<u64> {
        self.node_by_external_id.get(external_id).copied()
    }

    pub(crate) fn nodes_with_label(&self, label: &str) -> Vec<u64> {
        self.label_index
            .get(label)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) fn edges_by_type(&self, edge_type: &str) -> Vec<u64> {
        self.edge_type_index
            .get(edge_type)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) fn neighbors(
        &self,
        node_id: u64,
        direction: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<u64>, String> {
        self.require_node(node_id)?;
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let direction = Direction::parse(direction)?;

        if direction.includes_out() {
            self.collect_neighbors(node_id, true, edge_type, &mut seen, &mut result);
        }
        if direction.includes_in() {
            self.collect_neighbors(node_id, false, edge_type, &mut seen, &mut result);
        }

        Ok(result)
    }

    pub(crate) fn k_hop(
        &self,
        start: u64,
        hops: usize,
        direction: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<u64>, String> {
        self.require_node(start)?;
        let direction = Direction::parse(direction)?;
        let mut visited = BTreeSet::new();
        let mut ordered = Vec::new();
        let mut frontier = VecDeque::from([(start, 0usize)]);
        visited.insert(start);

        while let Some((node_id, depth)) = frontier.pop_front() {
            if depth == hops {
                continue;
            }
            for next in self.neighbors_for_direction(node_id, direction, edge_type) {
                if visited.insert(next) {
                    ordered.push(next);
                    frontier.push_back((next, depth + 1));
                }
            }
        }

        Ok(ordered)
    }

    pub(crate) fn propagate(
        &self,
        seeds: &HashMap<u64, f64>,
        steps: usize,
        edge_property: &str,
        damping: f64,
    ) -> Result<HashMap<u64, f64>, String> {
        if !(0.0..=1.0).contains(&damping) {
            return Err("damping must be between 0.0 and 1.0".to_string());
        }
        for (&node_id, &probability) in seeds {
            self.require_node(node_id)?;
            if !probability.is_finite() || probability < 0.0 {
                return Err("seed probabilities must be finite and non-negative".to_string());
            }
        }

        let mut current = seeds.clone();
        let mut accumulated = seeds.clone();

        for _ in 0..steps {
            let mut next: HashMap<u64, f64> = HashMap::new();
            for (&source, &probability) in &current {
                let Some(edge_ids) = self.out_adj.get(source as usize) else {
                    continue;
                };
                for &edge_id in edge_ids {
                    let Some(edge) = self.edge_record(edge_id) else {
                        continue;
                    };
                    let weight = edge_weight(edge, edge_property)?;
                    if weight == 0.0 {
                        continue;
                    }
                    *next.entry(edge.target).or_insert(0.0) += probability * weight * damping;
                }
            }

            if next.is_empty() {
                break;
            }
            for (&node_id, &probability) in &next {
                *accumulated.entry(node_id).or_insert(0.0) += probability;
            }
            current = next;
        }

        Ok(accumulated)
    }

    fn insert_loaded_node(&mut self, record: NodeRecord) -> Result<(), String> {
        self.insert_node_record(record)
    }

    fn insert_loaded_edge(&mut self, record: EdgeRecord) -> Result<(), String> {
        self.require_node(record.source)?;
        self.require_node(record.target)?;
        self.insert_edge_record(record)
    }

    fn insert_node_record(&mut self, record: NodeRecord) -> Result<(), String> {
        if self.node_by_external_id.contains_key(&record.external_id) {
            return Err(format!(
                "external_id {:?} already exists",
                record.external_id
            ));
        }
        let id = record.id;
        self.ensure_node_slot(id);
        if self.nodes[id as usize].is_some() {
            return Err(format!("node id {id} already exists"));
        }

        for label in &record.labels {
            self.label_index
                .entry(label.clone())
                .or_default()
                .insert(id);
        }
        self.node_by_external_id
            .insert(record.external_id.clone(), id);
        self.nodes[id as usize] = Some(record);
        self.next_node_id = self.next_node_id.max(id + 1);
        Ok(())
    }

    fn insert_edge_record(&mut self, record: EdgeRecord) -> Result<(), String> {
        let id = record.id;
        self.ensure_edge_slot(id);
        if self.edges[id as usize].is_some() {
            return Err(format!("edge id {id} already exists"));
        }

        self.ensure_node_slot(record.source);
        self.ensure_node_slot(record.target);
        self.out_adj[record.source as usize].push(id);
        self.in_adj[record.target as usize].push(id);
        self.edge_type_index
            .entry(record.edge_type.clone())
            .or_default()
            .insert(id);
        self.edges[id as usize] = Some(record);
        self.next_edge_id = self.next_edge_id.max(id + 1);
        Ok(())
    }

    fn require_node(&self, node_id: u64) -> Result<(), String> {
        match self.nodes.get(node_id as usize) {
            Some(Some(_)) => Ok(()),
            _ => Err(format!("node {node_id} not found")),
        }
    }

    fn ensure_node_slot(&mut self, node_id: u64) {
        let size = node_id as usize + 1;
        if self.nodes.len() < size {
            self.nodes.resize_with(size, || None);
        }
        if self.out_adj.len() < size {
            self.out_adj.resize_with(size, Vec::new);
        }
        if self.in_adj.len() < size {
            self.in_adj.resize_with(size, Vec::new);
        }
    }

    fn ensure_edge_slot(&mut self, edge_id: u64) {
        let size = edge_id as usize + 1;
        if self.edges.len() < size {
            self.edges.resize_with(size, || None);
        }
    }

    fn edge_record(&self, edge_id: u64) -> Option<&EdgeRecord> {
        self.edges.get(edge_id as usize).and_then(Option::as_ref)
    }

    fn collect_neighbors(
        &self,
        node_id: u64,
        outgoing: bool,
        edge_type: Option<&str>,
        seen: &mut BTreeSet<u64>,
        result: &mut Vec<u64>,
    ) {
        let edge_ids = if outgoing {
            self.out_adj.get(node_id as usize)
        } else {
            self.in_adj.get(node_id as usize)
        };

        let Some(edge_ids) = edge_ids else {
            return;
        };

        for &edge_id in edge_ids {
            let Some(edge) = self.edge_record(edge_id) else {
                continue;
            };
            if let Some(filter) = edge_type {
                if edge.edge_type != filter {
                    continue;
                }
            }
            let next = if outgoing { edge.target } else { edge.source };
            if seen.insert(next) {
                result.push(next);
            }
        }
    }

    fn neighbors_for_direction(
        &self,
        node_id: u64,
        direction: Direction,
        edge_type: Option<&str>,
    ) -> Vec<u64> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        if direction.includes_out() {
            self.collect_neighbors(node_id, true, edge_type, &mut seen, &mut result);
        }
        if direction.includes_in() {
            self.collect_neighbors(node_id, false, edge_type, &mut seen, &mut result);
        }
        result
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Out,
    In,
    Both,
}

impl Direction {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "out" | "outgoing" => Ok(Self::Out),
            "in" | "incoming" => Ok(Self::In),
            "both" | "all" => Ok(Self::Both),
            other => Err(format!(
                "direction must be 'out', 'in', or 'both', got {other:?}"
            )),
        }
    }

    fn includes_out(self) -> bool {
        matches!(self, Self::Out | Self::Both)
    }

    fn includes_in(self) -> bool {
        matches!(self, Self::In | Self::Both)
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("{field} cannot be empty"))
    } else {
        Ok(())
    }
}

fn edge_weight(edge: &EdgeRecord, edge_property: &str) -> Result<f64, String> {
    match edge.properties.get(edge_property) {
        Some(value) => {
            let parsed = value.parse::<f64>().map_err(|_| {
                format!(
                    "edge {} property {edge_property:?} is not a floating-point value",
                    edge.id
                )
            })?;
            if parsed.is_finite() && parsed >= 0.0 {
                Ok(parsed)
            } else {
                Err(format!(
                    "edge {} property {edge_property:?} must be finite and non-negative",
                    edge.id
                ))
            }
        }
        None => Ok(1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(graph.get_edge(0).unwrap().properties["probability"], "0.75");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("db-shm"));
        let _ = fs::remove_file(path.with_extension("db-wal"));
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

    fn map<const N: usize>(values: [(&str, &str); N]) -> HashMap<String, String> {
        values
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
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
}
