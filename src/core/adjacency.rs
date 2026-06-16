use super::direction::Direction;
use super::GraphCore;
use crate::models::EdgeRecord;
use std::collections::BTreeSet;

impl GraphCore {
    pub(super) fn existing_node_ids(&self) -> Vec<u64> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(id, node)| node.as_ref().map(|_| id as u64))
            .collect()
    }

    pub(super) fn edge_record(&self, edge_id: u64) -> Option<&EdgeRecord> {
        self.edges.get(edge_id as usize).and_then(Option::as_ref)
    }

    pub(super) fn edge_ids_for_node(&self, node_id: u64, outgoing: bool) -> Vec<u64> {
        let mut edge_ids = Vec::new();
        if outgoing {
            edge_ids.extend_from_slice(self.base_segment.out_edges(node_id as usize));
            if let Some(delta) = self.delta_out_adj.get(node_id as usize) {
                edge_ids.extend(delta.iter().copied());
            }
        } else {
            edge_ids.extend_from_slice(self.base_segment.in_edges(node_id as usize));
            if let Some(delta) = self.delta_in_adj.get(node_id as usize) {
                edge_ids.extend(delta.iter().copied());
            }
        }
        edge_ids
    }

    pub(super) fn collect_neighbors(
        &self,
        node_id: u64,
        outgoing: bool,
        edge_type: Option<&str>,
        seen: &mut BTreeSet<u64>,
        result: &mut Vec<u64>,
    ) {
        for edge_id in self.edge_ids_for_node(node_id, outgoing) {
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

    pub(super) fn neighbors_for_direction(
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

    pub(super) fn neighbor_edges_for_direction(
        &self,
        node_id: u64,
        direction: Direction,
        edge_type: Option<&str>,
    ) -> Vec<(u64, u64)> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        if direction.includes_out() {
            self.collect_neighbor_edges(node_id, true, edge_type, &mut seen, &mut result);
        }
        if direction.includes_in() {
            self.collect_neighbor_edges(node_id, false, edge_type, &mut seen, &mut result);
        }
        result
    }

    pub(super) fn delta_edge_count(&self) -> usize {
        self.delta_out_adj.iter().map(Vec::len).sum()
    }

    fn collect_neighbor_edges(
        &self,
        node_id: u64,
        outgoing: bool,
        edge_type: Option<&str>,
        seen: &mut BTreeSet<u64>,
        result: &mut Vec<(u64, u64)>,
    ) {
        for edge_id in self.edge_ids_for_node(node_id, outgoing) {
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
                result.push((next, edge_id));
            }
        }
    }
}

pub(super) fn edge_weight(edge: &EdgeRecord, edge_property: &str) -> Result<f64, String> {
    match edge.properties.get(edge_property) {
        Some(value) => {
            let parsed = value.as_f64().ok_or_else(|| {
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
