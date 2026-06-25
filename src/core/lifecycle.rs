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
            fulltext_indexes: HashMap::new(),
            vector_indexes: HashMap::new(),
            vectors: HashMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
            next_variable_id: 0,
            next_factor_id: 0,
            next_evidence_id: 0,
            next_trace_id: 0,
            store: None,
            store_op_seq: None,
            mutation_version: 0,
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
        let fulltext_indexes = store.load_fulltext_indexes()?;
        store.rebuild_fulltext_indexes(&fulltext_indexes, &core.nodes(), &core.edges())?;
        core.fulltext_indexes = fulltext_indexes
            .into_iter()
            .map(|definition| (definition.name.clone(), definition))
            .collect();
        core.load_vector_state(store.load_vector_indexes()?, store.load_vectors()?)?;
        let (next_node_id, next_edge_id) = store.load_next_ids()?;
        if let Some(next_node_id) = next_node_id {
            core.next_node_id = core.next_node_id.max(next_node_id);
        }
        if let Some(next_edge_id) = next_edge_id {
            core.next_edge_id = core.next_edge_id.max(next_edge_id);
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

    pub(super) fn rebuild_compacted_segment(&mut self) {
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
            fulltext_indexes: self.fulltext_indexes.clone(),
            vector_indexes: self.vector_indexes.clone(),
            vectors: self.vectors.clone(),
            next_node_id: self.next_node_id,
            next_edge_id: self.next_edge_id,
            next_variable_id: self.next_variable_id,
            next_factor_id: self.next_factor_id,
            next_evidence_id: self.next_evidence_id,
            next_trace_id: self.next_trace_id,
            store: None,
            store_op_seq: None,
            mutation_version: self.mutation_version,
        }
    }

    pub(crate) fn transaction_snapshot(&self) -> (Self, u64) {
        (self.snapshot(), self.mutation_version)
    }

    pub(crate) fn commit_transaction_snapshot(
        &mut self,
        staged: &Self,
        base_version: u64,
    ) -> Result<(), String> {
        if self.mutation_version != base_version {
            return Err(
                "graph changed since transaction started; rollback and retry the transaction"
                    .to_string(),
            );
        }

        let changes = self.graph_changes(staged);
        if changes.is_empty() {
            return Ok(());
        }

        let mut published = staged.snapshot();
        published.rebuild_derived_state();
        if let Some(store) = &self.store {
            self.ensure_store_current()?;
            store.apply_graph_changes(&changes)?;
            let _ = store.save_segment(
                &published.base_segment,
                published.nodes.len(),
                published.edge_count(),
            );
        }
        self.publish_staged(&published, base_version.wrapping_add(1));
        if self.store.is_some() {
            self.refresh_store_op_seq()?;
        }
        Ok(())
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
