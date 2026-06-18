use super::properties::{index_properties, validate_non_empty, validate_properties};
use super::GraphCore;
use crate::models::{EdgeRecord, NewEdgeRecord, NewNodeRecord, NodeRecord, PropertyMap};
use std::collections::BTreeSet;

impl GraphCore {
    pub(crate) fn add_node(
        &mut self,
        external_id: Option<String>,
        labels: Vec<String>,
        properties: PropertyMap,
    ) -> Result<u64, String> {
        let ids = self.add_nodes(vec![NewNodeRecord {
            external_id,
            labels,
            properties,
        }])?;
        Ok(ids[0])
    }

    pub(crate) fn add_edge(
        &mut self,
        source: u64,
        target: u64,
        edge_type: String,
        properties: PropertyMap,
    ) -> Result<u64, String> {
        let ids = self.add_edges(vec![NewEdgeRecord {
            source,
            target,
            edge_type,
            properties,
        }])?;
        Ok(ids[0])
    }

    pub(crate) fn add_nodes(&mut self, nodes: Vec<NewNodeRecord>) -> Result<Vec<u64>, String> {
        let mut external_ids = BTreeSet::new();
        let mut records = Vec::with_capacity(nodes.len());
        for (offset, node) in nodes.into_iter().enumerate() {
            for label in &node.labels {
                validate_non_empty("label", label)?;
            }
            validate_properties(&node.properties)?;
            let id = self
                .next_node_id
                .checked_add(offset as u64)
                .ok_or_else(|| "node id overflows".to_string())?;
            let external_id = node.external_id.unwrap_or_else(|| format!("node:{id}"));
            validate_non_empty("external_id", &external_id)?;
            if self.node_by_external_id.contains_key(&external_id)
                || !external_ids.insert(external_id.clone())
            {
                return Err(format!("external_id {external_id:?} already exists"));
            }
            records.push(NodeRecord {
                id,
                external_id,
                labels: node.labels,
                properties: node.properties,
            });
        }

        if self.store.is_some() {
            self.ensure_store_current()?;
            self.store.as_ref().unwrap().insert_nodes(&records)?;
            self.refresh_store_op_seq()?;
        }
        let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        for record in records {
            self.insert_node_record(record)?;
        }
        Ok(ids)
    }

    pub(crate) fn add_edges(&mut self, edges: Vec<NewEdgeRecord>) -> Result<Vec<u64>, String> {
        let mut records = Vec::with_capacity(edges.len());
        for (offset, edge) in edges.into_iter().enumerate() {
            validate_non_empty("edge_type", &edge.edge_type)?;
            validate_properties(&edge.properties)?;
            self.require_node(edge.source)?;
            self.require_node(edge.target)?;
            let id = self
                .next_edge_id
                .checked_add(offset as u64)
                .ok_or_else(|| "edge id overflows".to_string())?;
            records.push(EdgeRecord {
                id,
                source: edge.source,
                target: edge.target,
                edge_type: edge.edge_type,
                properties: edge.properties,
            });
        }

        if self.store.is_some() {
            self.ensure_store_current()?;
            self.store.as_ref().unwrap().insert_edges(&records)?;
            self.refresh_store_op_seq()?;
        }
        let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        for record in records {
            self.insert_edge_record(record)?;
        }
        self.maybe_auto_compact_segments()?;
        Ok(ids)
    }

    pub(super) fn insert_loaded_node(&mut self, record: NodeRecord) -> Result<(), String> {
        self.insert_node_record(record)
    }

    pub(super) fn insert_loaded_edge(&mut self, record: EdgeRecord) -> Result<(), String> {
        self.require_node(record.source)?;
        self.require_node(record.target)?;
        self.insert_edge_record(record)
    }

    pub(super) fn insert_node_record(&mut self, record: NodeRecord) -> Result<(), String> {
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
        index_properties(
            id,
            &record.properties,
            &mut self.node_property_key_index,
            &mut self.node_property_value_index,
        );
        self.node_by_external_id
            .insert(record.external_id.clone(), id);
        self.nodes[id as usize] = Some(record);
        self.next_node_id = self.next_node_id.max(id + 1);
        Ok(())
    }

    pub(super) fn insert_edge_record(&mut self, record: EdgeRecord) -> Result<(), String> {
        let id = record.id;
        self.ensure_edge_slot(id);
        if self.edges[id as usize].is_some() {
            return Err(format!("edge id {id} already exists"));
        }

        self.ensure_node_slot(record.source);
        self.ensure_node_slot(record.target);
        self.delta_out_adj[record.source as usize].push(id);
        self.delta_in_adj[record.target as usize].push(id);
        self.edge_type_index
            .entry(record.edge_type.clone())
            .or_default()
            .insert(id);
        index_properties(
            id,
            &record.properties,
            &mut self.edge_property_key_index,
            &mut self.edge_property_value_index,
        );
        self.edges[id as usize] = Some(record);
        self.next_edge_id = self.next_edge_id.max(id + 1);
        Ok(())
    }

    pub(super) fn require_node(&self, node_id: u64) -> Result<(), String> {
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
        if self.delta_out_adj.len() < size {
            self.delta_out_adj.resize_with(size, Vec::new);
        }
        if self.delta_in_adj.len() < size {
            self.delta_in_adj.resize_with(size, Vec::new);
        }
    }

    fn ensure_edge_slot(&mut self, edge_id: u64) {
        let size = edge_id as usize + 1;
        if self.edges.len() < size {
            self.edges.resize_with(size, || None);
        }
    }
}
