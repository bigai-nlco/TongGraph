use crate::models::{FullTextIndexDefinition, FullTextSearchResult};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

pub(super) fn definitions_to_py(
    py: Python<'_>,
    definitions: Vec<FullTextIndexDefinition>,
) -> PyResult<Py<PyAny>> {
    let result = PyList::empty(py);
    for definition in definitions {
        let item = PyDict::new(py);
        item.set_item("name", definition.name)?;
        item.set_item("target", definition.target)?;
        item.set_item("properties", definition.properties)?;
        item.set_item("tokenizer", definition.tokenizer)?;
        result.append(item)?;
    }
    Ok(result.into_any().unbind())
}

pub(super) fn search_results_to_py(
    py: Python<'_>,
    results: Vec<FullTextSearchResult>,
) -> PyResult<Py<PyAny>> {
    let output = PyList::empty(py);
    for result in results {
        let item = PyDict::new(py);
        item.set_item("kind", result.kind)?;
        item.set_item("id", result.id)?;
        item.set_item("score", result.score)?;
        item.set_item("matched_fields", result.matched_fields)?;
        output.append(item)?;
    }
    Ok(output.into_any().unbind())
}
