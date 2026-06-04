//! PyO3 bindings exposing the `sidecar` SCAR library to Python as `sidecar_rs`.

use std::path::PathBuf;

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyKeyError, PyOverflowError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyByteArray, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString};
use pyo3::IntoPyObjectExt;
use sidecar::conventions::photo;
use sidecar::{SidecarDocument, Value, ValueKind};

create_exception!(_sidecar_rs, SidecarError, PyException);

fn map_err(err: sidecar::SidecarError) -> PyErr {
    SidecarError::new_err(err.to_string())
}

/// Convert a SCAR `Value` into an owned Python object.
fn value_to_py(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => (*b).into_py_any(py),
        Value::I64(v) => (*v).into_py_any(py),
        Value::U64(v) => (*v).into_py_any(py),
        Value::F32(v) => (*v as f64).into_py_any(py),
        Value::F64(v) => (*v).into_py_any(py),
        Value::String(s) => s.as_str().into_py_any(py),
        Value::Bytes(b) => PyBytes::new(py, b).into_py_any(py),
        Value::Array { elements, .. } => {
            let list = PyList::empty(py);
            for element in elements {
                list.append(value_to_py(py, element)?)?;
            }
            list.into_py_any(py)
        }
    }
}

/// Convert a Python object into a SCAR `Value` by inferring the type.
///
/// `bool` is checked before `int` because Python's `bool` subclasses `int`.
fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Null);
    }
    if let Ok(b) = obj.cast::<PyBool>() {
        return Ok(Value::Bool(b.is_true()));
    }
    if let Ok(i) = obj.cast::<PyInt>() {
        if let Ok(v) = i.extract::<i64>() {
            return Ok(Value::I64(v));
        }
        if let Ok(v) = i.extract::<u64>() {
            return Ok(Value::U64(v));
        }
        return Err(PyOverflowError::new_err("integer out of range for i64/u64"));
    }
    if let Ok(f) = obj.cast::<PyFloat>() {
        return Ok(Value::F64(f.extract::<f64>()?));
    }
    if let Ok(s) = obj.cast::<PyString>() {
        return Ok(Value::String(s.extract::<String>()?));
    }
    if let Ok(b) = obj.cast::<PyBytes>() {
        return Ok(Value::Bytes(b.as_bytes().to_vec()));
    }
    if let Ok(ba) = obj.cast::<PyByteArray>() {
        return Ok(Value::Bytes(ba.to_vec()));
    }
    if let Ok(list) = obj.cast::<PyList>() {
        return py_list_to_array(list);
    }
    Err(PyTypeError::new_err(format!(
        "unsupported Python type for sidecar value: {}",
        obj.get_type()
    )))
}

/// Convert a Python list into a homogeneous SCAR array `Value`.
fn py_list_to_array(list: &Bound<'_, PyList>) -> PyResult<Value> {
    let mut elements = Vec::with_capacity(list.len());
    for item in list.iter() {
        elements.push(py_to_value(&item)?);
    }
    let element_kind = elements.first().map(Value::kind).unwrap_or(ValueKind::Null);
    for element in &elements {
        if element.kind() != element_kind {
            return Err(PyTypeError::new_err(
                "sidecar arrays must be homogeneous (all elements the same type)",
            ));
        }
    }
    Ok(Value::Array {
        element_kind,
        elements,
    })
}

/// A SCAR sidecar document: a typed key-value catalog with a binary payload.
#[pyclass(name = "SidecarDocument", module = "sidecar_rs._sidecar_rs")]
struct PySidecarDocument {
    inner: SidecarDocument,
}

#[pymethods]
impl PySidecarDocument {
    #[new]
    fn new() -> Self {
        Self {
            inner: SidecarDocument::new(),
        }
    }

    /// Load a document from raw SCAR bytes.
    #[staticmethod]
    fn from_bytes(data: &[u8]) -> PyResult<Self> {
        let inner = SidecarDocument::from_reader(std::io::Cursor::new(data)).map_err(map_err)?;
        Ok(Self { inner })
    }

    /// Load a document from a `.scar` file path.
    #[staticmethod]
    fn from_path(path: PathBuf) -> PyResult<Self> {
        let inner = SidecarDocument::from_path(&path).map_err(map_err)?;
        Ok(Self { inner })
    }

    /// Serialize the document to raw SCAR bytes.
    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let mut buf = Vec::new();
        self.inner.to_writer(&mut buf).map_err(map_err)?;
        Ok(PyBytes::new(py, &buf))
    }

    /// Write the document to a `.scar` file path.
    fn to_path(&self, path: PathBuf) -> PyResult<()> {
        self.inner.to_path(&path).map_err(map_err)
    }

    /// Set a value, inferring the SCAR type from the Python object.
    fn set(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let v = py_to_value(value)?;
        self.inner.set(key, v).map_err(map_err)
    }

    /// Get a value, or `None` if the key is absent.
    fn get(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        match self.inner.get(key) {
            Some(value) => value_to_py(py, value),
            None => Ok(py.None()),
        }
    }

    fn set_null(&mut self, key: &str) -> PyResult<()> {
        self.inner.set(key, Value::Null).map_err(map_err)
    }

    fn set_bool(&mut self, key: &str, value: bool) -> PyResult<()> {
        self.inner.set(key, Value::Bool(value)).map_err(map_err)
    }

    fn set_i64(&mut self, key: &str, value: i64) -> PyResult<()> {
        self.inner.set(key, Value::I64(value)).map_err(map_err)
    }

    fn set_u64(&mut self, key: &str, value: u64) -> PyResult<()> {
        self.inner.set(key, Value::U64(value)).map_err(map_err)
    }

    fn set_f32(&mut self, key: &str, value: f32) -> PyResult<()> {
        self.inner.set(key, Value::F32(value)).map_err(map_err)
    }

    fn set_f64(&mut self, key: &str, value: f64) -> PyResult<()> {
        self.inner.set(key, Value::F64(value)).map_err(map_err)
    }

    fn set_str(&mut self, key: &str, value: String) -> PyResult<()> {
        self.inner.set(key, Value::String(value)).map_err(map_err)
    }

    fn set_bytes(&mut self, key: &str, value: Vec<u8>) -> PyResult<()> {
        self.inner.set(key, Value::Bytes(value)).map_err(map_err)
    }

    /// Set a homogeneous array value from a Python list.
    fn set_array(&mut self, key: &str, value: &Bound<'_, PyList>) -> PyResult<()> {
        let array = py_list_to_array(value)?;
        self.inner.set(key, array).map_err(map_err)
    }

    /// Remove a key, returning its value or `None`.
    fn remove(&mut self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        match self.inner.remove(key) {
            Some(value) => value_to_py(py, &value),
            None => Ok(py.None()),
        }
    }

    /// Return all catalog keys.
    fn keys(&self) -> Vec<String> {
        self.inner.keys().map(str::to_string).collect()
    }

    /// Return all entries as a dict of key -> value.
    fn entries(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in self.inner.entries() {
            dict.set_item(key, value_to_py(py, value)?)?;
        }
        Ok(dict.unbind())
    }

    /// Set the linked media basename (`_media.basename`).
    fn set_media_basename(&mut self, basename: &str) -> PyResult<()> {
        self.inner.set_media_basename(basename).map_err(map_err)
    }

    /// Return the linked media basename, if set.
    fn media_basename(&self) -> Option<String> {
        self.inner.media_basename().map(str::to_string)
    }

    /// Return header information as a dict.
    fn header(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let header = self.inner.header_info();
        let dict = PyDict::new(py);
        dict.set_item("version", header.version)?;
        dict.set_item("flags", header.flags)?;
        dict.set_item("catalog_len", header.catalog_len)?;
        dict.set_item("payload_len", header.payload_len)?;
        dict.set_item("entry_count", self.inner.entry_count())?;
        Ok(dict.unbind())
    }

    fn __len__(&self) -> usize {
        self.inner.entry_count()
    }

    fn __contains__(&self, key: &str) -> bool {
        self.inner.get(key).is_some()
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        match self.inner.get(key) {
            Some(value) => value_to_py(py, value),
            None => Err(PyKeyError::new_err(key.to_string())),
        }
    }

    fn __setitem__(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.set(key, value)
    }

    fn __delitem__(&mut self, key: &str) -> PyResult<()> {
        if self.inner.remove(key).is_none() {
            return Err(PyKeyError::new_err(key.to_string()));
        }
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("SidecarDocument(entries={})", self.inner.entry_count())
    }
}

#[pymodule]
fn _sidecar_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySidecarDocument>()?;
    m.add("SidecarError", m.py().get_type::<SidecarError>())?;

    m.add("MEDIA_BASENAME_KEY", sidecar::MEDIA_BASENAME_KEY)?;
    m.add("SIDECAR_EXTENSION", sidecar::SIDECAR_EXTENSION)?;

    m.add("PHOTO_GPS_LATITUDE", photo::GPS_LATITUDE)?;
    m.add("PHOTO_GPS_LONGITUDE", photo::GPS_LONGITUDE)?;
    m.add("PHOTO_EXPOSURE_TIME_SEC", photo::EXPOSURE_TIME_SEC)?;
    m.add("PHOTO_EXPOSURE_ISO", photo::EXPOSURE_ISO)?;
    m.add("PHOTO_LENS_FOCAL_LENGTH_MM", photo::LENS_FOCAL_LENGTH_MM)?;
    m.add("PHOTO_RATING", photo::RATING)?;
    m.add("PHOTO_TAGS", photo::TAGS)?;

    Ok(())
}
