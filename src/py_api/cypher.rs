use crate::core::GraphCore;
use crate::cypher::{self, CypherParams, CypherResult, CypherSummary, CypherValue};
use crate::py_api::records::{PyEdge, PyNode};
use crate::py_api::to_py_value_error;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PyFloat, PyInt, PyList, PyNone, PyString};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[pyclass(name = "CypherResult", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PyCypherResult {
    keys: Vec<String>,
    records: Vec<Vec<CypherValue>>,
    summary: CypherSummary,
}

#[pymethods]
impl PyCypherResult {
    #[getter]
    fn keys(&self) -> Vec<String> {
        self.keys.clone()
    }

    #[getter]
    fn records(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        records_to_py(py, &self.keys, &self.records)
    }

    #[getter]
    fn summary(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        summary_to_py(py, &self.summary)
    }

    fn __len__(&self) -> usize {
        self.records.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "CypherResult(keys={:?}, rows={})",
            self.keys,
            self.records.len()
        )
    }
}

impl From<CypherResult> for PyCypherResult {
    fn from(value: CypherResult) -> Self {
        Self {
            keys: value.keys,
            records: value.records,
            summary: value.summary,
        }
    }
}

#[pyclass(name = "GraphTransaction", unsendable)]
pub(crate) struct PyGraphTransaction {
    parent: Rc<RefCell<GraphCore>>,
    staged: GraphCore,
    base_version: u64,
    write: bool,
    active: bool,
}

#[pymethods]
impl PyGraphTransaction {
    #[pyo3(signature = (query, parameters=None))]
    fn run(
        &mut self,
        query: &str,
        parameters: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyCypherResult> {
        self.ensure_active()?;
        let params = params_from_py(parameters)?;
        let mut next_staged = self.staged.snapshot();
        let result = cypher::execute_transaction(&mut next_staged, query, &params, self.write)
            .map(PyCypherResult::from)
            .map_err(to_py_value_error)?;
        self.staged = next_staged;
        Ok(result)
    }

    fn commit(&mut self) -> PyResult<()> {
        self.ensure_active()?;
        if self.write {
            self.parent
                .borrow_mut()
                .commit_transaction_snapshot(&self.staged, self.base_version)
                .map_err(to_py_value_error)?;
        }
        self.active = false;
        Ok(())
    }

    fn rollback(&mut self) {
        self.active = false;
    }

    fn __enter__(slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        exc_type: Option<&Bound<'_, PyAny>>,
        _exc: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        if exc_type.is_some() {
            self.rollback();
            Ok(false)
        } else if !self.active {
            Ok(false)
        } else {
            self.commit()?;
            Ok(false)
        }
    }
}

impl PyGraphTransaction {
    pub(super) fn new(parent: Rc<RefCell<GraphCore>>, write: bool) -> Self {
        let (staged, base_version) = {
            let core = parent.borrow();
            core.transaction_snapshot()
        };
        Self {
            parent,
            staged,
            base_version,
            write,
            active: true,
        }
    }

    fn ensure_active(&self) -> PyResult<()> {
        if self.active {
            Ok(())
        } else {
            Err(PyValueError::new_err("transaction is no longer active"))
        }
    }
}

pub(super) fn params_from_py(parameters: Option<&Bound<'_, PyDict>>) -> PyResult<CypherParams> {
    let mut result = BTreeMap::new();
    let Some(parameters) = parameters else {
        return Ok(result);
    };
    for (key, value) in parameters.iter() {
        let key = key
            .extract::<String>()
            .map_err(|_| PyValueError::new_err("Cypher parameter keys must be strings"))?;
        result.insert(key, value_from_py(&value)?);
    }
    Ok(result)
}

fn value_from_py(value: &Bound<'_, PyAny>) -> PyResult<CypherValue> {
    if value.is_instance_of::<PyNone>() {
        return Ok(CypherValue::Null);
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(CypherValue::Bool(value.extract()?));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(CypherValue::Int(value.extract()?));
    }
    if value.is_instance_of::<PyFloat>() {
        let parsed = value.extract::<f64>()?;
        if parsed.is_finite() {
            return Ok(CypherValue::Float(parsed));
        }
        return Err(PyValueError::new_err(
            "Cypher float parameters must be finite",
        ));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(CypherValue::String(value.extract()?));
    }
    if let Ok(list) = value.cast::<PyList>() {
        return Ok(CypherValue::List(
            list.iter()
                .map(|item| value_from_py(&item))
                .collect::<PyResult<Vec<_>>>()?,
        ));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let mut map = BTreeMap::new();
        for (key, value) in dict.iter() {
            let key = key
                .extract::<String>()
                .map_err(|_| PyValueError::new_err("Cypher map keys must be strings"))?;
            map.insert(key, value_from_py(&value)?);
        }
        return Ok(CypherValue::Map(map));
    }
    Err(PyValueError::new_err(
        "Cypher parameters must be None, bool, int, float, str, list, or dict",
    ))
}

fn records_to_py(
    py: Python<'_>,
    keys: &[String],
    records: &[Vec<CypherValue>],
) -> PyResult<Py<PyAny>> {
    let rows = PyList::empty(py);
    for record in records {
        let dict = PyDict::new(py);
        for (index, key) in keys.iter().enumerate() {
            dict.set_item(key, value_to_py(py, &record[index])?)?;
        }
        rows.append(dict)?;
    }
    Ok(rows.into_any().unbind())
}

fn summary_to_py(py: Python<'_>, summary: &CypherSummary) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("statement_type", &summary.statement_type)?;
    dict.set_item("nodes_created", summary.nodes_created)?;
    dict.set_item("relationships_created", summary.relationships_created)?;
    dict.set_item("properties_set", summary.properties_set)?;
    dict.set_item("properties_removed", summary.properties_removed)?;
    dict.set_item("labels_added", summary.labels_added)?;
    dict.set_item("labels_removed", summary.labels_removed)?;
    dict.set_item("nodes_deleted", summary.nodes_deleted)?;
    dict.set_item("relationships_deleted", summary.relationships_deleted)?;
    dict.set_item("rows", summary.rows)?;
    Ok(dict.into_any().unbind())
}

fn value_to_py(py: Python<'_>, value: &CypherValue) -> PyResult<Py<PyAny>> {
    match value {
        CypherValue::Null => Ok(py.None()),
        CypherValue::Bool(value) => Ok(PyBool::new(py, *value).to_owned().into_any().unbind()),
        CypherValue::Int(value) => Ok(PyInt::new(py, *value).into_any().unbind()),
        CypherValue::Float(value) => Ok(PyFloat::new(py, *value).into_any().unbind()),
        CypherValue::String(value) => Ok(PyString::new(py, value).into_any().unbind()),
        CypherValue::Node(node) => {
            Py::new(py, PyNode::from(node.clone())).map(|value| value.into_any())
        }
        CypherValue::Relationship(edge) => {
            Py::new(py, PyEdge::from(edge.clone())).map(|value| value.into_any())
        }
        CypherValue::List(values) => {
            let list = PyList::empty(py);
            for value in values {
                list.append(value_to_py(py, value)?)?;
            }
            Ok(list.into_any().unbind())
        }
        CypherValue::Map(values) => {
            let dict = PyDict::new(py);
            for (key, value) in values {
                dict.set_item(key, value_to_py(py, value)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}
