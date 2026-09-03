use std::io::{Read, Write};
use std::path::Path;

use indexmap::IndexMap;

use crate::error::{Result, SidecarError};
use crate::value::Value;
use crate::MEDIA_BASENAME_KEY;

#[derive(Debug, Clone)]
pub struct SidecarDocument {
    entries: IndexMap<String, Value>,
}

impl Default for SidecarDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl SidecarDocument {
    pub fn new() -> Self {
        Self {
            entries: IndexMap::new(),
        }
    }

    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        let cbor: ciborium::value::Value =
            ciborium::from_reader(reader).map_err(|e| SidecarError::Decode(e.to_string()))?;

        let ciborium::value::Value::Map(pairs) = cbor else {
            return Err(SidecarError::NotACborMap);
        };

        let mut entries = IndexMap::with_capacity(pairs.len());
        for (key, val) in pairs {
            let ciborium::value::Value::Text(key) = key else {
                return Err(SidecarError::NonStringMapKey(format!("{key:?}")));
            };
            if key.is_empty() {
                return Err(SidecarError::EmptyKey);
            }
            entries.insert(key, Value::try_from(val)?);
        }

        Ok(Self { entries })
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        Self::from_reader(file)
    }

    pub fn to_writer<W: Write>(&self, writer: &mut W) -> Result<()> {
        let cbor = self.to_cbor_map()?;
        ciborium::into_writer(&cbor, writer).map_err(|e| SidecarError::Encode(e.to_string()))
    }

    pub fn to_path(&self, path: &Path) -> Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.to_writer(&mut writer)?;
        let inner = writer
            .into_inner()
            .map_err(|e| std::io::Error::new(e.error().kind(), e))?;
        inner.sync_all()?;
        Ok(())
    }

    /// Load `path` under an exclusive lock, apply `apply`, and atomically replace the file.
    ///
    /// Waits up to 10 seconds for `{path}.lock`. See [`update_path_with_timeout`].
    pub fn update_path<P, F>(path: P, apply: F) -> Result<()>
    where
        P: AsRef<Path>,
        F: FnMut(&mut Self) -> Result<()>,
    {
        crate::locked_io::update_path(path, apply)
    }

    /// Same as [`Self::update_path`] with an explicit lock wait.
    pub fn update_path_with_timeout<P, F>(
        path: P,
        timeout: std::time::Duration,
        apply: F,
    ) -> Result<()>
    where
        P: AsRef<Path>,
        F: FnMut(&mut Self) -> Result<()>,
    {
        crate::locked_io::update_path_with_timeout(path, timeout, apply)
    }

    pub fn byte_len(&self) -> Result<usize> {
        let mut buf = Vec::new();
        self.to_writer(&mut buf)?;
        Ok(buf.len())
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.get(key)
    }

    pub fn set(&mut self, key: impl Into<String>, value: Value) -> Result<()> {
        let key = key.into();
        if key.is_empty() {
            return Err(SidecarError::EmptyKey);
        }
        self.entries.insert(key, value);
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.entries.swap_remove(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn set_media_basename(&mut self, basename: impl Into<String>) -> Result<()> {
        self.set(MEDIA_BASENAME_KEY, Value::Text(basename.into()))
    }

    pub fn media_basename(&self) -> Option<&str> {
        match self.get(MEDIA_BASENAME_KEY) {
            Some(Value::Text(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    fn to_cbor_map(&self) -> Result<ciborium::value::Value> {
        let pairs = self
            .entries
            .iter()
            .map(|(k, v)| Ok((ciborium::value::Value::Text(k.clone()), v.to_cbor()?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(ciborium::value::Value::Map(pairs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn roundtrip_scalar_values() {
        let mut doc = SidecarDocument::new();
        doc.set("lat", Value::Float(37.7749)).unwrap();
        doc.set("iso", Value::Integer(400)).unwrap();
        doc.set("flag", Value::Bool(true)).unwrap();

        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();

        let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(decoded.get("lat"), Some(&Value::Float(37.7749)));
        assert_eq!(decoded.get("iso"), Some(&Value::Integer(400)));
        assert_eq!(decoded.get("flag"), Some(&Value::Bool(true)));
    }

    #[test]
    fn roundtrip_large_text() {
        let mut doc = SidecarDocument::new();
        let long = "x".repeat(100);
        doc.set("note", Value::Text(long.clone())).unwrap();

        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();

        let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(decoded.get("note"), Some(&Value::Text(long)));
    }

    #[test]
    fn roundtrip_heterogeneous_array() {
        let mut doc = SidecarDocument::new();
        doc.set(
            "mixed",
            Value::Array(vec![
                Value::Integer(1),
                Value::Text("two".into()),
                Value::Float(3.0),
            ]),
        )
        .unwrap();

        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();

        let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(doc.get("mixed"), decoded.get("mixed"));
    }

    #[test]
    fn roundtrip_nested_map() {
        let mut inner = IndexMap::new();
        inner.insert("width".into(), Value::Integer(800));
        inner.insert("height".into(), Value::Integer(600));
        let mut doc = SidecarDocument::new();
        doc.set("window", Value::Map(inner)).unwrap();

        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();

        let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(doc.get("window"), decoded.get("window"));
    }

    #[test]
    fn media_basename_roundtrip() {
        let mut doc = SidecarDocument::new();
        doc.set_media_basename("IMG_1234.jpg").unwrap();

        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();

        let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(decoded.media_basename(), Some("IMG_1234.jpg"));
    }

    #[test]
    fn output_is_valid_cbor_map() {
        let mut doc = SidecarDocument::new();
        doc.set("k", Value::Integer(1)).unwrap();
        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();
        let cbor: ciborium::value::Value =
            ciborium::from_reader(Cursor::new(buf)).expect("valid CBOR");
        assert!(matches!(cbor, ciborium::value::Value::Map(_)));
    }

    #[test]
    fn byte_len_matches_serialized_size() {
        let mut doc = SidecarDocument::new();
        doc.set("k", Value::Integer(1)).unwrap();
        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();
        assert_eq!(doc.byte_len().unwrap(), buf.len());
    }
}
