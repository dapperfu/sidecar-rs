use indexmap::IndexMap;

use crate::error::{Result, SidecarError};

/// A CBOR-aligned sidecar value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i128),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Map(IndexMap<String, Value>),
}

impl Value {
    /// Return a short type label for display (CLI, logging).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Bool(_) => "Bool",
            Self::Integer(_) => "Integer",
            Self::Float(_) => "Float",
            Self::Text(_) => "Text",
            Self::Bytes(_) => "Bytes",
            Self::Array(_) => "Array",
            Self::Map(_) => "Map",
        }
    }

    /// Encode this value as a dynamic CBOR value.
    pub fn to_cbor(&self) -> Result<ciborium::value::Value> {
        match self {
            Value::Null => Ok(ciborium::value::Value::Null),
            Value::Bool(v) => Ok(ciborium::value::Value::Bool(*v)),
            Value::Integer(v) => Ok(ciborium::value::Value::Integer(
                ciborium::value::Integer::try_from(*v)
                    .map_err(|_| SidecarError::IntegerOutOfRange)?,
            )),
            Value::Float(v) => Ok(ciborium::value::Value::Float(*v)),
            Value::Text(v) => Ok(ciborium::value::Value::Text(v.clone())),
            Value::Bytes(v) => Ok(ciborium::value::Value::Bytes(v.clone())),
            Value::Array(items) => {
                let elements = items
                    .iter()
                    .map(Value::to_cbor)
                    .collect::<Result<Vec<_>>>()?;
                Ok(ciborium::value::Value::Array(elements))
            }
            Value::Map(map) => {
                let pairs = map
                    .iter()
                    .map(|(k, v)| Ok((ciborium::value::Value::Text(k.clone()), v.to_cbor()?)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(ciborium::value::Value::Map(pairs))
            }
        }
    }
}

impl TryFrom<ciborium::value::Value> for Value {
    type Error = SidecarError;

    fn try_from(value: ciborium::value::Value) -> Result<Self> {
        match value {
            ciborium::value::Value::Null => Ok(Self::Null),
            ciborium::value::Value::Bool(v) => Ok(Self::Bool(v)),
            ciborium::value::Value::Integer(v) => Ok(Self::Integer(i128::from(v))),
            ciborium::value::Value::Float(v) => Ok(Self::Float(v)),
            ciborium::value::Value::Text(v) => Ok(Self::Text(v)),
            ciborium::value::Value::Bytes(v) => Ok(Self::Bytes(v)),
            ciborium::value::Value::Array(items) => {
                let elements = items
                    .into_iter()
                    .map(Value::try_from)
                    .collect::<Result<Vec<_>>>()?;
                Ok(Self::Array(elements))
            }
            ciborium::value::Value::Map(pairs) => {
                let mut map = IndexMap::with_capacity(pairs.len());
                for (key, val) in pairs {
                    let ciborium::value::Value::Text(key) = key else {
                        return Err(SidecarError::NonStringMapKey(format!("{key:?}")));
                    };
                    if key.is_empty() {
                        return Err(SidecarError::EmptyKey);
                    }
                    map.insert(key, Value::try_from(val)?);
                }
                Ok(Self::Map(map))
            }
            ciborium::value::Value::Tag(_, _) => Err(SidecarError::Decode(
                "CBOR tags are not supported in sidecar values".into(),
            )),
            _ => Err(SidecarError::Decode(
                "unsupported CBOR value kind in sidecar document".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn cbor_roundtrip_scalar() {
        let original = Value::Float(0.008);
        let cbor = original.to_cbor().unwrap();
        let mut buf = Vec::new();
        ciborium::into_writer(&cbor, &mut buf).unwrap();
        let decoded: ciborium::value::Value = ciborium::from_reader(Cursor::new(buf)).unwrap();
        assert_eq!(Value::try_from(decoded).unwrap(), original);
    }

    #[test]
    fn nested_map_roundtrip() {
        let mut inner = IndexMap::new();
        inner.insert("x".into(), Value::Integer(1));
        let original = Value::Map(inner);
        let cbor = original.to_cbor().unwrap();
        let back = Value::try_from(cbor).unwrap();
        assert_eq!(back, original);
    }
}
