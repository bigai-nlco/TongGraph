use crate::models::{PropertyMap, PropertyValue};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PyFloat, PyInt, PyString};

pub(super) fn property_map_to_py(py: Python<'_>, properties: &PropertyMap) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (key, value) in properties {
        match value {
            PropertyValue::Bool(value) => dict.set_item(key, *value)?,
            PropertyValue::Int(value) => dict.set_item(key, *value)?,
            PropertyValue::Float(value) => dict.set_item(key, *value)?,
            PropertyValue::String(value) => dict.set_item(key, value)?,
        }
    }
    Ok(dict.into_any().unbind())
}

pub(super) fn properties_from_py(properties: Option<&Bound<'_, PyDict>>) -> PyResult<PropertyMap> {
    let mut result = PropertyMap::new();
    let Some(properties) = properties else {
        return Ok(result);
    };
    for (key, value) in properties.iter() {
        let key = key
            .extract::<String>()
            .map_err(|_| PyValueError::new_err("property keys must be strings"))?;
        result.insert(key, property_value_from_py(&value)?);
    }
    Ok(result)
}

pub(super) fn optional_property_value_from_py(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<PropertyValue>> {
    value.map(property_value_from_py).transpose()
}

fn property_value_from_py(value: &Bound<'_, PyAny>) -> PyResult<PropertyValue> {
    if value.is_instance_of::<PyBool>() {
        return Ok(PropertyValue::Bool(value.extract()?));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(PropertyValue::Int(value.extract()?));
    }
    if value.is_instance_of::<PyFloat>() {
        let parsed = value.extract::<f64>()?;
        if parsed.is_finite() {
            return Ok(PropertyValue::Float(parsed));
        }
        return Err(PyValueError::new_err(
            "float property values must be finite",
        ));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(PropertyValue::String(value.extract()?));
    }
    Err(PyValueError::new_err(
        "property values must be str, int, float, or bool",
    ))
}
