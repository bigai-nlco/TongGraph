use crate::models::{EdgeRecord, NodeRecord, PropertyMap};
use crate::py_api::properties::property_map_to_py;
use pyo3::prelude::*;

#[pyclass(name = "Node", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PyNode {
    id: u64,
    external_id: String,
    labels: Vec<String>,
    properties: PropertyMap,
}

#[pymethods]
impl PyNode {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn external_id(&self) -> String {
        self.external_id.clone()
    }

    #[getter]
    fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }

    #[getter]
    fn properties(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        property_map_to_py(py, &self.properties)
    }

    fn __repr__(&self) -> String {
        format!(
            "Node(id={}, external_id={:?}, labels={:?})",
            self.id, self.external_id, self.labels
        )
    }
}

impl From<NodeRecord> for PyNode {
    fn from(value: NodeRecord) -> Self {
        Self {
            id: value.id,
            external_id: value.external_id,
            labels: value.labels,
            properties: value.properties,
        }
    }
}

#[pyclass(name = "Edge", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PyEdge {
    id: u64,
    source: u64,
    target: u64,
    edge_type: String,
    properties: PropertyMap,
}

#[pymethods]
impl PyEdge {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn source(&self) -> u64 {
        self.source
    }

    #[getter]
    fn target(&self) -> u64 {
        self.target
    }

    #[getter]
    fn edge_type(&self) -> String {
        self.edge_type.clone()
    }

    #[getter]
    fn properties(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        property_map_to_py(py, &self.properties)
    }

    fn __repr__(&self) -> String {
        format!(
            "Edge(id={}, source={}, target={}, edge_type={:?})",
            self.id, self.source, self.target, self.edge_type
        )
    }
}

impl From<EdgeRecord> for PyEdge {
    fn from(value: EdgeRecord) -> Self {
        Self {
            id: value.id,
            source: value.source,
            target: value.target,
            edge_type: value.edge_type,
            properties: value.properties,
        }
    }
}
