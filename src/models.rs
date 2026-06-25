use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl PropertyValue {
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
        }
    }

    pub(crate) fn encoded_value(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::String(value) => value.clone(),
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Bool(_) => None,
            Self::Int(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            Self::String(value) => value.parse::<f64>().ok(),
        }
    }
}

pub(crate) type PropertyMap = HashMap<String, PropertyValue>;

#[derive(Clone, Debug)]
pub(crate) struct NewNodeRecord {
    pub(crate) external_id: Option<String>,
    pub(crate) labels: Vec<String>,
    pub(crate) properties: PropertyMap,
}

#[derive(Clone, Debug)]
pub(crate) struct NewEdgeRecord {
    pub(crate) source: u64,
    pub(crate) target: u64,
    pub(crate) edge_type: String,
    pub(crate) properties: PropertyMap,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NodeRecord {
    pub(crate) id: u64,
    pub(crate) external_id: String,
    pub(crate) labels: Vec<String>,
    pub(crate) properties: PropertyMap,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EdgeRecord {
    pub(crate) id: u64,
    pub(crate) source: u64,
    pub(crate) target: u64,
    pub(crate) edge_type: String,
    pub(crate) properties: PropertyMap,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GraphChanges {
    pub(crate) upsert_nodes: Vec<NodeRecord>,
    pub(crate) upsert_edges: Vec<EdgeRecord>,
    pub(crate) delete_node_ids: Vec<u64>,
    pub(crate) delete_edge_ids: Vec<u64>,
    pub(crate) next_node_id: u64,
    pub(crate) next_edge_id: u64,
    pub(crate) counters_changed: bool,
}

impl GraphChanges {
    pub(crate) fn is_empty(&self) -> bool {
        self.upsert_nodes.is_empty()
            && self.upsert_edges.is_empty()
            && self.delete_node_ids.is_empty()
            && self.delete_edge_ids.is_empty()
            && !self.counters_changed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FullTextIndexDefinition {
    pub(crate) name: String,
    pub(crate) target: String,
    pub(crate) properties: Vec<String>,
    pub(crate) tokenizer: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FullTextSearchResult {
    pub(crate) kind: String,
    pub(crate) id: u64,
    pub(crate) score: f64,
    pub(crate) matched_fields: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct VariableRecord {
    pub(crate) id: u64,
    pub(crate) owner_id: Option<u64>,
    pub(crate) domain: String,
    pub(crate) states: Vec<String>,
    pub(crate) prior: PropertyMap,
    pub(crate) posterior: PropertyMap,
}

#[derive(Clone, Debug)]
pub(crate) struct FactorRecord {
    pub(crate) id: u64,
    pub(crate) input_variables: Vec<u64>,
    pub(crate) output_variables: Vec<u64>,
    pub(crate) function: String,
    pub(crate) parameters: PropertyMap,
}

#[derive(Clone, Debug)]
pub(crate) struct EvidenceRecord {
    pub(crate) id: u64,
    pub(crate) variable_id: u64,
    pub(crate) payload: PropertyMap,
}

#[derive(Clone, Debug)]
pub(crate) struct TraceRecord {
    pub(crate) id: u64,
    pub(crate) payload: PropertyMap,
}

#[derive(Clone, Debug)]
pub(crate) struct FactorTableRecord {
    pub(crate) factor_id: u64,
    pub(crate) variables: Vec<u64>,
    pub(crate) values: Vec<f64>,
    pub(crate) is_cpd: bool,
}
