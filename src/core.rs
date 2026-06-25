mod adjacency;
mod algorithms;
mod direction;
mod entities;
mod inference;
mod lifecycle;
mod metadata;
mod mutations;
mod properties;
mod queries;
pub(crate) mod segment;
mod traversal;

#[cfg(test)]
mod tests;

pub(crate) use algorithms::{ComputeJob, ComputeResult, ShortestPath};
pub(crate) use inference::{ActiveSubgraph, BeliefPropagationResult};
pub(crate) use queries::{
    EdgePattern, NodePattern, PropertyConstraint, PropertyOperator, QueryDirection, QueryElement,
    QueryRow, QuerySpec, QueryValue,
};

use crate::models::{
    EdgeRecord, EvidenceRecord, FactorRecord, FactorTableRecord, NodeRecord, TraceRecord,
    VariableRecord,
};
use crate::sqlite::GraphStore;
use properties::PropertyIndexKey;
use segment::ComputeSegment;
use std::collections::{BTreeSet, HashMap};

pub(crate) struct GraphCore {
    nodes: Vec<Option<NodeRecord>>,
    edges: Vec<Option<EdgeRecord>>,
    variables: Vec<Option<VariableRecord>>,
    factors: Vec<Option<FactorRecord>>,
    factor_tables: HashMap<u64, FactorTableRecord>,
    posteriors: HashMap<u64, Vec<f64>>,
    evidence: Vec<Option<EvidenceRecord>>,
    traces: Vec<Option<TraceRecord>>,
    base_segment: ComputeSegment,
    delta_out_adj: Vec<Vec<u64>>,
    delta_in_adj: Vec<Vec<u64>>,
    node_by_external_id: HashMap<String, u64>,
    label_index: HashMap<String, BTreeSet<u64>>,
    edge_type_index: HashMap<String, BTreeSet<u64>>,
    node_property_key_index: HashMap<String, BTreeSet<u64>>,
    node_property_value_index: HashMap<PropertyIndexKey, BTreeSet<u64>>,
    edge_property_key_index: HashMap<String, BTreeSet<u64>>,
    edge_property_value_index: HashMap<PropertyIndexKey, BTreeSet<u64>>,
    next_node_id: u64,
    next_edge_id: u64,
    next_variable_id: u64,
    next_factor_id: u64,
    next_evidence_id: u64,
    next_trace_id: u64,
    store: Option<Box<dyn GraphStore>>,
    store_op_seq: Option<u64>,
    mutation_version: u64,
}
