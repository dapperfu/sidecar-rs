use std::io::{Read, Write};
use std::path::Path;

use crate::error::Result;
use crate::format::catalog::Catalog;
use crate::format::header::Header;
use crate::format::payload::PayloadArena;
use crate::format::value::Value;
use crate::MEDIA_BASENAME_KEY;

#[derive(Debug, Clone)]
pub struct SidecarDocument {
    catalog: Catalog,
    payload: PayloadArena,
}

impl Default for SidecarDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl SidecarDocument {
    pub fn new() -> Self {
        Self {
            catalog: Catalog::new(),
            payload: PayloadArena::new(),
        }
    }

    pub fn from_reader<R: Read>(mut reader: R) -> Result<Self> {
        let header = Header::read_from(&mut reader)?;

        let mut catalog_bytes = vec![0u8; header.catalog_len as usize];
        reader.read_exact(&mut catalog_bytes)?;
        let mut catalog_cursor = std::io::Cursor::new(catalog_bytes);

        let mut payload_bytes = vec![0u8; header.payload_len as usize];
        reader.read_exact(&mut payload_bytes)?;
        let payload = PayloadArena::from_bytes(payload_bytes);

        let entry_count = crate::format::header::read_u32(&mut catalog_cursor)?;
        let mut catalog = Catalog::read_from(&mut catalog_cursor, &payload, entry_count)?;
        catalog.materialize_values(&payload)?;

        Ok(Self { catalog, payload })
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        Self::from_reader(file)
    }

    pub fn to_writer<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut catalog = self.catalog.clone();
        let payload = catalog.rebuild_payload()?;

        let mut catalog_buf = Vec::new();
        catalog.write_to(&mut catalog_buf, &payload)?;

        let header = Header::new(catalog_buf.len() as u32, payload.len() as u32);
        header.write_to(writer)?;
        writer.write_all(&catalog_buf)?;
        writer.write_all(payload.as_slice())?;
        Ok(())
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

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.catalog.get_value(key)
    }

    pub fn set(&mut self, key: impl Into<String>, value: Value) -> Result<()> {
        self.catalog.insert(key.into(), value)?;
        self.payload = self.catalog.rebuild_payload()?;
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let removed = self.catalog.remove(key).map(|e| e.value);
        if removed.is_some() {
            self.payload = self.catalog.rebuild_payload().unwrap_or_default();
        }
        removed
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.catalog.keys()
    }

    pub fn header_info(&self) -> Header {
        let mut catalog = self.catalog.clone();
        let payload = catalog.rebuild_payload().unwrap_or_default();
        let mut catalog_buf = Vec::new();
        let _ = catalog.write_to(&mut catalog_buf, &payload);
        Header::new(catalog_buf.len() as u32, payload.len() as u32)
    }

    pub fn set_media_basename(&mut self, basename: impl Into<String>) -> Result<()> {
        self.set(MEDIA_BASENAME_KEY, Value::String(basename.into()))
    }

    pub fn media_basename(&self) -> Option<&str> {
        match self.get(MEDIA_BASENAME_KEY) {
            Some(Value::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn entry_count(&self) -> usize {
        self.catalog.len()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.catalog.entries().map(|e| (e.key.as_str(), &e.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::value::ValueKind;
    use std::io::Cursor;

    #[test]
    fn roundtrip_scalar_values() {
        let mut doc = SidecarDocument::new();
        doc.set("lat", Value::F64(37.7749)).unwrap();
        doc.set("iso", Value::U64(400)).unwrap();
        doc.set("flag", Value::Bool(true)).unwrap();

        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();

        let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(decoded.get("lat"), Some(&Value::F64(37.7749)));
        assert_eq!(decoded.get("iso"), Some(&Value::U64(400)));
        assert_eq!(decoded.get("flag"), Some(&Value::Bool(true)));
    }

    #[test]
    fn roundtrip_large_string_uses_payload() {
        let mut doc = SidecarDocument::new();
        let long = "x".repeat(100);
        doc.set("note", Value::String(long.clone())).unwrap();

        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();

        let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(decoded.get("note"), Some(&Value::String(long)));
    }

    #[test]
    fn roundtrip_array() {
        let mut doc = SidecarDocument::new();
        doc.set(
            "tags",
            Value::Array {
                element_kind: ValueKind::String,
                elements: vec![Value::String("a".into()), Value::String("b".into())],
            },
        )
        .unwrap();

        let mut buf = Vec::new();
        doc.to_writer(&mut buf).unwrap();

        let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(doc.get("tags"), decoded.get("tags"));
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
}
