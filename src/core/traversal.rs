use super::adjacency::edge_weight;
use super::direction::Direction;
use super::GraphCore;
use std::collections::{BTreeSet, HashMap, VecDeque};

impl GraphCore {
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

    pub(crate) fn frontier(
        &self,
        starts: &[u64],
        steps: usize,
        direction: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<u64>, String> {
        let direction = Direction::parse(direction)?;
        let mut frontier = Vec::new();
        let mut frontier_seen = BTreeSet::new();
        for &node_id in starts {
            self.require_node(node_id)?;
            if frontier_seen.insert(node_id) {
                frontier.push(node_id);
            }
        }

        if steps == 0 {
            return Ok(frontier);
        }

        let mut visited = frontier_seen;
        for _ in 0..steps {
            let mut next_frontier = Vec::new();
            let mut next_seen = BTreeSet::new();
            for node_id in frontier {
                for next in self.neighbors_for_direction(node_id, direction, edge_type) {
                    if visited.insert(next) && next_seen.insert(next) {
                        next_frontier.push(next);
                    }
                }
            }
            frontier = next_frontier;
            if frontier.is_empty() {
                break;
            }
        }

        Ok(frontier)
    }

    pub(crate) fn propagate(
        &self,
        seeds: &HashMap<u64, f64>,
        steps: usize,
        edge_type: Option<&str>,
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
                for edge_id in self.edge_ids_for_node(source, true) {
                    let Some(edge) = self.edge_record(edge_id) else {
                        continue;
                    };
                    if let Some(filter) = edge_type {
                        if edge.edge_type != filter {
                            continue;
                        }
                    }
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
}
