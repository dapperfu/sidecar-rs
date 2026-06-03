use std::io::{Read, Write};

use crate::error::{Result, SidecarError};
use crate::format::header::{
    read_f32, read_f64, read_i64, read_u32, read_u64, write_f32, write_f64, write_i64, write_u32,
    write_u64,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueKind {
    Null = 0,
    Bool = 1,
    I64 = 2,
    U64 = 3,
    F32 = 4,
    F64 = 5,
    String = 6,
    Bytes = 7,
    Array = 8,
}

impl ValueKind {
    pub fn from_tag(tag: u8) -> Result<Self> {
        match tag {
            0 => Ok(Self::Null),
            1 => Ok(Self::Bool),
            2 => Ok(Self::I64),
            3 => Ok(Self::U64),
            4 => Ok(Self::F32),
            5 => Ok(Self::F64),
            6 => Ok(Self::String),
            7 => Ok(Self::Bytes),
            8 => Ok(Self::Array),
            _ => Err(SidecarError::UnknownKindTag(tag)),
        }
    }

    pub fn tag(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Bool => "Bool",
            Self::I64 => "I64",
            Self::U64 => "U64",
            Self::F32 => "F32",
            Self::F64 => "F64",
            Self::String => "String",
            Self::Bytes => "Bytes",
            Self::Array => "Array",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
    Array {
        element_kind: ValueKind,
        elements: Vec<Value>,
    },
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Null => ValueKind::Null,
            Self::Bool(_) => ValueKind::Bool,
            Self::I64(_) => ValueKind::I64,
            Self::U64(_) => ValueKind::U64,
            Self::F32(_) => ValueKind::F32,
            Self::F64(_) => ValueKind::F64,
            Self::String(_) => ValueKind::String,
            Self::Bytes(_) => ValueKind::Bytes,
            Self::Array { element_kind, .. } => {
                let _ = element_kind;
                ValueKind::Array
            }
        }
    }

    pub fn element_kind(&self) -> Option<ValueKind> {
        match self {
            Self::Array { element_kind, .. } => Some(*element_kind),
            _ => None,
        }
    }

    pub fn inline_size(&self) -> Result<usize> {
        match self {
            Self::Null => Ok(0),
            Self::Bool(_) => Ok(1),
            Self::I64(_) | Self::U64(_) | Self::F64(_) => Ok(8),
            Self::F32(_) => Ok(4),
            Self::String(s) => Ok(4 + s.len()),
            Self::Bytes(b) => Ok(4 + b.len()),
            Self::Array { .. } => Err(SidecarError::KindMismatch {
                expected: "scalar or string/bytes".into(),
                actual: "Array".into(),
            }),
        }
    }

    pub fn write_inline<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            Self::Null => Ok(()),
            Self::Bool(v) => {
                writer.write_all(&[u8::from(*v)])?;
                Ok(())
            }
            Self::I64(v) => write_i64(writer, *v),
            Self::U64(v) => write_u64(writer, *v),
            Self::F32(v) => write_f32(writer, *v),
            Self::F64(v) => write_f64(writer, *v),
            Self::String(s) => {
                write_u32(writer, s.len() as u32)?;
                writer.write_all(s.as_bytes()).map_err(SidecarError::from)
            }
            Self::Bytes(b) => {
                write_u32(writer, b.len() as u32)?;
                writer.write_all(b).map_err(SidecarError::from)
            }
            Self::Array { .. } => Err(SidecarError::KindMismatch {
                expected: "inline scalar".into(),
                actual: "Array".into(),
            }),
        }
    }

    pub fn read_inline<R: Read>(kind: ValueKind, reader: &mut R) -> Result<Self> {
        match kind {
            ValueKind::Null => Ok(Self::Null),
            ValueKind::Bool => {
                let mut buf = [0u8; 1];
                reader.read_exact(&mut buf)?;
                Ok(Self::Bool(buf[0] != 0))
            }
            ValueKind::I64 => Ok(Self::I64(read_i64(reader)?)),
            ValueKind::U64 => Ok(Self::U64(read_u64(reader)?)),
            ValueKind::F32 => Ok(Self::F32(read_f32(reader)?)),
            ValueKind::F64 => Ok(Self::F64(read_f64(reader)?)),
            ValueKind::String => read_length_prefixed_string(reader).map(Self::String),
            ValueKind::Bytes => read_length_prefixed_bytes(reader).map(Self::Bytes),
            ValueKind::Array => Err(SidecarError::KindMismatch {
                expected: "inline scalar".into(),
                actual: "Array".into(),
            }),
        }
    }

    pub fn encode_array_payload(&self) -> Result<Vec<u8>> {
        let Self::Array {
            element_kind,
            elements,
        } = self
        else {
            return Err(SidecarError::KindMismatch {
                expected: "Array".into(),
                actual: self.kind().name().into(),
            });
        };

        let mut buf = Vec::new();
        write_u32(&mut buf, elements.len() as u32)?;
        for element in elements {
            if element.kind() != *element_kind && *element_kind != ValueKind::Null {
                return Err(SidecarError::KindMismatch {
                    expected: element_kind.name().into(),
                    actual: element.kind().name().into(),
                });
            }
            element.write_inline(&mut buf)?;
        }
        Ok(buf)
    }

    pub fn decode_array_payload(element_kind: ValueKind, data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);
        let count = read_u32(&mut cursor)? as usize;
        let mut elements = Vec::with_capacity(count);
        for _ in 0..count {
            elements.push(Self::read_inline(element_kind, &mut cursor)?);
        }
        if cursor.position() as usize != data.len() {
            return Err(SidecarError::InvalidPayload);
        }
        Ok(Self::Array {
            element_kind,
            elements,
        })
    }
}

pub fn read_length_prefixed_bytes<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let len = read_u32(reader)? as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_length_prefixed_string<R: Read>(reader: &mut R) -> Result<String> {
    let bytes = read_length_prefixed_bytes(reader)?;
    String::from_utf8(bytes).map_err(|_| SidecarError::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn f64_inline_roundtrip() {
        let value = Value::F64(0.008);
        let mut buf = Vec::new();
        value.write_inline(&mut buf).unwrap();
        let decoded = Value::read_inline(ValueKind::F64, &mut Cursor::new(buf)).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn array_payload_roundtrip() {
        let value = Value::Array {
            element_kind: ValueKind::F64,
            elements: vec![Value::F64(1.0), Value::F64(2.5)],
        };
        let payload = value.encode_array_payload().unwrap();
        let decoded = Value::decode_array_payload(ValueKind::F64, &payload).unwrap();
        assert_eq!(decoded, value);
    }
}
