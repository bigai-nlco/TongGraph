use super::compute::{
    compute_jobs_from_py, compute_result_to_py, scores_to_py, shortest_path_to_py,
};
use super::cypher::{params_from_py, PyCypherResult, PyGraphTransaction};
use super::inference::{
    active_subgraph_to_py, belief_result_to_py, distribution_to_py, evidence_from_py,
};
use super::properties::{optional_property_value_from_py, properties_from_py};
use super::query::{query_rows_to_py, query_schema_to_py, query_spec_from_py};
use super::records::{PyEdge, PyEvidence, PyFactor, PyNode, PyTrace, PyVariable};
use super::snapshot::PyGraphSnapshot;
use super::to_py_value_error;
use crate::core::GraphCore;
use crate::cypher;
use crate::models::{NewEdgeRecord, NewNodeRecord};
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[pyclass(name = "Graph", unsendable)]
pub(crate) struct PyGraph {
    pub(super) core: Rc<RefCell<GraphCore>>,
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
        Ok(Self {
            core: Rc::new(RefCell::new(core)),
        })
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
            .borrow_mut()
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
            .borrow_mut()
            .add_edge(source, target, edge_type, properties_from_py(properties)?)
            .map_err(to_py_value_error)
    }

    fn node_count(&self) -> usize {
        self.core.borrow().node_count()
    }

    fn edge_count(&self) -> usize {
        self.core.borrow().edge_count()
    }

    fn variable_count(&self) -> usize {
        self.core.borrow().variable_count()
    }

    fn factor_count(&self) -> usize {
        self.core.borrow().factor_count()
    }

    fn evidence_count(&self) -> usize {
        self.core.borrow().evidence_count()
    }

    fn trace_count(&self) -> usize {
        self.core.borrow().trace_count()
    }

    fn node_ids(&self) -> Vec<u64> {
        self.core.borrow().node_ids()
    }

    fn edge_ids(&self) -> Vec<u64> {
        self.core.borrow().edge_ids()
    }

    fn nodes(&self) -> Vec<PyNode> {
        self.core
            .borrow()
            .nodes()
            .into_iter()
            .map(PyNode::from)
            .collect()
    }

    fn edges(&self) -> Vec<PyEdge> {
        self.core
            .borrow()
            .edges()
            .into_iter()
            .map(PyEdge::from)
            .collect()
    }

    fn get_node(&self, node_id: u64) -> PyResult<PyNode> {
        self.core
            .borrow()
            .get_node(node_id)
            .map(PyNode::from)
            .ok_or_else(|| PyKeyError::new_err(format!("node {node_id} not found")))
    }

    fn get_edge(&self, edge_id: u64) -> PyResult<PyEdge> {
        self.core
            .borrow()
            .get_edge(edge_id)
            .map(PyEdge::from)
            .ok_or_else(|| PyKeyError::new_err(format!("edge {edge_id} not found")))
    }

    fn get_variable(&self, variable_id: u64) -> PyResult<PyVariable> {
        self.core
            .borrow()
            .get_variable(variable_id)
            .map(PyVariable::from)
            .ok_or_else(|| PyKeyError::new_err(format!("variable {variable_id} not found")))
    }

    fn get_factor(&self, factor_id: u64) -> PyResult<PyFactor> {
        self.core
            .borrow()
            .get_factor(factor_id)
            .map(PyFactor::from)
            .ok_or_else(|| PyKeyError::new_err(format!("factor {factor_id} not found")))
    }

    fn get_evidence(&self, evidence_id: u64) -> PyResult<PyEvidence> {
        self.core
            .borrow()
            .get_evidence(evidence_id)
            .map(PyEvidence::from)
            .ok_or_else(|| PyKeyError::new_err(format!("evidence {evidence_id} not found")))
    }

    fn get_trace(&self, trace_id: u64) -> PyResult<PyTrace> {
        self.core
            .borrow()
            .get_trace(trace_id)
            .map(PyTrace::from)
            .ok_or_else(|| PyKeyError::new_err(format!("trace {trace_id} not found")))
    }

    fn get_node_id(&self, external_id: String) -> Option<u64> {
        self.core.borrow().get_node_id(&external_id)
    }

    fn nodes_with_label(&self, label: String) -> Vec<u64> {
        self.core.borrow().nodes_with_label(&label)
    }

    fn edges_by_type(&self, edge_type: String) -> Vec<u64> {
        self.core.borrow().edges_by_type(&edge_type)
    }

    #[pyo3(signature = (key, value=None))]
    fn nodes_with_property(
        &self,
        key: String,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<u64>> {
        let value = optional_property_value_from_py(value)?;
        Ok(self.core.borrow().nodes_with_property(&key, value.as_ref()))
    }

    #[pyo3(signature = (key, value=None))]
    fn edges_with_property(
        &self,
        key: String,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<u64>> {
        let value = optional_property_value_from_py(value)?;
        Ok(self.core.borrow().edges_with_property(&key, value.as_ref()))
    }

    #[pyo3(signature = (node_id, direction="out", edge_type=None))]
    fn neighbors(
        &self,
        node_id: u64,
        direction: &str,
        edge_type: Option<String>,
    ) -> PyResult<Vec<u64>> {
        self.core
            .borrow()
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
            .borrow()
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
            .borrow()
            .frontier(&starts, steps, direction, edge_type.as_deref())
            .map_err(to_py_value_error)
    }

    fn compact(&mut self) -> PyResult<()> {
        self.core
            .borrow_mut()
            .compact_segments()
            .map_err(to_py_value_error)
    }

    fn refresh(&mut self) -> PyResult<()> {
        self.core.borrow_mut().refresh().map_err(to_py_value_error)
    }

    fn snapshot(&self) -> PyGraphSnapshot {
        PyGraphSnapshot::new(self.core.borrow().snapshot())
    }

    fn add_nodes(&mut self, records: &Bound<'_, PyAny>) -> PyResult<Vec<u64>> {
        let records = new_node_records_from_py(records)?;
        self.core
            .borrow_mut()
            .add_nodes(records)
            .map_err(to_py_value_error)
    }

    fn add_edges(&mut self, records: &Bound<'_, PyAny>) -> PyResult<Vec<u64>> {
        let records = new_edge_records_from_py(records)?;
        self.core
            .borrow_mut()
            .add_edges(records)
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
            .borrow()
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
            .borrow()
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
            .borrow()
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
            .borrow()
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
            .borrow()
            .random_walk(start, steps, direction, edge_type.as_deref(), seed)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (nodes, edge_type=None))]
    fn subgraph(&self, nodes: Vec<u64>, edge_type: Option<String>) -> PyResult<PyGraphSnapshot> {
        self.core
            .borrow()
            .subgraph(&nodes, edge_type.as_deref())
            .map(PyGraphSnapshot::new)
            .map_err(to_py_value_error)
    }

    fn compute_batch(&self, py: Python<'_>, jobs: &Bound<'_, PyAny>) -> PyResult<Vec<Py<PyAny>>> {
        let jobs = compute_jobs_from_py(jobs)?;
        self.core
            .borrow()
            .compute_batch(&jobs)
            .map_err(to_py_value_error)?
            .into_iter()
            .map(|result| compute_result_to_py(py, result))
            .collect()
    }

    fn query(&self, py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let spec = query_spec_from_py(spec)?;
        let rows = self.core.borrow().query(&spec).map_err(to_py_value_error)?;
        query_rows_to_py(py, &rows)
    }

    fn query_schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        query_schema_to_py(py)
    }

    #[pyo3(signature = (query, parameters=None))]
    fn cypher(
        &mut self,
        query: &str,
        parameters: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyCypherResult> {
        let params = params_from_py(parameters)?;
        cypher::execute_autocommit(&mut self.core.borrow_mut(), query, &params)
            .map(PyCypherResult::from)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (write=true))]
    fn transaction(&self, write: bool) -> PyGraphTransaction {
        PyGraphTransaction::new(Rc::clone(&self.core), write)
    }

    #[pyo3(signature = (seeds, steps, edge_property="probability", damping=1.0, edge_type=None))]
    fn propagate(
        &self,
        seeds: HashMap<u64, f64>,
        steps: usize,
        edge_property: &str,
        damping: f64,
        edge_type: Option<String>,
    ) -> PyResult<HashMap<u64, f64>> {
        self.core
            .borrow()
            .propagate(&seeds, steps, edge_type.as_deref(), edge_property, damping)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (domain, owner_id=None, prior=None, posterior=None, states=None))]
    fn add_variable(
        &mut self,
        domain: String,
        owner_id: Option<u64>,
        prior: Option<&Bound<'_, PyDict>>,
        posterior: Option<&Bound<'_, PyDict>>,
        states: Option<Vec<String>>,
    ) -> PyResult<u64> {
        self.core
            .borrow_mut()
            .add_variable(
                owner_id,
                domain,
                states,
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
            .borrow_mut()
            .add_factor(
                input_variables,
                output_variables,
                function,
                properties_from_py(parameters)?,
            )
            .map_err(to_py_value_error)
    }

    fn add_factor_table(&mut self, variables: Vec<u64>, values: Vec<f64>) -> PyResult<u64> {
        self.core
            .borrow_mut()
            .add_factor_table(variables, values)
            .map_err(to_py_value_error)
    }

    fn add_cpd(
        &mut self,
        variable_id: u64,
        parent_variables: Vec<u64>,
        values: Vec<f64>,
    ) -> PyResult<u64> {
        self.core
            .borrow_mut()
            .add_cpd(variable_id, parent_variables, values)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (variable_id, payload=None))]
    fn add_evidence(
        &mut self,
        variable_id: u64,
        payload: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<u64> {
        self.core
            .borrow_mut()
            .add_evidence(variable_id, properties_from_py(payload)?)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (payload=None))]
    fn add_trace(&mut self, payload: Option<&Bound<'_, PyDict>>) -> PyResult<u64> {
        self.core
            .borrow_mut()
            .add_trace(properties_from_py(payload)?)
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (query_variables, evidence=None, radius=2, max_nodes=10000, max_factors=50000))]
    fn compile_active_subgraph(
        &self,
        py: Python<'_>,
        query_variables: Vec<u64>,
        evidence: Option<&Bound<'_, PyDict>>,
        radius: usize,
        max_nodes: usize,
        max_factors: usize,
    ) -> PyResult<Py<PyAny>> {
        let evidence = evidence_from_py(evidence)?;
        let active = self
            .core
            .borrow()
            .compile_active_subgraph(&query_variables, &evidence, radius, max_nodes, max_factors)
            .map_err(to_py_value_error)?;
        active_subgraph_to_py(py, &active)
    }

    #[pyo3(signature = (query_variables=None, evidence=None, radius=2, max_iters=1000, tolerance=1e-6, damping=0.2, persist=false))]
    fn belief_propagation(
        &mut self,
        py: Python<'_>,
        query_variables: Option<Vec<u64>>,
        evidence: Option<&Bound<'_, PyDict>>,
        radius: usize,
        max_iters: usize,
        tolerance: f64,
        damping: f64,
        persist: bool,
    ) -> PyResult<Py<PyAny>> {
        let evidence = evidence_from_py(evidence)?;
        let result = self
            .core
            .borrow_mut()
            .belief_propagation(
                query_variables.as_deref(),
                &evidence,
                radius,
                max_iters,
                tolerance,
                damping,
                persist,
            )
            .map_err(to_py_value_error)?;
        belief_result_to_py(py, result)
    }

    fn posterior(&self, py: Python<'_>, variable_id: u64) -> PyResult<Py<PyAny>> {
        distribution_to_py(
            py,
            self.core
                .borrow()
                .posterior(variable_id)
                .map_err(to_py_value_error)?,
        )
    }

    #[pyo3(signature = (seeds, radius=2, query_nodes=None, edge_type=None, edge_property="probability", damping=1.0))]
    fn local_propagate(
        &self,
        seeds: HashMap<u64, f64>,
        radius: usize,
        query_nodes: Option<Vec<u64>>,
        edge_type: Option<String>,
        edge_property: &str,
        damping: f64,
    ) -> PyResult<HashMap<u64, f64>> {
        self.core
            .borrow()
            .local_propagate(
                &seeds,
                radius,
                query_nodes.as_deref(),
                edge_type.as_deref(),
                edge_property,
                damping,
            )
            .map_err(to_py_value_error)
    }
}

fn new_node_records_from_py(records: &Bound<'_, PyAny>) -> PyResult<Vec<NewNodeRecord>> {
    let records = records
        .cast::<PyList>()
        .map_err(|_| PyValueError::new_err("add_nodes records must be a list"))?;
    let mut parsed = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        let record = record
            .cast::<PyDict>()
            .map_err(|_| PyValueError::new_err(format!("node record {index} must be a dict")))?;
        reject_unknown_record_fields(
            record,
            &format!("node record {index}"),
            &["external_id", "labels", "properties"],
        )?;
        let properties = optional_dict(record, "properties")?;
        parsed.push(NewNodeRecord {
            external_id: optional_string(record, "external_id")?,
            labels: optional_string_list(record, "labels")?.unwrap_or_default(),
            properties: properties_from_py(properties.as_ref())?,
        });
    }
    Ok(parsed)
}

fn new_edge_records_from_py(records: &Bound<'_, PyAny>) -> PyResult<Vec<NewEdgeRecord>> {
    let records = records
        .cast::<PyList>()
        .map_err(|_| PyValueError::new_err("add_edges records must be a list"))?;
    let mut parsed = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        let record = record
            .cast::<PyDict>()
            .map_err(|_| PyValueError::new_err(format!("edge record {index} must be a dict")))?;
        reject_unknown_record_fields(
            record,
            &format!("edge record {index}"),
            &["source", "target", "edge_type", "properties"],
        )?;
        let properties = optional_dict(record, "properties")?;
        parsed.push(NewEdgeRecord {
            source: required_u64(record, index, "source")?,
            target: required_u64(record, index, "target")?,
            edge_type: required_string(record, index, "edge_type")?,
            properties: properties_from_py(properties.as_ref())?,
        });
    }
    Ok(parsed)
}

fn reject_unknown_record_fields(
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

fn required_item<'py>(
    dict: &Bound<'py, PyDict>,
    index: usize,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    dict.get_item(key)?
        .filter(|value| !value.is_none())
        .ok_or_else(|| PyValueError::new_err(format!("record {index} missing {key:?}")))
}

fn optional_item<'py>(dict: &Bound<'py, PyDict>, key: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(dict.get_item(key)?.filter(|value| !value.is_none()))
}

fn required_string(dict: &Bound<'_, PyDict>, index: usize, key: &str) -> PyResult<String> {
    required_item(dict, index, key)?.extract().map_err(|_| {
        PyValueError::new_err(format!("record {index} field {key:?} must be a string"))
    })
}

fn optional_string(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    optional_item(dict, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!("optional record field {key:?} must be a string"))
            })
        })
        .transpose()
}

fn optional_string_list(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<Vec<String>>> {
    optional_item(dict, key)?
        .map(|value| {
            value.extract().map_err(|_| {
                PyValueError::new_err(format!(
                    "optional record field {key:?} must be a list of strings"
                ))
            })
        })
        .transpose()
}

fn required_u64(dict: &Bound<'_, PyDict>, index: usize, key: &str) -> PyResult<u64> {
    required_item(dict, index, key)?
        .extract()
        .map_err(|_| PyValueError::new_err(format!("record {index} field {key:?} must be an int")))
}

fn optional_dict<'py>(
    dict: &Bound<'py, PyDict>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    optional_item(dict, key)?
        .map(|value| {
            value.cast::<PyDict>().cloned().map_err(|_| {
                PyValueError::new_err(format!("optional record field {key:?} must be a dict"))
            })
        })
        .transpose()
}
