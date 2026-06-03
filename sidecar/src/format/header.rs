use std::io::{Read, Write};

use crate::error::{Result, SidecarError};

pub const MAGIC: [u8; 4] = *b"SCAR";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub version: u16,
    pub flags: u16,
    pub catalog_len: u32,
    pub payload_len: u32,
}

impl Header {
    pub fn new(catalog_len: u32, payload_len: u32) -> Self {
        Self {
            version: FORMAT_VERSION,
            flags: 0,
            catalog_len,
            payload_len,
        }
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(SidecarError::InvalidMagic);
        }

        let version = read_u16(reader)?;
        if version != FORMAT_VERSION {
            return Err(SidecarError::UnsupportedVersion(version));
        }

        let flags = read_u16(reader)?;
        let catalog_len = read_u32(reader)?;
        let payload_len = read_u32(reader)?;

        Ok(Self {
            version,
            flags,
            catalog_len,
            payload_len,
        })
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&MAGIC)?;
        write_u16(writer, self.version)?;
        write_u16(writer, self.flags)?;
        write_u32(writer, self.catalog_len)?;
        write_u32(writer, self.payload_len)?;
        Ok(())
    }
}

pub fn read_u16<R: Read>(reader: &mut R) -> Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn read_u32<R: Read>(reader: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn read_u64<R: Read>(reader: &mut R) -> Result<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

pub fn read_i64<R: Read>(reader: &mut R) -> Result<i64> {
    Ok(read_u64(reader)? as i64)
}

pub fn read_f32<R: Read>(reader: &mut R) -> Result<f32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

pub fn read_f64<R: Read>(reader: &mut R) -> Result<f64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

pub fn write_u16<W: Write>(writer: &mut W, value: u16) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

pub fn write_u32<W: Write>(writer: &mut W, value: u32) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

pub fn write_u64<W: Write>(writer: &mut W, value: u64) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

pub fn write_i64<W: Write>(writer: &mut W, value: i64) -> Result<()> {
    write_u64(writer, value as u64)
}

pub fn write_f32<W: Write>(writer: &mut W, value: f32) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

pub fn write_f64<W: Write>(writer: &mut W, value: f64) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn header_roundtrip() {
        let header = Header::new(42, 100);
        let mut buf = Vec::new();
        header.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(buf);
        let decoded = Header::read_from(&mut cursor).unwrap();
        assert_eq!(decoded, header);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut cursor = Cursor::new(b"NOPE".to_vec());
        let err = Header::read_from(&mut cursor).unwrap_err();
        assert!(matches!(err, SidecarError::InvalidMagic));
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"SCAR");
        write_u16(&mut buf, 99).unwrap();
        write_u16(&mut buf, 0).unwrap();
        write_u32(&mut buf, 0).unwrap();
        write_u32(&mut buf, 0).unwrap();

        let mut cursor = Cursor::new(buf);
        let err = Header::read_from(&mut cursor).unwrap_err();
        assert!(matches!(err, SidecarError::UnsupportedVersion(99)));
    }
}
