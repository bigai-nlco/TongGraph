use super::properties::property_index_lookup;
use super::GraphCore;
use crate::models::{
    EdgeRecord, EvidenceRecord, FactorRecord, NodeRecord, PropertyValue, TraceRecord,
    VariableRecord,
};

impl GraphCore {
    pub(crate) fn node_count(&self) -> usize {
        self.nodes.iter().flatten().count()
    }

    pub(crate) fn edge_count(&self) -> usize {
        self.edges.iter().flatten().count()
    }

    pub(crate) fn variable_count(&self) -> usize {
        self.variables.iter().flatten().count()
    }

    pub(crate) fn factor_count(&self) -> usize {
        self.factors.iter().flatten().count()
    }

    pub(crate) fn evidence_count(&self) -> usize {
        self.evidence.iter().flatten().count()
    }

    pub(crate) fn trace_count(&self) -> usize {
        self.traces.iter().flatten().count()
    }

    pub(crate) fn get_node(&self, node_id: u64) -> Option<NodeRecord> {
        self.nodes.get(node_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn get_edge(&self, edge_id: u64) -> Option<EdgeRecord> {
        self.edges.get(edge_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn get_variable(&self, variable_id: u64) -> Option<VariableRecord> {
        self.variables
            .get(variable_id as usize)
            .and_then(Clone::clone)
    }

    pub(crate) fn get_factor(&self, factor_id: u64) -> Option<FactorRecord> {
        self.factors.get(factor_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn get_evidence(&self, evidence_id: u64) -> Option<EvidenceRecord> {
        self.evidence
            .get(evidence_id as usize)
            .and_then(Clone::clone)
    }

    pub(crate) fn get_trace(&self, trace_id: u64) -> Option<TraceRecord> {
        self.traces.get(trace_id as usize).and_then(Clone::clone)
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

    pub(crate) fn nodes_with_property(&self, key: &str, value: Option<&PropertyValue>) -> Vec<u64> {
        property_index_lookup(
            &self.node_property_key_index,
            &self.node_property_value_index,
            key,
            value,
        )
    }

    pub(crate) fn edges_with_property(&self, key: &str, value: Option<&PropertyValue>) -> Vec<u64> {
        property_index_lookup(
            &self.edge_property_key_index,
            &self.edge_property_value_index,
            key,
            value,
        )
    }
}
