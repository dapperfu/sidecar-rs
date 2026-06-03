use std::collections::BTreeMap;
use std::io::{Read, Write};

use crate::error::{Result, SidecarError};
use crate::format::header::{read_u32, write_u32};
use crate::format::payload::PayloadArena;
use crate::format::value::{Value, ValueKind};
use crate::format::INLINE_THRESHOLD;

const STORAGE_INLINE: u8 = 0;
const STORAGE_PAYLOAD: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub key: String,
    pub value: Value,
    pub storage: EntryStorage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryStorage {
    Inline,
    PayloadRef { offset: u32, len: u32 },
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    entries: BTreeMap<String, CatalogEntry>,
}

impl Catalog {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn from_entries(entries: BTreeMap<String, CatalogEntry>) -> Self {
        Self { entries }
    }

    pub fn get(&self, key: &str) -> Option<&CatalogEntry> {
        self.entries.get(key)
    }

    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.entries.get(key).map(|e| &e.value)
    }

    pub fn insert(&mut self, key: String, value: Value) -> Result<()> {
        if key.is_empty() {
            return Err(SidecarError::EmptyKey);
        }
        let storage = plan_storage(&value)?;
        let entry = CatalogEntry {
            key: key.clone(),
            value,
            storage,
        };
        self.entries.insert(key, entry);
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Option<CatalogEntry> {
        self.entries.remove(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn entries(&self) -> impl Iterator<Item = &CatalogEntry> {
        self.entries.values()
    }

    pub fn entries_mut(&mut self) -> impl Iterator<Item = &mut CatalogEntry> {
        self.entries.values_mut()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn write_to<W: Write>(&self, writer: &mut W, payload: &PayloadArena) -> Result<()> {
        write_u32(writer, self.entries.len() as u32)?;
        for entry in self.entries.values() {
            write_entry(writer, entry, payload)?;
        }
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R, payload: &PayloadArena, count: u32) -> Result<Self> {
        let mut catalog = Self::new();
        for _ in 0..count {
            let entry = read_entry(reader, payload)?;
            catalog.entries.insert(entry.key.clone(), entry);
        }
        Ok(catalog)
    }

    pub fn materialize_values(&mut self, payload: &PayloadArena) -> Result<()> {
        for entry in self.entries.values_mut() {
            if let EntryStorage::PayloadRef { offset, len } = entry.storage {
                let kind = entry.value.kind();
                let element_kind = entry.value.element_kind().unwrap_or(ValueKind::Null);
                entry.value = match kind {
                    ValueKind::Array => {
                        let data = payload.slice(offset, len)?;
                        Value::decode_array_payload(element_kind, data)?
                    }
                    ValueKind::String => {
                        let data = payload.slice(offset, len)?;
                        String::from_utf8(data.to_vec())
                            .map(Value::String)
                            .map_err(|_| SidecarError::InvalidUtf8)?
                    }
                    ValueKind::Bytes => {
                        let data = payload.slice(offset, len)?;
                        Value::Bytes(data.to_vec())
                    }
                    _ => payload.read_value(offset, len, kind)?,
                };
                entry.storage = EntryStorage::Inline;
            }
        }
        Ok(())
    }

    pub fn rebuild_payload(&mut self) -> Result<PayloadArena> {
        let mut payload = PayloadArena::new();
        for entry in self.entries.values_mut() {
            match plan_storage(&entry.value)? {
                EntryStorage::Inline => {
                    entry.storage = EntryStorage::Inline;
                }
                EntryStorage::PayloadRef { .. } => {
                    let bytes = match &entry.value {
                        Value::Array { .. } => entry.value.encode_array_payload()?,
                        Value::String(s) => s.as_bytes().to_vec(),
                        Value::Bytes(b) => b.clone(),
                        _ => unreachable!("plan_storage only returns payload for large values"),
                    };
                    let (offset, len) = payload.append(&bytes);
                    entry.storage = EntryStorage::PayloadRef { offset, len };
                }
            }
        }
        Ok(payload)
    }
}

fn plan_storage(value: &Value) -> Result<EntryStorage> {
    match value {
        Value::Array { .. } => Ok(EntryStorage::PayloadRef { offset: 0, len: 0 }),
        Value::String(s) if s.len() > INLINE_THRESHOLD => {
            Ok(EntryStorage::PayloadRef { offset: 0, len: 0 })
        }
        Value::Bytes(b) if b.len() > INLINE_THRESHOLD => {
            Ok(EntryStorage::PayloadRef { offset: 0, len: 0 })
        }
        other => {
            let size = other.inline_size()?;
            if size > INLINE_THRESHOLD + 4 {
                return Err(SidecarError::InlineTooLarge {
                    max: INLINE_THRESHOLD,
                });
            }
            Ok(EntryStorage::Inline)
        }
    }
}

fn write_entry<W: Write>(
    writer: &mut W,
    entry: &CatalogEntry,
    payload: &PayloadArena,
) -> Result<()> {
    let key_bytes = entry.key.as_bytes();
    write_u32(writer, key_bytes.len() as u32)?;
    writer.write_all(key_bytes)?;

    let kind = entry.value.kind();
    writer.write_all(&[kind.tag()])?;

    let element_kind = entry.value.element_kind().unwrap_or(ValueKind::Null);
    writer.write_all(&[element_kind.tag()])?;

    match &entry.storage {
        EntryStorage::Inline => {
            writer.write_all(&[STORAGE_INLINE])?;
            entry.value.write_inline(writer)?;
        }
        EntryStorage::PayloadRef { offset, len } => {
            writer.write_all(&[STORAGE_PAYLOAD])?;
            write_u32(writer, *offset)?;
            write_u32(writer, *len)?;
            let _ = payload;
        }
    }
    Ok(())
}

fn read_entry<R: Read>(reader: &mut R, payload: &PayloadArena) -> Result<CatalogEntry> {
    let key_len = read_u32(reader)? as usize;
    let mut key_buf = vec![0u8; key_len];
    reader.read_exact(&mut key_buf)?;
    let key = String::from_utf8(key_buf).map_err(|_| SidecarError::InvalidUtf8)?;

    let mut kind_tag = [0u8; 1];
    reader.read_exact(&mut kind_tag)?;
    let kind = ValueKind::from_tag(kind_tag[0])?;

    let mut element_kind_tag = [0u8; 1];
    reader.read_exact(&mut element_kind_tag)?;
    let element_kind = ValueKind::from_tag(element_kind_tag[0])?;

    let mut storage_tag = [0u8; 1];
    reader.read_exact(&mut storage_tag)?;

    let (storage, value) = match storage_tag[0] {
        STORAGE_INLINE => {
            let value = if kind == ValueKind::Array {
                Value::Array {
                    element_kind,
                    elements: Vec::new(),
                }
            } else {
                Value::read_inline(kind, reader)?
            };
            (EntryStorage::Inline, value)
        }
        STORAGE_PAYLOAD => {
            let offset = read_u32(reader)?;
            let len = read_u32(reader)?;
            let storage = EntryStorage::PayloadRef { offset, len };
            let value = match kind {
                ValueKind::Array => Value::Array {
                    element_kind,
                    elements: Vec::new(),
                },
                ValueKind::String => Value::String(String::new()),
                ValueKind::Bytes => Value::Bytes(Vec::new()),
                _ => payload.read_value(offset, len, kind)?,
            };
            (storage, value)
        }
        _ => return Err(SidecarError::InvalidCatalog),
    };

    Ok(CatalogEntry {
        key,
        value,
        storage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_insert_and_get() {
        let mut catalog = Catalog::new();
        catalog
            .insert("photo.rating".into(), Value::U64(4))
            .unwrap();
        assert_eq!(catalog.get_value("photo.rating"), Some(&Value::U64(4)));
    }
}
