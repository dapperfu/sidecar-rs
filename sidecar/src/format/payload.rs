use crate::error::{Result, SidecarError};
use crate::format::value::Value;

#[derive(Debug, Clone, Default)]
pub struct PayloadArena {
    data: Vec<u8>,
}

impl PayloadArena {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn append(&mut self, bytes: &[u8]) -> (u32, u32) {
        let offset = self.data.len() as u32;
        let len = bytes.len() as u32;
        self.data.extend_from_slice(bytes);
        (offset, len)
    }

    pub fn slice(&self, offset: u32, len: u32) -> Result<&[u8]> {
        let start = offset as usize;
        let end = start
            .checked_add(len as usize)
            .ok_or(SidecarError::PayloadOutOfBounds { offset, len })?;
        if end > self.data.len() {
            return Err(SidecarError::PayloadOutOfBounds { offset, len });
        }
        Ok(&self.data[start..end])
    }

    pub fn read_value(
        &self,
        offset: u32,
        len: u32,
        kind: crate::format::value::ValueKind,
    ) -> Result<Value> {
        let data = self.slice(offset, len)?;
        match kind {
            crate::format::value::ValueKind::String => {
                let s = String::from_utf8(data.to_vec()).map_err(|_| SidecarError::InvalidUtf8)?;
                Ok(Value::String(s))
            }
            crate::format::value::ValueKind::Bytes => Ok(Value::Bytes(data.to_vec())),
            _ => {
                let mut cursor = std::io::Cursor::new(data);
                Value::read_inline(kind, &mut cursor)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_slice() {
        let mut arena = PayloadArena::new();
        let (off, len) = arena.append(b"hello");
        assert_eq!(arena.slice(off, len).unwrap(), b"hello");
    }

    #[test]
    fn out_of_bounds() {
        let arena = PayloadArena::new();
        assert!(arena.slice(0, 1).is_err());
    }
}
