use super::properties::{properties_from_py, property_value_from_py};
use crate::core::{
    EdgePattern, NodePattern, PropertyConstraint, PropertyOperator, QueryDirection, QueryElement,
    QueryRow, QuerySpec, QueryValue,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

pub(super) fn query_spec_from_py(spec: &Bound<'_, PyAny>) -> PyResult<QuerySpec> {
    let spec = spec
        .cast::<PyDict>()
        .map_err(|_| PyValueError::new_err("query spec must be a dict"))?;
    reject_unknown_fields(spec, "query spec", &["match", "return", "limit"])?;
    let match_value = required_item(spec, "match")?;
    let match_items = match_value
        .cast::<PyList>()
        .map_err(|_| PyValueError::new_err("query field 'match' must be a list"))?;
    if match_items.is_empty() {
        return Err(PyValueError::new_err(
            "query field 'match' must contain at least one pattern element",
        ));
    }
    if match_items.len() % 2 == 0 {
        return Err(PyValueError::new_err(
            "query field 'match' must alternate node, edge, node",
        ));
    }

    let mut elements = Vec::with_capacity(match_items.len());
    for (index, item) in match_items.iter().enumerate() {
        let item = item
            .cast::<PyDict>()
            .map_err(|_| PyValueError::new_err(format!("match element {index} must be a dict")))?;
        if index % 2 == 0 {
            elements.push(QueryElement::Node(node_pattern_from_py(item, index)?));
        } else {
            elements.push(QueryElement::Edge(edge_pattern_from_py(item, index)?));
        }
    }

    let returns = optional_string_list(spec, "return")?;
    let limit = optional_usize(spec, "limit")?;
    Ok(QuerySpec {
        elements,
        returns,
        limit,
    })
}

pub(super) fn query_rows_to_py(py: Python<'_>, rows: &[QueryRow]) -> PyResult<Py<PyAny>> {
    let result = PyList::empty(py);
    for row in rows {
        let dict = PyDict::new(py);
        for (alias, id) in row {
            dict.set_item(alias, *id)?;
        }
        result.append(dict)?;
    }
    Ok(result.into_any().unbind())
}

pub(super) fn query_schema_to_py(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let schema = PyDict::new(py);
    schema.set_item("name", "tonggraph_query_dsl_v0")?;
    schema.set_item(
        "description",
        "Structured path-pattern query DSL for one connected node-edge-node path.",
    )?;
    schema.set_item(
        "operators",
        ["eq", "ne", "lt", "lte", "gt", "gte", "in", "contains"],
    )?;
    schema.set_item("directions", ["out", "in", "both"])?;
    schema.set_item("top_level_fields", ["match", "return", "limit"])?;
    schema.set_item("return", "optional list of node or edge aliases")?;
    schema.set_item("limit", "optional non-negative integer row limit")?;

    let node = PyDict::new(py);
    node.set_item(
        "allowed_fields",
        ["node", "id", "external_id", "labels", "properties", "where"],
    )?;
    node.set_item("node", "required alias string")?;
    node.set_item("id", "optional internal node id")?;
    node.set_item("external_id", "optional external node id")?;
    node.set_item("labels", "optional list of required labels")?;
    node.set_item("properties", "optional exact-match property dict")?;
    node.set_item("where", "optional list of property filters")?;
    schema.set_item("node_pattern", node)?;

    let edge = PyDict::new(py);
    edge.set_item(
        "allowed_fields",
        ["edge", "id", "type", "direction", "properties", "where"],
    )?;
    edge.set_item("edge", "optional alias string")?;
    edge.set_item("id", "optional internal edge id")?;
    edge.set_item("type", "optional edge type")?;
    edge.set_item(
        "direction",
        "optional direction relative to surrounding nodes",
    )?;
    edge.set_item("properties", "optional exact-match property dict")?;
    edge.set_item("where", "optional list of property filters")?;
    schema.set_item("edge_pattern", edge)?;

    let filter = PyDict::new(py);
    filter.set_item("allowed_fields", ["property", "op", "value"])?;
    filter.set_item("property", "required property key")?;
    filter.set_item("op", "optional operator, defaults to eq")?;
    filter.set_item("value", "scalar value, or list when op is in")?;
    schema.set_item("where_filter", filter)?;
    Ok(schema.into_any().unbind())
}

#[pyfunction(name = "_query_dsl_schema")]
pub(crate) fn py_query_dsl_schema(py: Python<'_>) -> PyResult<Py<PyAny>> {
    query_schema_to_py(py)
}

fn node_pattern_from_py(item: &Bound<'_, PyDict>, index: usize) -> PyResult<NodePattern> {
    reject_unexpected_field(item, index, "edge", "a node")?;
    reject_unknown_fields(
        item,
        &format!("match element {index}"),
        &["node", "id", "external_id", "labels", "properties", "where"],
    )?;
    let alias = required_string(item, index, "node")?;
    Ok(NodePattern {
        alias,
        id: optional_u64(item, "id")?,
        external_id: optional_string(item, "external_id")?,
        labels: optional_string_list(item, "labels")?.unwrap_or_default(),
        properties: property_constraints_from_py(item)?,
    })
}

fn edge_pattern_from_py(item: &Bound<'_, PyDict>, index: usize) -> PyResult<EdgePattern> {
    reject_unexpected_field(item, index, "node", "an edge")?;
    reject_unknown_fields(
        item,
        &format!("match element {index}"),
        &["edge", "id", "type", "direction", "properties", "where"],
    )?;
    let direction = optional_string(item, "direction")?.unwrap_or_else(|| "out".to_string());
    let direction = QueryDirection::parse(&direction).map_err(PyValueError::new_err)?;
    Ok(EdgePattern {
        alias: optional_string(item, "edge")?,
        id: optional_u64(item, "id")?,
        edge_type: optional_string(item, "type")?,
        direction,
        properties: property_constraints_from_py(item)?,
    })
}

fn reject_unexpected_field(
    dict: &Bound<'_, PyDict>,
    index: usize,
    key: &str,
    expected: &str,
) -> PyResult<()> {
    if optional_item(dict, key)?.is_some() {
        return Err(PyValueError::new_err(format!(
            "match element {index} must be {expected} pattern, not a {key} pattern"
        )));
    }
    Ok(())
}

fn property_constraints_from_py(item: &Bound<'_, PyDict>) -> PyResult<Vec<PropertyConstraint>> {
    let properties = optional_dict(item, "properties")?;
    let mut constraints = properties_from_py(properties.as_ref())?
        .into_iter()
        .map(|(key, value)| PropertyConstraint {
            key,
            op: PropertyOperator::Eq,
            value: QueryValue::Scalar(value),
        })
        .collect::<Vec<_>>();

    let Some(where_value) = optional_item(item, "where")? else {
        return Ok(constraints);
    };
    let filters = where_value
        .cast::<PyList>()
        .map_err(|_| PyValueError::new_err("query field 'where' must be a list"))?;
    for (index, filter) in filters.iter().enumerate() {
        let filter = filter
            .cast::<PyDict>()
            .map_err(|_| PyValueError::new_err(format!("where filter {index} must be a dict")))?;
        reject_unknown_fields(
            filter,
            &format!("where filter {index}"),
            &["property", "op", "value"],
        )?;
        let key = required_string(filter, index, "property")?;
        let op = optional_string(filter, "op")?.unwrap_or_else(|| "eq".to_string());
        let op = PropertyOperator::parse(&op).map_err(PyValueError::new_err)?;
        let value = required_item(filter, "value")?;
        let value = match op {
            PropertyOperator::In => {
                let values = value.cast::<PyList>().map_err(|_| {
                    PyValueError::new_err("property filter op 'in' requires a list value")
                })?;
                let parsed = values
                    .iter()
                    .map(|value| property_value_from_py(&value))
                    .collect::<PyResult<Vec<_>>>()?;
                QueryValue::List(parsed)
            }
            _ => QueryValue::Scalar(property_value_from_py(&value)?),
        };
        constraints.push(PropertyConstraint { key, op, value });
    }

    Ok(constraints)
}

fn reject_unknown_fields(
    dict: &Bound<'_, PyDict>,
    context: &str,
    allowed: &[&str],
) -> PyResult<()> {
    for (key, _) in dict.iter() {
        let key = key
            .extract::<String>()
            .map_err(|_| PyValueError::new_err(format!("{context} keys must be strings")))?;
        if !allowed.contains(&key.as_str()) {
            return Err(PyValueError::new_err(format!(
                "{context} has unknown field {key:?}"
            )));
        }
    }
    Ok(())
}

fn required_item<'py>(dict: &Bound<'py, PyDict>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    dict.get_item(key)?
        .filter(|value| !value.is_none())
        .ok_or_else(|| PyValueError::new_err(format!("query field {key:?} is required")))
}

fn optional_item<'py>(dict: &Bound<'py, PyDict>, key: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(dict.get_item(key)?.filter(|value| !value.is_none()))
}

fn required_string(dict: &Bound<'_, PyDict>, index: usize, key: &str) -> PyResult<String> {
    required_item(dict, key)?.extract().map_err(|_| {
        PyValueError::new_err(format!(
            "match element {index} field {key:?} must be a string"
        ))
    })
}

fn optional_string(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    optional_item(dict, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!("optional query field {key:?} must be a string"))
            })
        })
        .transpose()
}

fn optional_string_list(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<Vec<String>>> {
    optional_item(dict, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!(
                    "optional query field {key:?} must be a list of strings"
                ))
            })
        })
        .transpose()
}

fn optional_u64(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<u64>> {
    optional_item(dict, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!("optional query field {key:?} must be an int"))
            })
        })
        .transpose()
}

fn optional_usize(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<usize>> {
    optional_item(dict, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!("optional query field {key:?} must be an int"))
            })
        })
        .transpose()
}

fn optional_dict<'py>(
    dict: &Bound<'py, PyDict>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    optional_item(dict, key)?
        .map(|value| {
            value.cast::<PyDict>().cloned().map_err(|_| {
                PyValueError::new_err(format!("optional query field {key:?} must be a dict"))
            })
        })
        .transpose()
}
