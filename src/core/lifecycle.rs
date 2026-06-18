use super::{segment::ComputeSegment, GraphCore};
use crate::sqlite::{GraphStore, SqliteStore};
use std::collections::HashMap;

impl GraphCore {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            variables: Vec::new(),
            factors: Vec::new(),
            factor_tables: HashMap::new(),
            posteriors: HashMap::new(),
            evidence: Vec::new(),
            traces: Vec::new(),
            base_segment: ComputeSegment::default(),
            delta_out_adj: Vec::new(),
            delta_in_adj: Vec::new(),
            node_by_external_id: HashMap::new(),
            label_index: HashMap::new(),
            edge_type_index: HashMap::new(),
            node_property_key_index: HashMap::new(),
            node_property_value_index: HashMap::new(),
            edge_property_key_index: HashMap::new(),
            edge_property_value_index: HashMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
            next_variable_id: 0,
            next_factor_id: 0,
            next_evidence_id: 0,
            next_trace_id: 0,
            store: None,
            store_op_seq: None,
        }
    }

    pub(crate) fn open(path: &str) -> Result<Self, String> {
        let store: Box<dyn GraphStore> = Box::new(SqliteStore::open(path)?);
        let mut core = Self::new();
        for node in store.load_nodes()? {
            core.insert_loaded_node(node)?;
        }
        for edge in store.load_edges()? {
            core.insert_loaded_edge(edge)?;
        }
        let node_count = core.nodes.len();
        let edge_count = core.edge_count();
        if let Some(segment) = store.load_segment(node_count, edge_count)? {
            core.base_segment = segment;
            core.clear_delta_segments();
        } else {
            core.rebuild_compacted_segment();
            if edge_count > 0 {
                store.save_segment(&core.base_segment, node_count, edge_count)?;
            }
        }
        for variable in store.load_variables()? {
            core.insert_loaded_variable(variable)?;
        }
        for factor in store.load_factors()? {
            core.insert_loaded_factor(factor)?;
        }
        for factor_table in store.load_factor_tables()? {
            core.insert_loaded_factor_table(factor_table)?;
        }
        for (variable_id, posterior) in store.load_posteriors()? {
            core.insert_loaded_posterior(variable_id, posterior)?;
        }
        for evidence in store.load_evidence()? {
            core.insert_loaded_evidence(evidence)?;
        }
        for trace in store.load_traces()? {
            core.insert_loaded_trace(trace)?;
        }
        core.store_op_seq = Some(store.current_op_seq()?);
        core.store = Some(store);
        Ok(core)
    }

    pub(crate) fn compact_segments(&mut self) -> Result<(), String> {
        self.ensure_store_current()?;
        self.rebuild_compacted_segment();
        self.persist_segment()
    }

    pub(crate) fn refresh(&mut self) -> Result<(), String> {
        let path = self
            .store
            .as_ref()
            .map(|store| store.path())
            .ok_or_else(|| "refresh is only available for SQLite-backed graphs".to_string())?;
        *self = Self::open(&path)?;
        Ok(())
    }

    pub(super) fn maybe_auto_compact_segments(&mut self) -> Result<(), String> {
        if self.store.is_none() {
            return Ok(());
        }
        let delta_edges = self.delta_edge_count();
        let base_edges = self.base_segment.edge_count();
        let exceeds_delta_limit = delta_edges > 1024;
        let exceeds_base_ratio = base_edges > 0 && delta_edges * 4 > base_edges;
        if exceeds_delta_limit || exceeds_base_ratio {
            self.compact_segments()?;
        }
        Ok(())
    }

    fn rebuild_compacted_segment(&mut self) {
        let node_count = self.nodes.len();
        let mut out_adj = vec![Vec::new(); node_count];
        let mut in_adj = vec![Vec::new(); node_count];

        for edge in self.edges.iter().flatten() {
            if let Some(edge_ids) = out_adj.get_mut(edge.source as usize) {
                edge_ids.push(edge.id);
            }
            if let Some(edge_ids) = in_adj.get_mut(edge.target as usize) {
                edge_ids.push(edge.id);
            }
        }

        self.base_segment = ComputeSegment::from_adjacency(&out_adj, &in_adj);
        self.clear_delta_segments();
    }

    fn clear_delta_segments(&mut self) {
        let node_count = self.nodes.len();
        self.delta_out_adj = vec![Vec::new(); node_count];
        self.delta_in_adj = vec![Vec::new(); node_count];
    }

    fn persist_segment(&self) -> Result<(), String> {
        if let Some(store) = &self.store {
            store.save_segment(&self.base_segment, self.nodes.len(), self.edge_count())?;
        }
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            variables: self.variables.clone(),
            factors: self.factors.clone(),
            factor_tables: self.factor_tables.clone(),
            posteriors: self.posteriors.clone(),
            evidence: self.evidence.clone(),
            traces: self.traces.clone(),
            base_segment: self.base_segment.clone(),
            delta_out_adj: self.delta_out_adj.clone(),
            delta_in_adj: self.delta_in_adj.clone(),
            node_by_external_id: self.node_by_external_id.clone(),
            label_index: self.label_index.clone(),
            edge_type_index: self.edge_type_index.clone(),
            node_property_key_index: self.node_property_key_index.clone(),
            node_property_value_index: self.node_property_value_index.clone(),
            edge_property_key_index: self.edge_property_key_index.clone(),
            edge_property_value_index: self.edge_property_value_index.clone(),
            next_node_id: self.next_node_id,
            next_edge_id: self.next_edge_id,
            next_variable_id: self.next_variable_id,
            next_factor_id: self.next_factor_id,
            next_evidence_id: self.next_evidence_id,
            next_trace_id: self.next_trace_id,
            store: None,
            store_op_seq: None,
        }
    }

    pub(crate) fn transaction_snapshot(&self) -> (Self, u64, u64) {
        (self.snapshot(), self.next_node_id, self.next_edge_id)
    }

    pub(crate) fn commit_transaction_snapshot(
        &mut self,
        staged: &Self,
        base_next_node_id: u64,
        base_next_edge_id: u64,
    ) -> Result<(), String> {
        if self.next_node_id != base_next_node_id || self.next_edge_id != base_next_edge_id {
            return Err(
                "graph changed since transaction started; rollback and retry the transaction"
                    .to_string(),
            );
        }

        let nodes = staged
            .nodes
            .iter()
            .flatten()
            .filter(|node| node.id >= base_next_node_id)
            .cloned()
            .collect::<Vec<_>>();
        let edges = staged
            .edges
            .iter()
            .flatten()
            .filter(|edge| edge.id >= base_next_edge_id)
            .cloned()
            .collect::<Vec<_>>();

        if nodes.is_empty() && edges.is_empty() {
            return Ok(());
        }

        if self.store.is_some() {
            self.ensure_store_current()?;
            self.store
                .as_ref()
                .unwrap()
                .insert_graph_records(&nodes, &edges)?;
            self.refresh_store_op_seq()?;
        }

        for node in nodes {
            self.insert_node_record(node)?;
        }
        for edge in edges {
            self.insert_edge_record(edge)?;
        }
        self.maybe_auto_compact_segments()
    }

    pub(super) fn ensure_store_current(&self) -> Result<(), String> {
        let Some(store) = &self.store else {
            return Ok(());
        };
        let current = store.current_op_seq()?;
        if self.store_op_seq == Some(current) {
            Ok(())
        } else {
            Err(
                "SQLite graph has changed since this handle was opened; call refresh() before writing"
                    .to_string(),
            )
        }
    }

    pub(super) fn refresh_store_op_seq(&mut self) -> Result<(), String> {
        if let Some(store) = &self.store {
            self.store_op_seq = Some(store.current_op_seq()?);
        }
        Ok(())
    }
}
