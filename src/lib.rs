mod codec;
mod core;
mod models;
mod py_api;
mod sqlite;

use pyo3::prelude::*;

#[pymodule]
fn _tonggraph(m: &Bound<'_, PyModule>) -> PyResult<()> {
    py_api::register_module(m)
}
