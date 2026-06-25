use super::properties::{index_properties, validate_non_empty, validate_properties};
use super::GraphCore;
use crate::models::{EdgeRecord, GraphChanges, NodeRecord, PropertyMap};
use std::collections::BTreeSet;

impl GraphCore {
    pub(crate) fn update_node(
        &mut self,
        node_id: u64,
        external_id: Option<String>,
        add_labels: Vec<String>,
        remove_labels: Vec<String>,
        set_properties: PropertyMap,
        remove_properties: Vec<String>,
    ) -> Result<NodeRecord, String> {
        let base_version = self.mutation_version;
        let mut staged = self.snapshot();
        let record = staged.update_node_in_place(
            node_id,
            external_id,
            add_labels,
            remove_labels,
            set_properties,
            remove_properties,
        )?;
        self.commit_transaction_snapshot(&staged, base_version)?;
        Ok(record)
    }

    pub(crate) fn update_edge(
        &mut self,
        edge_id: u64,
        set_properties: PropertyMap,
        remove_properties: Vec<String>,
    ) -> Result<EdgeRecord, String> {
        let base_version = self.mutation_version;
        let mut staged = self.snapshot();
        let record = staged.update_edge_in_place(edge_id, set_properties, remove_properties)?;
        self.commit_transaction_snapshot(&staged, base_version)?;
        Ok(record)
    }

    pub(crate) fn delete_node(&mut self, node_id: u64, detach: bool) -> Result<(), String> {
        let base_version = self.mutation_version;
        let mut staged = self.snapshot();
        staged.delete_node_in_place(node_id, detach)?;
        self.commit_transaction_snapshot(&staged, base_version)
    }

    pub(crate) fn delete_edge(&mut self, edge_id: u64) -> Result<(), String> {
        let base_version = self.mutation_version;
        let mut staged = self.snapshot();
        staged.delete_edge_in_place(edge_id)?;
        self.commit_transaction_snapshot(&staged, base_version)
    }

    pub(crate) fn update_node_in_place(
        &mut self,
        node_id: u64,
        external_id: Option<String>,
        add_labels: Vec<String>,
        remove_labels: Vec<String>,
        set_properties: PropertyMap,
        remove_properties: Vec<String>,
    ) -> Result<NodeRecord, String> {
        self.require_node(node_id)?;
        for label in add_labels.iter().chain(remove_labels.iter()) {
            validate_non_empty("label", label)?;
        }
        validate_properties(&set_properties)?;
        if set_properties.contains_key("external_id")
            || remove_properties.iter().any(|key| key == "external_id")
        {
            return Err("external_id must be changed through the external_id field".to_string());
        }
        for key in &remove_properties {
            validate_non_empty("property key", key)?;
        }
        if let Some(value) = &external_id {
            validate_non_empty("external_id", value)?;
            if self
                .node_by_external_id
                .get(value)
                .is_some_and(|existing| *existing != node_id)
            {
                return Err(format!("external_id {value:?} already exists"));
            }
        }

        let record = self.nodes[node_id as usize].as_mut().unwrap();
        if let Some(external_id) = external_id {
            record.external_id = external_id;
        }
        let remove_labels = remove_labels.into_iter().collect::<BTreeSet<_>>();
        record.labels.retain(|label| !remove_labels.contains(label));
        for label in add_labels {
            if !record.labels.contains(&label) {
                record.labels.push(label);
            }
        }
        for key in remove_properties {
            record.properties.remove(&key);
        }
        record.properties.extend(set_properties);
        let result = record.clone();
        self.rebuild_derived_state();
        self.mutation_version = self.mutation_version.wrapping_add(1);
        Ok(result)
    }

    pub(crate) fn update_edge_in_place(
        &mut self,
        edge_id: u64,
        set_properties: PropertyMap,
        remove_properties: Vec<String>,
    ) -> Result<EdgeRecord, String> {
        validate_properties(&set_properties)?;
        for key in &remove_properties {
            validate_non_empty("property key", key)?;
        }
        let record = self
            .edges
            .get_mut(edge_id as usize)
            .and_then(Option::as_mut)
            .ok_or_else(|| format!("edge {edge_id} not found"))?;
        for key in remove_properties {
            record.properties.remove(&key);
        }
        record.properties.extend(set_properties);
        let result = record.clone();
        self.rebuild_derived_state();
        self.mutation_version = self.mutation_version.wrapping_add(1);
        Ok(result)
    }

    pub(crate) fn delete_node_in_place(
        &mut self,
        node_id: u64,
        detach: bool,
    ) -> Result<(), String> {
        self.require_node(node_id)?;
        if self
            .variables
            .iter()
            .flatten()
            .any(|variable| variable.owner_id == Some(node_id))
        {
            return Err(format!(
                "node {node_id} owns probabilistic variables and cannot be deleted"
            ));
        }
        let incident = self
            .edges
            .iter()
            .flatten()
            .filter(|edge| edge.source == node_id || edge.target == node_id)
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        if !detach && !incident.is_empty() {
            return Err(format!(
                "node {node_id} still has relationships; use detach deletion"
            ));
        }
        for edge_id in incident {
            self.edges[edge_id as usize] = None;
        }
        self.nodes[node_id as usize] = None;
        self.rebuild_derived_state();
        self.mutation_version = self.mutation_version.wrapping_add(1);
        Ok(())
    }

    pub(crate) fn delete_edge_in_place(&mut self, edge_id: u64) -> Result<(), String> {
        let edge = self
            .edges
            .get_mut(edge_id as usize)
            .and_then(Option::take)
            .ok_or_else(|| format!("edge {edge_id} not found"))?;
        let _ = edge;
        self.rebuild_derived_state();
        self.mutation_version = self.mutation_version.wrapping_add(1);
        Ok(())
    }

    pub(super) fn graph_changes(&self, staged: &Self) -> GraphChanges {
        let mut changes = GraphChanges::default();
        let node_len = self.nodes.len().max(staged.nodes.len());
        for index in 0..node_len {
            let current = self.nodes.get(index).and_then(Option::as_ref);
            let next = staged.nodes.get(index).and_then(Option::as_ref);
            match (current, next) {
                (Some(_), None) => changes.delete_node_ids.push(index as u64),
                (None, Some(record)) => changes.upsert_nodes.push(record.clone()),
                (Some(current), Some(next)) if current != next => {
                    changes.upsert_nodes.push(next.clone())
                }
                _ => {}
            }
        }
        let edge_len = self.edges.len().max(staged.edges.len());
        for index in 0..edge_len {
            let current = self.edges.get(index).and_then(Option::as_ref);
            let next = staged.edges.get(index).and_then(Option::as_ref);
            match (current, next) {
                (Some(_), None) => changes.delete_edge_ids.push(index as u64),
                (None, Some(record)) => changes.upsert_edges.push(record.clone()),
                (Some(current), Some(next)) if current != next => {
                    changes.upsert_edges.push(next.clone())
                }
                _ => {}
            }
        }
        changes.next_node_id = staged.next_node_id;
        changes.next_edge_id = staged.next_edge_id;
        changes.counters_changed =
            self.next_node_id != staged.next_node_id || self.next_edge_id != staged.next_edge_id;
        changes
    }

    pub(super) fn rebuild_derived_state(&mut self) {
        self.node_by_external_id.clear();
        self.label_index.clear();
        self.edge_type_index.clear();
        self.node_property_key_index.clear();
        self.node_property_value_index.clear();
        self.edge_property_key_index.clear();
        self.edge_property_value_index.clear();

        for node in self.nodes.iter().flatten() {
            self.node_by_external_id
                .insert(node.external_id.clone(), node.id);
            for label in &node.labels {
                self.label_index
                    .entry(label.clone())
                    .or_default()
                    .insert(node.id);
            }
            index_properties(
                node.id,
                &node.properties,
                &mut self.node_property_key_index,
                &mut self.node_property_value_index,
            );
        }
        for edge in self.edges.iter().flatten() {
            self.edge_type_index
                .entry(edge.edge_type.clone())
                .or_default()
                .insert(edge.id);
            index_properties(
                edge.id,
                &edge.properties,
                &mut self.edge_property_key_index,
                &mut self.edge_property_value_index,
            );
        }
        self.rebuild_compacted_segment();
    }

    pub(super) fn publish_staged(&mut self, staged: &Self, next_version: u64) {
        self.nodes = staged.nodes.clone();
        self.edges = staged.edges.clone();
        self.base_segment = staged.base_segment.clone();
        self.delta_out_adj = staged.delta_out_adj.clone();
        self.delta_in_adj = staged.delta_in_adj.clone();
        self.node_by_external_id = staged.node_by_external_id.clone();
        self.label_index = staged.label_index.clone();
        self.edge_type_index = staged.edge_type_index.clone();
        self.node_property_key_index = staged.node_property_key_index.clone();
        self.node_property_value_index = staged.node_property_value_index.clone();
        self.edge_property_key_index = staged.edge_property_key_index.clone();
        self.edge_property_value_index = staged.edge_property_value_index.clone();
        self.next_node_id = staged.next_node_id;
        self.next_edge_id = staged.next_edge_id;
        self.mutation_version = next_version;
    }
}
