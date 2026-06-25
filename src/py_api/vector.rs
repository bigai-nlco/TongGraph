use crate::models::{VectorIndexDefinition, VectorSearchResult};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

pub(super) fn definitions_to_py(
    py: Python<'_>,
    definitions: Vec<VectorIndexDefinition>,
) -> PyResult<Py<PyAny>> {
    let output = PyList::empty(py);
    for definition in definitions {
        let item = PyDict::new(py);
        item.set_item("name", definition.name)?;
        item.set_item("target", definition.target)?;
        item.set_item("dimensions", definition.dimensions)?;
        item.set_item("metric", definition.metric)?;
        item.set_item("model", definition.model)?;
        item.set_item("model_version", definition.model_version)?;
        output.append(item)?;
    }
    Ok(output.into_any().unbind())
}

pub(super) fn search_results_to_py(
    py: Python<'_>,
    results: Vec<VectorSearchResult>,
) -> PyResult<Py<PyAny>> {
    let output = PyList::empty(py);
    for result in results {
        let item = PyDict::new(py);
        item.set_item("kind", result.kind)?;
        item.set_item("id", result.id)?;
        item.set_item("score", result.score)?;
        output.append(item)?;
    }
    Ok(output.into_any().unbind())
}

pub(super) fn vectors_from_py(vectors: &Bound<'_, PyDict>) -> PyResult<Vec<(u64, Vec<f64>)>> {
    let mut output = Vec::with_capacity(vectors.len());
    for (entity_id, vector) in vectors.iter() {
        let entity_id = entity_id.extract::<u64>().map_err(|_| {
            PyValueError::new_err("vector entity IDs must be non-negative integers")
        })?;
        let vector = vector
            .extract::<Vec<f64>>()
            .map_err(|_| PyValueError::new_err("vectors must be sequences of numbers"))?;
        output.push((entity_id, vector));
    }
    output.sort_by_key(|(entity_id, _)| *entity_id);
    Ok(output)
}
