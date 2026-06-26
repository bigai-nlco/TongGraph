use super::GraphCore;
use crate::models::{PropertyMap, PropertyValue};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) struct GraphSchema {
    pub(crate) labels: Vec<LabelSchema>,
    pub(crate) edge_types: Vec<TypeSchema>,
    pub(crate) node_properties: Vec<GraphPropertySchema>,
    pub(crate) edge_properties: Vec<GraphPropertySchema>,
}

#[derive(Clone, Debug)]
pub(crate) struct LabelSchema {
    pub(crate) name: String,
    pub(crate) count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct TypeSchema {
    pub(crate) name: String,
    pub(crate) count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct GraphPropertySchema {
    pub(crate) key: String,
    pub(crate) types: Vec<String>,
    pub(crate) count: usize,
    pub(crate) samples: Vec<PropertyValue>,
}

#[derive(Clone, Debug)]
pub(crate) struct GraphStats {
    pub(crate) persistence_mode: String,
    pub(crate) nodes: usize,
    pub(crate) edges: usize,
    pub(crate) variables: usize,
    pub(crate) factors: usize,
    pub(crate) evidence: usize,
    pub(crate) traces: usize,
    pub(crate) labels: Vec<LabelSchema>,
    pub(crate) edge_types: Vec<TypeSchema>,
    pub(crate) fulltext_indexes: usize,
    pub(crate) vector_indexes: usize,
    pub(crate) vectors: usize,
    pub(crate) segment: SegmentStats,
}

#[derive(Clone, Debug)]
pub(crate) struct SegmentStats {
    pub(crate) base_edges: usize,
    pub(crate) delta_edges: usize,
}

impl GraphCore {
    pub(crate) fn schema_summary(&self) -> GraphSchema {
        GraphSchema {
            labels: self.label_counts(),
            edge_types: self.edge_type_counts(),
            node_properties: property_schema(
                self.nodes.iter().flatten().map(|node| &node.properties),
            ),
            edge_properties: property_schema(
                self.edges.iter().flatten().map(|edge| &edge.properties),
            ),
        }
    }

    pub(crate) fn stats_summary(&self) -> GraphStats {
        GraphStats {
            persistence_mode: if self.store.is_some() {
                "sqlite".to_string()
            } else {
                "memory".to_string()
            },
            nodes: self.node_count(),
            edges: self.edge_count(),
            variables: self.variable_count(),
            factors: self.factor_count(),
            evidence: self.evidence_count(),
            traces: self.trace_count(),
            labels: self.label_counts(),
            edge_types: self.edge_type_counts(),
            fulltext_indexes: self.fulltext_indexes.len(),
            vector_indexes: self.vector_indexes.len(),
            vectors: self.vectors.values().map(|vectors| vectors.len()).sum(),
            segment: SegmentStats {
                base_edges: self.base_segment.edge_count(),
                delta_edges: self.delta_edge_count(),
            },
        }
    }

    fn label_counts(&self) -> Vec<LabelSchema> {
        let mut labels = self
            .label_index
            .iter()
            .map(|(name, ids)| LabelSchema {
                name: name.clone(),
                count: ids.len(),
            })
            .collect::<Vec<_>>();
        labels.sort_by(|left, right| left.name.cmp(&right.name));
        labels
    }

    fn edge_type_counts(&self) -> Vec<TypeSchema> {
        let mut edge_types = self
            .edge_type_index
            .iter()
            .map(|(name, ids)| TypeSchema {
                name: name.clone(),
                count: ids.len(),
            })
            .collect::<Vec<_>>();
        edge_types.sort_by(|left, right| left.name.cmp(&right.name));
        edge_types
    }
}

fn property_schema<'a>(records: impl Iterator<Item = &'a PropertyMap>) -> Vec<GraphPropertySchema> {
    let mut properties: BTreeMap<String, PropertyAccumulator> = BTreeMap::new();
    for record in records {
        for (key, value) in record {
            let entry = properties.entry(key.clone()).or_default();
            entry.count += 1;
            entry.types.insert(value.type_name().to_string());
            if entry.samples.len() < 3 && !entry.samples.iter().any(|sample| sample == value) {
                entry.samples.push(value.clone());
            }
        }
    }

    properties
        .into_iter()
        .map(|(key, value)| GraphPropertySchema {
            key,
            types: value.types.into_iter().collect(),
            count: value.count,
            samples: value.samples,
        })
        .collect()
}

#[derive(Default)]
struct PropertyAccumulator {
    count: usize,
    types: BTreeSet<String>,
    samples: Vec<PropertyValue>,
}
