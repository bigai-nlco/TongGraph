mod compute;
mod cypher;
mod graph;
mod inference;
mod properties;
mod query;
mod records;
mod snapshot;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

pub(crate) use graph::PyGraph;
pub(crate) use snapshot::PyGraphSnapshot;

pub(crate) fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(query::py_query_dsl_schema, m)?)?;
    m.add_class::<PyGraph>()?;
    m.add_class::<PyGraphSnapshot>()?;
    m.add_class::<cypher::PyCypherResult>()?;
    m.add_class::<cypher::PyGraphTransaction>()?;
    m.add_class::<records::PyNode>()?;
    m.add_class::<records::PyEdge>()?;
    m.add_class::<records::PyVariable>()?;
    m.add_class::<records::PyFactor>()?;
    m.add_class::<records::PyEvidence>()?;
    m.add_class::<records::PyTrace>()?;
    Ok(())
}

fn to_py_value_error(message: String) -> PyErr {
    PyValueError::new_err(message)
}
