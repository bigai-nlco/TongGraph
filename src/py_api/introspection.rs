use crate::core::{
    GraphPropertySchema, GraphSchema, GraphStats, LabelSchema, QueryProfile, SegmentStats,
    TypeSchema,
};
use crate::models::PropertyValue;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

pub(super) fn graph_schema_to_py(py: Python<'_>, schema: GraphSchema) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("labels", labels_to_py(py, &schema.labels)?)?;
    dict.set_item("edge_types", edge_types_to_py(py, &schema.edge_types)?)?;
    dict.set_item(
        "node_properties",
        property_schema_to_py(py, &schema.node_properties)?,
    )?;
    dict.set_item(
        "edge_properties",
        property_schema_to_py(py, &schema.edge_properties)?,
    )?;
    Ok(dict.into_any().unbind())
}

pub(super) fn graph_stats_to_py(py: Python<'_>, stats: GraphStats) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("persistence_mode", stats.persistence_mode)?;
    dict.set_item("nodes", stats.nodes)?;
    dict.set_item("edges", stats.edges)?;
    dict.set_item("variables", stats.variables)?;
    dict.set_item("factors", stats.factors)?;
    dict.set_item("evidence", stats.evidence)?;
    dict.set_item("traces", stats.traces)?;
    dict.set_item("labels", labels_to_py(py, &stats.labels)?)?;
    dict.set_item("edge_types", edge_types_to_py(py, &stats.edge_types)?)?;
    dict.set_item("fulltext_indexes", stats.fulltext_indexes)?;
    dict.set_item("vector_indexes", stats.vector_indexes)?;
    dict.set_item("vectors", stats.vectors)?;
    dict.set_item("segment", segment_stats_to_py(py, &stats.segment)?)?;
    Ok(dict.into_any().unbind())
}

pub(super) fn query_profile_to_py(py: Python<'_>, profile: &QueryProfile) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("plan_steps", profile.plan_steps.clone())?;
    dict.set_item("chosen_anchor_index", profile.chosen_anchor_index)?;
    dict.set_item("chosen_anchor_alias", &profile.chosen_anchor_alias)?;
    dict.set_item("anchor_candidates", profile.anchor_candidates)?;
    dict.set_item("expanded_edges", profile.expanded_edges)?;
    dict.set_item("filtered_edges", profile.filtered_edges)?;
    dict.set_item("filtered_nodes", profile.filtered_nodes)?;
    dict.set_item("alias_conflicts", profile.alias_conflicts)?;
    dict.set_item("result_count", profile.result_count)?;
    dict.set_item("elapsed_ns", profile.elapsed_ns)?;
    Ok(dict.into_any().unbind())
}

fn labels_to_py(py: Python<'_>, labels: &[LabelSchema]) -> PyResult<Py<PyAny>> {
    let output = PyList::empty(py);
    for label in labels {
        let item = PyDict::new(py);
        item.set_item("name", &label.name)?;
        item.set_item("count", label.count)?;
        output.append(item)?;
    }
    Ok(output.into_any().unbind())
}

fn edge_types_to_py(py: Python<'_>, edge_types: &[TypeSchema]) -> PyResult<Py<PyAny>> {
    let output = PyList::empty(py);
    for edge_type in edge_types {
        let item = PyDict::new(py);
        item.set_item("name", &edge_type.name)?;
        item.set_item("count", edge_type.count)?;
        output.append(item)?;
    }
    Ok(output.into_any().unbind())
}

fn property_schema_to_py(
    py: Python<'_>,
    properties: &[GraphPropertySchema],
) -> PyResult<Py<PyAny>> {
    let output = PyList::empty(py);
    for property in properties {
        let item = PyDict::new(py);
        item.set_item("key", &property.key)?;
        item.set_item("types", property.types.clone())?;
        item.set_item("count", property.count)?;
        item.set_item("samples", property_samples_to_py(py, &property.samples)?)?;
        output.append(item)?;
    }
    Ok(output.into_any().unbind())
}

fn property_samples_to_py(py: Python<'_>, samples: &[PropertyValue]) -> PyResult<Py<PyAny>> {
    let list = PyList::empty(py);
    for sample in samples {
        let dict = PyDict::new(py);
        dict.set_item("type", sample.type_name())?;
        match sample {
            PropertyValue::Bool(value) => dict.set_item("value", *value)?,
            PropertyValue::Int(value) => dict.set_item("value", *value)?,
            PropertyValue::Float(value) => dict.set_item("value", *value)?,
            PropertyValue::String(value) => dict.set_item("value", value)?,
        }
        list.append(dict)?;
    }
    Ok(list.into_any().unbind())
}

fn segment_stats_to_py(py: Python<'_>, segment: &SegmentStats) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("base_edges", segment.base_edges)?;
    dict.set_item("delta_edges", segment.delta_edges)?;
    Ok(dict.into_any().unbind())
}
