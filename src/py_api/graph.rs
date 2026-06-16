use super::compute::{
    compute_jobs_from_py, compute_result_to_py, scores_to_py, shortest_path_to_py,
};
use super::properties::{optional_property_value_from_py, properties_from_py};
use super::records::{PyEdge, PyEvidence, PyFactor, PyNode, PyTrace, PyVariable};
use super::snapshot::PyGraphSnapshot;
use super::to_py_value_error;
use crate::core::GraphCore;
use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use std::collections::HashMap;

#[pyclass(name = "Graph", unsendable)]
pub(crate) struct PyGraph {
    core: GraphCore,
}

#[pymethods]
impl PyGraph {
    #[new]
    #[pyo3(signature = (path=None))]
    fn new(path: Option<String>) -> PyResult<Self> {
        let core = match path {
            Some(path) => GraphCore::open(&path).map_err(PyRuntimeError::new_err)?,
            None => GraphCore::new(),
        };
        Ok(Self { core })
    }

    #[staticmethod]
    fn open(path: String) -> PyResult<Self> {
        Self::new(Some(path))
    }

    #[pyo3(signature = (external_id=None, labels=None, properties=None))]
    fn add_node(
        &mut self,
        external_id: Option<String>,
        labels: Option<Vec<String>>,
        properties: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<u64> {
        self.core
            .add_node(
                external_id,
                labels.unwrap_or_default(),
                properties_from_py(properties)?,
            )
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (source, target, edge_type, properties=None))]
    fn add_edge(
        &mut self,
        source: u64,
        target: u64,
        edge_type: String,
        properties: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<u64> {
        self.core
            .add_edge(source, target, edge_type, properties_from_py(properties)?)
            .map_err(to_py_value_error)
    }

    fn node_count(&self) -> usize {
        self.core.node_count()
    }

    fn edge_count(&self) -> usize {
        self.core.edge_count()
    }

    fn variable_count(&self) -> usize {
        self.core.variable_count()
    }

    fn factor_count(&self) -> usize {
        self.core.factor_count()
    }

    fn evidence_count(&self) -> usize {
        self.core.evidence_count()
    }

    fn trace_count(&self) -> usize {
        self.core.trace_count()
    }

    fn get_node(&self, node_id: u64) -> PyResult<PyNode> {
        self.core
            .get_node(node_id)
            .map(PyNode::from)
            .ok_or_else(|| PyKeyError::new_err(format!("node {node_id} not found")))
    }

    fn get_edge(&self, edge_id: u64) -> PyResult<PyEdge> {
        self.core
            .get_edge(edge_id)
            .map(PyEdge::from)
            .ok_or_else(|| PyKeyError::new_err(format!("edge {edge_id} not found")))
    }

    fn get_variable(&self, variable_id: u64) -> PyResult<PyVariable> {
        self.core
            .get_variable(variable_id)
            .map(PyVariable::from)
            .ok_or_else(|| PyKeyError::new_err(format!("variable {variable_id} not found")))
    }

    fn get_factor(&self, factor_id: u64) -> PyResult<PyFactor> {
        self.core
            .get_factor(factor_id)
            .map(PyFactor::from)
            .ok_or_else(|| PyKeyError::new_err(format!("factor {factor_id} not found")))
    }

    fn get_evidence(&self, evidence_id: u64) -> PyResult<PyEvidence> {
        self.core
            .get_evidence(evidence_id)
            .map(PyEvidence::from)
            .ok_or_else(|| PyKeyError::new_err(format!("evidence {evidence_id} not found")))
    }

    fn get_trace(&self, trace_id: u64) -> PyResult<PyTrace> {
        self.core
            .get_trace(trace_id)
            .map(PyTrace::from)
            .ok_or_else(|| PyKeyError::new_err(format!("trace {trace_id} not found")))
    }

    fn get_node_id(&self, external_id: String) -> Option<u64> {
        self.core.get_node_id(&external_id)
    }

    fn nodes_with_label(&self, label: String) -> Vec<u64> {
        self.core.nodes_with_label(&label)
    }

    fn edges_by_type(&self, edge_type: String) -> Vec<u64> {
        self.core.edges_by_type(&edge_type)
    }

    #[pyo3(signature = (key, value=None))]
    fn nodes_with_property(
        &self,
        key: String,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<u64>> {
        let value = optional_property_value_from_py(value)?;
        Ok(self.core.nodes_with_property(&key, value.as_ref()))
    }

    #[pyo3(signature = (key, value=None))]
    fn edges_with_property(
        &self,
        key: String,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<u64>> {
        let value = optional_property_value_from_py(value)?;
        Ok(self.core.edges_with_property(&key, value.as_ref()))
    }

    #[pyo3(signature = (node_id, direction="out", edge_type=None))]
    fn neighbors(
        &self,
        node_id: u64,
        direction: &str,
        edge_type: Option<String>,
    ) -> PyResult<Vec<u64>> {
        self.core
            .neighbors(node_id, direction, edge_type.as_deref())
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (start, hops, direction="out", edge_type=None))]
    fn k_hop(
        &self,
        start: u64,
        hops: usize,
        direction: &str,
        edge_type: Option<String>,
    ) -> PyResult<Vec<u64>> {
        self.core
            .k_hop(start, hops, direction, edge_type.as_deref())
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (starts, steps, direction="out", edge_type=None))]
    fn frontier(
        &self,
        starts: Vec<u64>,
        steps: usize,
        direction: &str,
        edge_type: Option<String>,
    ) -> PyResult<Vec<u64>> {
        self.core
            .frontier(&starts, steps, direction, edge_type.as_deref())
            .map_err(to_py_value_error)
    }

    fn compact(&mut self) -> PyResult<()> {
        self.core.compact_segments().map_err(to_py_value_error)
    }

    fn snapshot(&self) -> PyGraphSnapshot {
        PyGraphSnapshot::new(self.core.snapshot())
    }

    #[pyo3(signature = (start, direction="out", edge_type=None, max_depth=None))]
    fn bfs(
        &self,
        start: u64,
        direction: &str,
        edge_type: Option<String>,
        max_depth: Option<usize>,
    ) -> PyResult<Vec<u64>> {
        self.core
            .bfs(start, direction, edge_type.as_deref(), max_depth)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (start, target, direction="out", edge_type=None, weight_property=None))]
    fn shortest_path(
        &self,
        py: Python<'_>,
        start: u64,
        target: u64,
        direction: &str,
        edge_type: Option<String>,
        weight_property: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let path = self
            .core
            .shortest_path(
                start,
                target,
                direction,
                edge_type.as_deref(),
                weight_property.as_deref(),
            )
            .map_err(to_py_value_error)?;
        shortest_path_to_py(py, path)
    }

    #[pyo3(signature = (edge_type=None))]
    fn connected_components(&self, edge_type: Option<String>) -> PyResult<Vec<Vec<u64>>> {
        self.core
            .connected_components(edge_type.as_deref())
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (iterations=20, damping=0.85, tolerance=None, edge_type=None))]
    fn pagerank(
        &self,
        py: Python<'_>,
        iterations: usize,
        damping: f64,
        tolerance: Option<f64>,
        edge_type: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let scores = self
            .core
            .pagerank(iterations, damping, tolerance, edge_type.as_deref())
            .map_err(to_py_value_error)?;
        scores_to_py(py, scores)
    }

    #[pyo3(signature = (start, steps, direction="out", edge_type=None, seed=None))]
    fn random_walk(
        &self,
        start: u64,
        steps: usize,
        direction: &str,
        edge_type: Option<String>,
        seed: Option<u64>,
    ) -> PyResult<Vec<u64>> {
        self.core
            .random_walk(start, steps, direction, edge_type.as_deref(), seed)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (nodes, edge_type=None))]
    fn subgraph(&self, nodes: Vec<u64>, edge_type: Option<String>) -> PyResult<PyGraphSnapshot> {
        self.core
            .subgraph(&nodes, edge_type.as_deref())
            .map(PyGraphSnapshot::new)
            .map_err(to_py_value_error)
    }

    fn compute_batch(&self, py: Python<'_>, jobs: &Bound<'_, PyAny>) -> PyResult<Vec<Py<PyAny>>> {
        let jobs = compute_jobs_from_py(jobs)?;
        self.core
            .compute_batch(&jobs)
            .map_err(to_py_value_error)?
            .into_iter()
            .map(|result| compute_result_to_py(py, result))
            .collect()
    }

    #[pyo3(signature = (seeds, steps, edge_property="probability", damping=1.0))]
    fn propagate(
        &self,
        seeds: HashMap<u64, f64>,
        steps: usize,
        edge_property: &str,
        damping: f64,
    ) -> PyResult<HashMap<u64, f64>> {
        self.core
            .propagate(&seeds, steps, edge_property, damping)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (domain, owner_id=None, prior=None, posterior=None))]
    fn add_variable(
        &mut self,
        domain: String,
        owner_id: Option<u64>,
        prior: Option<&Bound<'_, PyDict>>,
        posterior: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<u64> {
        self.core
            .add_variable(
                owner_id,
                domain,
                properties_from_py(prior)?,
                properties_from_py(posterior)?,
            )
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (input_variables, output_variables, function, parameters=None))]
    fn add_factor(
        &mut self,
        input_variables: Vec<u64>,
        output_variables: Vec<u64>,
        function: String,
        parameters: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<u64> {
        self.core
            .add_factor(
                input_variables,
                output_variables,
                function,
                properties_from_py(parameters)?,
            )
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (variable_id, payload=None))]
    fn add_evidence(
        &mut self,
        variable_id: u64,
        payload: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<u64> {
        self.core
            .add_evidence(variable_id, properties_from_py(payload)?)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (payload=None))]
    fn add_trace(&mut self, payload: Option<&Bound<'_, PyDict>>) -> PyResult<u64> {
        self.core
            .add_trace(properties_from_py(payload)?)
            .map_err(to_py_value_error)
    }
}
