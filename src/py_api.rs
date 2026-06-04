use crate::core::GraphCore;
use crate::models::{PyEdge, PyNode};
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
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
        properties: Option<HashMap<String, String>>,
    ) -> PyResult<u64> {
        self.core
            .add_node(
                external_id,
                labels.unwrap_or_default(),
                properties.unwrap_or_default(),
            )
            .map_err(to_py_value_error)
    }

    #[pyo3(signature = (source, target, edge_type, properties=None))]
    fn add_edge(
        &mut self,
        source: u64,
        target: u64,
        edge_type: String,
        properties: Option<HashMap<String, String>>,
    ) -> PyResult<u64> {
        self.core
            .add_edge(source, target, edge_type, properties.unwrap_or_default())
            .map_err(to_py_value_error)
    }

    fn node_count(&self) -> usize {
        self.core.node_count()
    }

    fn edge_count(&self) -> usize {
        self.core.edge_count()
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

    fn get_node_id(&self, external_id: String) -> Option<u64> {
        self.core.get_node_id(&external_id)
    }

    fn nodes_with_label(&self, label: String) -> Vec<u64> {
        self.core.nodes_with_label(&label)
    }

    fn edges_by_type(&self, edge_type: String) -> Vec<u64> {
        self.core.edges_by_type(&edge_type)
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
}

pub(crate) fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<PyGraph>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyEdge>()?;
    Ok(())
}

fn to_py_value_error(message: String) -> PyErr {
    PyValueError::new_err(message)
}
