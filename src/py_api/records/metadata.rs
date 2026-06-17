use crate::models::{EvidenceRecord, FactorRecord, PropertyMap, TraceRecord, VariableRecord};
use crate::py_api::properties::property_map_to_py;
use pyo3::prelude::*;

#[pyclass(name = "Variable", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PyVariable {
    id: u64,
    owner_id: Option<u64>,
    domain: String,
    states: Vec<String>,
    prior: PropertyMap,
    posterior: PropertyMap,
}

#[pymethods]
impl PyVariable {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn owner_id(&self) -> Option<u64> {
        self.owner_id
    }

    #[getter]
    fn domain(&self) -> String {
        self.domain.clone()
    }

    #[getter]
    fn states(&self) -> Vec<String> {
        self.states.clone()
    }

    #[getter]
    fn prior(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        property_map_to_py(py, &self.prior)
    }

    #[getter]
    fn posterior(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        property_map_to_py(py, &self.posterior)
    }
}

impl From<VariableRecord> for PyVariable {
    fn from(value: VariableRecord) -> Self {
        Self {
            id: value.id,
            owner_id: value.owner_id,
            domain: value.domain,
            states: value.states,
            prior: value.prior,
            posterior: value.posterior,
        }
    }
}

#[pyclass(name = "Factor", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PyFactor {
    id: u64,
    input_variables: Vec<u64>,
    output_variables: Vec<u64>,
    function: String,
    parameters: PropertyMap,
}

#[pymethods]
impl PyFactor {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn input_variables(&self) -> Vec<u64> {
        self.input_variables.clone()
    }

    #[getter]
    fn output_variables(&self) -> Vec<u64> {
        self.output_variables.clone()
    }

    #[getter]
    fn function(&self) -> String {
        self.function.clone()
    }

    #[getter]
    fn parameters(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        property_map_to_py(py, &self.parameters)
    }
}

impl From<FactorRecord> for PyFactor {
    fn from(value: FactorRecord) -> Self {
        Self {
            id: value.id,
            input_variables: value.input_variables,
            output_variables: value.output_variables,
            function: value.function,
            parameters: value.parameters,
        }
    }
}

#[pyclass(name = "Evidence", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PyEvidence {
    id: u64,
    variable_id: u64,
    payload: PropertyMap,
}

#[pymethods]
impl PyEvidence {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn variable_id(&self) -> u64 {
        self.variable_id
    }

    #[getter]
    fn payload(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        property_map_to_py(py, &self.payload)
    }
}

impl From<EvidenceRecord> for PyEvidence {
    fn from(value: EvidenceRecord) -> Self {
        Self {
            id: value.id,
            variable_id: value.variable_id,
            payload: value.payload,
        }
    }
}

#[pyclass(name = "Trace", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PyTrace {
    id: u64,
    payload: PropertyMap,
}

#[pymethods]
impl PyTrace {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn payload(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        property_map_to_py(py, &self.payload)
    }
}

impl From<TraceRecord> for PyTrace {
    fn from(value: TraceRecord) -> Self {
        Self {
            id: value.id,
            payload: value.payload,
        }
    }
}
