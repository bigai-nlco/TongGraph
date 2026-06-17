use crate::core::{ActiveSubgraph, BeliefPropagationResult};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use std::collections::{BTreeMap, HashMap};

pub(super) fn evidence_from_py(
    evidence: Option<&Bound<'_, PyDict>>,
) -> PyResult<HashMap<u64, String>> {
    let mut result = HashMap::new();
    let Some(evidence) = evidence else {
        return Ok(result);
    };
    for (key, value) in evidence.iter() {
        let variable_id = key
            .extract::<u64>()
            .map_err(|_| PyValueError::new_err("evidence keys must be variable ids"))?;
        let state = value
            .extract::<String>()
            .map_err(|_| PyValueError::new_err("evidence values must be state strings"))?;
        result.insert(variable_id, state);
    }
    Ok(result)
}

pub(super) fn active_subgraph_to_py(
    py: Python<'_>,
    active: &ActiveSubgraph,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("variables", active.variables.clone())?;
    dict.set_item("factors", active.factors.clone())?;
    dict.set_item("graph_nodes", active.graph_nodes.clone())?;
    dict.set_item("boundary_variables", active.boundary_variables.clone())?;
    dict.set_item("truncated", active.truncated)?;
    Ok(dict.into_any().unbind())
}

pub(super) fn belief_result_to_py(
    py: Python<'_>,
    result: BeliefPropagationResult,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("beliefs", belief_map_to_py(py, result.beliefs)?)?;
    dict.set_item("active", active_subgraph_to_py(py, &result.active)?)?;
    dict.set_item("schedule", result.schedule)?;
    dict.set_item("iterations", result.iterations)?;
    dict.set_item("messages_updated", result.messages_updated)?;
    dict.set_item("converged", result.converged)?;
    dict.set_item("max_residual", result.max_residual)?;
    dict.set_item("trace_id", result.trace_id)?;
    Ok(dict.into_any().unbind())
}

pub(super) fn belief_map_to_py(
    py: Python<'_>,
    beliefs: BTreeMap<u64, BTreeMap<String, f64>>,
) -> PyResult<Py<PyAny>> {
    let outer = PyDict::new(py);
    for (variable_id, distribution) in beliefs {
        let inner = PyDict::new(py);
        for (state, probability) in distribution {
            inner.set_item(state, probability)?;
        }
        outer.set_item(variable_id, inner)?;
    }
    Ok(outer.into_any().unbind())
}

pub(super) fn distribution_to_py(
    py: Python<'_>,
    distribution: BTreeMap<String, f64>,
) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (state, probability) in distribution {
        dict.set_item(state, probability)?;
    }
    Ok(dict.into_any().unbind())
}
