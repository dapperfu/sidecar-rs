//! PyO3 bindings exposing the `sidecar` CBOR sidecar library to Python as `sidecar_rs`.

use std::path::PathBuf;

use indexmap::IndexMap;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyKeyError, PyOverflowError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyByteArray, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString};
use pyo3::IntoPyObjectExt;
use sidecar::conventions::photo;
use sidecar::{resolve_sidecar_path, SidecarDocument, Value};

create_exception!(_sidecar_rs, SidecarError, PyException);

fn map_err(err: sidecar::SidecarError) -> PyErr {
    SidecarError::new_err(err.to_string())
}

fn callback_err() -> sidecar::SidecarError {
    sidecar::SidecarError::Decode("python update_path callback failed".into())
}

/// Convert a sidecar `Value` into an owned Python object.
fn value_to_py(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => (*b).into_py_any(py),
        Value::Integer(v) => {
            if *v >= i64::MIN as i128 && *v <= i64::MAX as i128 {
                (*v as i64).into_py_any(py)
            } else if *v >= 0 {
                (*v as u64).into_py_any(py)
            } else {
                Err(PyOverflowError::new_err(
                    "integer out of range for Python int",
                ))
            }
        }
        Value::Float(v) => (*v).into_py_any(py),
        Value::Text(s) => s.as_str().into_py_any(py),
        Value::Bytes(b) => PyBytes::new(py, b).into_py_any(py),
        Value::Array(elements) => {
            let list = PyList::empty(py);
            for element in elements {
                list.append(value_to_py(py, element)?)?;
            }
            list.into_py_any(py)
        }
        Value::Map(map) => {
            let dict = PyDict::new(py);
            for (key, val) in map {
                dict.set_item(key, value_to_py(py, val)?)?;
            }
            dict.into_py_any(py)
        }
    }
}

/// Convert a Python object into a sidecar `Value` by inferring the type.
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
            return Ok(Value::Integer(v as i128));
        }
        if let Ok(v) = i.extract::<u64>() {
            return Ok(Value::Integer(v as i128));
        }
        return Err(PyOverflowError::new_err("integer out of range for i128"));
    }
    if let Ok(f) = obj.cast::<PyFloat>() {
        return Ok(Value::Float(f.extract::<f64>()?));
    }
    if let Ok(s) = obj.cast::<PyString>() {
        return Ok(Value::Text(s.extract::<String>()?));
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
    if let Ok(dict) = obj.cast::<PyDict>() {
        return py_dict_to_map(dict);
    }
    Err(PyTypeError::new_err(format!(
        "unsupported Python type for sidecar value: {}",
        obj.get_type()
    )))
}

fn py_list_to_array(list: &Bound<'_, PyList>) -> PyResult<Value> {
    let mut elements = Vec::with_capacity(list.len());
    for item in list.iter() {
        elements.push(py_to_value(&item)?);
    }
    Ok(Value::Array(elements))
}

fn py_dict_to_map(dict: &Bound<'_, PyDict>) -> PyResult<Value> {
    let mut map = IndexMap::with_capacity(dict.len());
    for (key, val) in dict.iter() {
        let key_str = key
            .extract::<String>()
            .map_err(|_| PyTypeError::new_err("sidecar map keys must be strings"))?;
        if key_str.is_empty() {
            return Err(PyTypeError::new_err("sidecar map keys must not be empty"));
        }
        map.insert(key_str, py_to_value(&val)?);
    }
    Ok(Value::Map(map))
}

/// A CBOR sidecar document: a key-value catalog for media metadata.
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

    #[staticmethod]
    fn from_bytes(data: &[u8]) -> PyResult<Self> {
        let inner = SidecarDocument::from_reader(std::io::Cursor::new(data)).map_err(map_err)?;
        Ok(Self { inner })
    }

    #[staticmethod]
    fn from_path(path: PathBuf) -> PyResult<Self> {
        let inner = SidecarDocument::from_path(&path).map_err(map_err)?;
        Ok(Self { inner })
    }

    /// Load `path` under a file lock, call `updater(doc)`, and atomically save.
    #[staticmethod]
    fn update_path(py: Python<'_>, path: PathBuf, updater: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_err = std::cell::Cell::new(None::<PyErr>);
        let rust_result = SidecarDocument::update_path(&path, |doc| {
            let py_doc = PySidecarDocument { inner: doc.clone() };
            let py_obj = match Py::new(py, py_doc) {
                Ok(obj) => obj,
                Err(err) => {
                    py_err.set(Some(err));
                    return Err(callback_err());
                }
            };
            if let Err(err) = updater.call1((py_obj.clone_ref(py),)) {
                py_err.set(Some(err));
                return Err(callback_err());
            }
            *doc = py_obj.borrow(py).inner.clone();
            Ok(())
        });
        if let Some(err) = py_err.take() {
            return Err(err);
        }
        rust_result.map_err(map_err)
    }

    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let mut buf = Vec::new();
        self.inner.to_writer(&mut buf).map_err(map_err)?;
        Ok(PyBytes::new(py, &buf))
    }

    fn to_path(&self, path: PathBuf) -> PyResult<()> {
        self.inner.to_path(&path).map_err(map_err)
    }

    fn set(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let v = py_to_value(value)?;
        self.inner.set(key, v).map_err(map_err)
    }

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
        self.inner
            .set(key, Value::Integer(value as i128))
            .map_err(map_err)
    }

    fn set_u64(&mut self, key: &str, value: u64) -> PyResult<()> {
        self.inner
            .set(key, Value::Integer(value as i128))
            .map_err(map_err)
    }

    fn set_f32(&mut self, key: &str, value: f32) -> PyResult<()> {
        self.inner
            .set(key, Value::Float(f64::from(value)))
            .map_err(map_err)
    }

    fn set_f64(&mut self, key: &str, value: f64) -> PyResult<()> {
        self.inner.set(key, Value::Float(value)).map_err(map_err)
    }

    fn set_str(&mut self, key: &str, value: String) -> PyResult<()> {
        self.inner.set(key, Value::Text(value)).map_err(map_err)
    }

    fn set_bytes(&mut self, key: &str, value: Vec<u8>) -> PyResult<()> {
        self.inner.set(key, Value::Bytes(value)).map_err(map_err)
    }

    fn set_array(&mut self, key: &str, value: &Bound<'_, PyList>) -> PyResult<()> {
        let array = py_list_to_array(value)?;
        self.inner.set(key, array).map_err(map_err)
    }

    fn set_map(&mut self, key: &str, value: &Bound<'_, PyDict>) -> PyResult<()> {
        let map = py_dict_to_map(value)?;
        self.inner.set(key, map).map_err(map_err)
    }

    fn remove(&mut self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        match self.inner.remove(key) {
            Some(value) => value_to_py(py, &value),
            None => Ok(py.None()),
        }
    }

    fn keys(&self) -> Vec<String> {
        self.inner.keys().map(str::to_string).collect()
    }

    fn entries(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in self.inner.entries() {
            dict.set_item(key, value_to_py(py, value)?)?;
        }
        Ok(dict.unbind())
    }

    fn set_media_basename(&mut self, basename: &str) -> PyResult<()> {
        self.inner.set_media_basename(basename).map_err(map_err)
    }

    fn media_basename(&self) -> Option<String> {
        self.inner.media_basename().map(str::to_string)
    }

    fn header(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("format", "cbor")?;
        dict.set_item("entry_count", self.inner.entry_count())?;
        dict.set_item("byte_len", self.inner.byte_len().map_err(map_err)?)?;
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

#[pyfunction]
#[pyo3(name = "resolve_sidecar_path")]
fn py_resolve_sidecar_path(path: &str) -> PyResult<String> {
    Ok(resolve_sidecar_path(std::path::Path::new(path))
        .display()
        .to_string())
}

#[pymodule]
fn _sidecar_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySidecarDocument>()?;
    m.add_function(wrap_pyfunction!(py_resolve_sidecar_path, m)?)?;
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
