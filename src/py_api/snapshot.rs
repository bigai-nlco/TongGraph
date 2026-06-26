use super::compute::{
    compute_jobs_from_py, compute_result_to_py, scores_to_py, shortest_path_to_py,
};
use super::cypher::{params_from_py, PyCypherResult};
use super::fulltext::{definitions_to_py, search_results_to_py};
use super::introspection::{graph_schema_to_py, graph_stats_to_py, query_profile_to_py};
use super::properties::{optional_property_value_from_py, properties_from_py};
use super::query::{query_rows_to_py, query_schema_to_py, query_spec_from_py};
use super::records::{PyEdge, PyEvidence, PyFactor, PyNode, PyTrace, PyVariable};
use super::to_py_value_error;
use super::vector::{
    batch_search_results_to_py as vector_batch_results_to_py,
    definitions_to_py as vector_definitions_to_py, search_results_to_py as vector_results_to_py,
};
use crate::core::{FullTextSearchOptions, GraphCore, VectorSearchOptions};
use crate::cypher;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

#[pyclass(name = "GraphSnapshot", unsendable)]
pub(crate) struct PyGraphSnapshot {
    core: GraphCore,
}

impl PyGraphSnapshot {
    pub(super) fn new(core: GraphCore) -> Self {
        Self { core }
    }
}

#[pymethods]
impl PyGraphSnapshot {
    fn fulltext_indexes(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        definitions_to_py(py, self.core.fulltext_indexes())
    }

    #[pyo3(signature = (index_name, query, mode="all", labels=None, edge_type=None, properties=None, limit=20, offset=0))]
    fn search_text(
        &self,
        py: Python<'_>,
        index_name: String,
        query: String,
        mode: &str,
        labels: Option<Vec<String>>,
        edge_type: Option<String>,
        properties: Option<&Bound<'_, PyDict>>,
        limit: usize,
        offset: usize,
    ) -> PyResult<Py<PyAny>> {
        let options = FullTextSearchOptions {
            labels: labels.unwrap_or_default(),
            edge_type,
            properties: properties_from_py(properties)?,
            limit,
            offset,
        };
        search_results_to_py(
            py,
            self.core
                .search_text(&index_name, &query, mode, &options)
                .map_err(to_py_value_error)?,
        )
    }

    fn vector_indexes(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        vector_definitions_to_py(py, self.core.vector_indexes())
    }

    fn get_vector(&self, index_name: String, entity_id: u64) -> PyResult<Vec<f32>> {
        self.core
            .get_vector(&index_name, entity_id)
            .map_err(to_py_value_error)?
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "vector for index {index_name:?} entity {entity_id} not found"
                ))
            })
    }

    #[pyo3(signature = (index_name, query_vector, labels=None, edge_type=None, properties=None, min_score=None, limit=20, offset=0))]
    fn search_vector(
        &self,
        py: Python<'_>,
        index_name: String,
        query_vector: Vec<f64>,
        labels: Option<Vec<String>>,
        edge_type: Option<String>,
        properties: Option<&Bound<'_, PyDict>>,
        min_score: Option<f64>,
        limit: usize,
        offset: usize,
    ) -> PyResult<Py<PyAny>> {
        let options = VectorSearchOptions {
            labels: labels.unwrap_or_default(),
            edge_type,
            properties: properties_from_py(properties)?,
            min_score,
            limit,
            offset,
        };
        vector_results_to_py(
            py,
            self.core
                .search_vector(&index_name, &query_vector, &options)
                .map_err(to_py_value_error)?,
        )
    }

    #[pyo3(signature = (index_name, query_vectors, labels=None, edge_type=None, properties=None, min_score=None, limit=20, offset=0))]
    fn search_vectors(
        &self,
        py: Python<'_>,
        index_name: String,
        query_vectors: Vec<Vec<f64>>,
        labels: Option<Vec<String>>,
        edge_type: Option<String>,
        properties: Option<&Bound<'_, PyDict>>,
        min_score: Option<f64>,
        limit: usize,
        offset: usize,
    ) -> PyResult<Py<PyAny>> {
        let options = VectorSearchOptions {
            labels: labels.unwrap_or_default(),
            edge_type,
            properties: properties_from_py(properties)?,
            min_score,
            limit,
            offset,
        };
        vector_batch_results_to_py(
            py,
            self.core
                .search_vectors(&index_name, &query_vectors, &options)
                .map_err(to_py_value_error)?,
        )
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

    fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        graph_schema_to_py(py, self.core.schema_summary())
    }

    fn stats(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        graph_stats_to_py(py, self.core.stats_summary())
    }

    fn node_ids(&self) -> Vec<u64> {
        self.core.node_ids()
    }

    fn edge_ids(&self) -> Vec<u64> {
        self.core.edge_ids()
    }

    fn nodes(&self) -> Vec<PyNode> {
        self.core.nodes().into_iter().map(PyNode::from).collect()
    }

    fn edges(&self) -> Vec<PyEdge> {
        self.core.edges().into_iter().map(PyEdge::from).collect()
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

    #[pyo3(signature = (spec, profile=false))]
    fn query(&self, py: Python<'_>, spec: &Bound<'_, PyAny>, profile: bool) -> PyResult<Py<PyAny>> {
        let spec = query_spec_from_py(spec)?;
        if profile {
            let result = self
                .core
                .query_with_profile(&spec)
                .map_err(to_py_value_error)?;
            let dict = PyDict::new(py);
            dict.set_item("rows", query_rows_to_py(py, &result.rows)?)?;
            dict.set_item("profile", query_profile_to_py(py, &result.profile)?)?;
            Ok(dict.into_any().unbind())
        } else {
            let rows = self.core.query(&spec).map_err(to_py_value_error)?;
            query_rows_to_py(py, &rows)
        }
    }

    fn query_schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        query_schema_to_py(py)
    }

    #[pyo3(signature = (query, parameters=None, profile=false))]
    fn cypher(
        &self,
        query: &str,
        parameters: Option<&Bound<'_, PyDict>>,
        profile: bool,
    ) -> PyResult<PyCypherResult> {
        let params = params_from_py(parameters)?;
        if profile {
            cypher::execute_snapshot_profiled(&self.core, query, &params)
                .map(PyCypherResult::from)
                .map_err(to_py_value_error)
        } else {
            cypher::execute_snapshot(&self.core, query, &params)
                .map(PyCypherResult::from)
                .map_err(to_py_value_error)
        }
    }
}
