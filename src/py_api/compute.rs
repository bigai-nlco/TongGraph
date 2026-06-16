use super::snapshot::PyGraphSnapshot;
use crate::core::{ComputeJob, ComputeResult, ShortestPath};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use std::collections::BTreeMap;

pub(super) fn shortest_path_to_py(
    py: Python<'_>,
    path: Option<ShortestPath>,
) -> PyResult<Py<PyAny>> {
    match path {
        Some(path) => {
            let dict = PyDict::new(py);
            dict.set_item("nodes", path.nodes)?;
            dict.set_item("distance", path.distance)?;
            Ok(dict.into_any().unbind())
        }
        None => Ok(py.None()),
    }
}

pub(super) fn scores_to_py(py: Python<'_>, scores: BTreeMap<u64, f64>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (node_id, score) in scores {
        dict.set_item(node_id, score)?;
    }
    Ok(dict.into_any().unbind())
}

pub(super) fn compute_result_to_py(py: Python<'_>, result: ComputeResult) -> PyResult<Py<PyAny>> {
    match result {
        ComputeResult::Nodes(nodes) => list_to_py(py, &nodes),
        ComputeResult::ShortestPath(path) => shortest_path_to_py(py, path),
        ComputeResult::Components(components) => components_to_py(py, &components),
        ComputeResult::Scores(scores) => scores_to_py(py, scores),
        ComputeResult::Snapshot(core) => {
            let snapshot = Py::new(py, PyGraphSnapshot::new(core))?;
            Ok(snapshot.into_any())
        }
    }
}

pub(super) fn compute_jobs_from_py(jobs: &Bound<'_, PyAny>) -> PyResult<Vec<ComputeJob>> {
    let jobs = jobs
        .cast::<PyList>()
        .map_err(|_| PyValueError::new_err("compute_batch jobs must be a list"))?;
    let mut parsed = Vec::with_capacity(jobs.len());

    for (index, item) in jobs.iter().enumerate() {
        let job = item
            .cast::<PyDict>()
            .map_err(|_| PyValueError::new_err(format!("job {index} must be a dict")))?;
        let op = required_string(job, index, "op")?;
        let job = match op.as_str() {
            "bfs" => ComputeJob::Bfs {
                start: required_u64(job, index, "start")?,
                direction: optional_string(job, "direction")?.unwrap_or_else(|| "out".to_string()),
                edge_type: optional_string(job, "edge_type")?,
                max_depth: optional_usize(job, "max_depth")?,
            },
            "shortest_path" => ComputeJob::ShortestPath {
                start: required_u64(job, index, "start")?,
                target: required_u64(job, index, "target")?,
                direction: optional_string(job, "direction")?.unwrap_or_else(|| "out".to_string()),
                edge_type: optional_string(job, "edge_type")?,
                weight_property: optional_string(job, "weight_property")?,
            },
            "connected_components" => ComputeJob::ConnectedComponents {
                edge_type: optional_string(job, "edge_type")?,
            },
            "pagerank" => ComputeJob::PageRank {
                iterations: optional_usize(job, "iterations")?.unwrap_or(20),
                damping: optional_f64(job, "damping")?.unwrap_or(0.85),
                tolerance: optional_f64(job, "tolerance")?,
                edge_type: optional_string(job, "edge_type")?,
            },
            "random_walk" => ComputeJob::RandomWalk {
                start: required_u64(job, index, "start")?,
                steps: required_usize(job, index, "steps")?,
                direction: optional_string(job, "direction")?.unwrap_or_else(|| "out".to_string()),
                edge_type: optional_string(job, "edge_type")?,
                seed: optional_u64(job, "seed")?,
            },
            "subgraph" => ComputeJob::Subgraph {
                nodes: required_u64_list(job, index, "nodes")?,
                edge_type: optional_string(job, "edge_type")?,
            },
            other => {
                return Err(PyValueError::new_err(format!(
                    "job {index} has unknown op {other:?}"
                )));
            }
        };
        parsed.push(job);
    }

    Ok(parsed)
}

fn list_to_py(py: Python<'_>, values: &[u64]) -> PyResult<Py<PyAny>> {
    Ok(PyList::new(py, values)?.into_any().unbind())
}

fn components_to_py(py: Python<'_>, components: &[Vec<u64>]) -> PyResult<Py<PyAny>> {
    let outer = PyList::empty(py);
    for component in components {
        outer.append(PyList::new(py, component)?)?;
    }
    Ok(outer.into_any().unbind())
}

fn required_item<'py>(
    job: &Bound<'py, PyDict>,
    index: usize,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    job.get_item(key)?
        .filter(|value| !value.is_none())
        .ok_or_else(|| PyValueError::new_err(format!("job {index} missing {key:?}")))
}

fn optional_item<'py>(job: &Bound<'py, PyDict>, key: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(job.get_item(key)?.filter(|value| !value.is_none()))
}

fn required_string(job: &Bound<'_, PyDict>, index: usize, key: &str) -> PyResult<String> {
    required_item(job, index, key)?
        .extract()
        .map_err(|_| PyValueError::new_err(format!("job {index} field {key:?} must be a string")))
}

fn optional_string(job: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    optional_item(job, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!("optional field {key:?} must be a string"))
            })
        })
        .transpose()
}

fn required_u64(job: &Bound<'_, PyDict>, index: usize, key: &str) -> PyResult<u64> {
    required_item(job, index, key)?
        .extract()
        .map_err(|_| PyValueError::new_err(format!("job {index} field {key:?} must be an int")))
}

fn optional_u64(job: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<u64>> {
    optional_item(job, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!("optional field {key:?} must be an int"))
            })
        })
        .transpose()
}

fn required_usize(job: &Bound<'_, PyDict>, index: usize, key: &str) -> PyResult<usize> {
    required_item(job, index, key)?
        .extract()
        .map_err(|_| PyValueError::new_err(format!("job {index} field {key:?} must be an int")))
}

fn optional_usize(job: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<usize>> {
    optional_item(job, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!("optional field {key:?} must be an int"))
            })
        })
        .transpose()
}

fn optional_f64(job: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<f64>> {
    optional_item(job, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!("optional field {key:?} must be a number"))
            })
        })
        .transpose()
}

fn required_u64_list(job: &Bound<'_, PyDict>, index: usize, key: &str) -> PyResult<Vec<u64>> {
    required_item(job, index, key)?.extract().map_err(|_| {
        PyValueError::new_err(format!("job {index} field {key:?} must be a list of ints"))
    })
}
