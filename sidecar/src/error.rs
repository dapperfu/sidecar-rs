use thiserror::Error;

#[derive(Error, Debug)]
pub enum SidecarError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid magic bytes; expected SCAR")]
    InvalidMagic,

    #[error("unsupported format version: {0}")]
    UnsupportedVersion(u16),

    #[error("catalog truncated or malformed")]
    InvalidCatalog,

    #[error("payload truncated or malformed")]
    InvalidPayload,

    #[error("invalid UTF-8 in key or string value")]
    InvalidUtf8,

    #[error("value kind mismatch: expected {expected}, got {actual}")]
    KindMismatch { expected: String, actual: String },

    #[error("payload reference out of bounds: offset {offset}, len {len}")]
    PayloadOutOfBounds { offset: u32, len: u32 },

    #[error("inline value exceeds maximum size of {max} bytes")]
    InlineTooLarge { max: usize },

    #[error("empty key")]
    EmptyKey,

    #[error("unsupported value kind tag: {0}")]
    UnknownKindTag(u8),
}

pub type Result<T> = std::result::Result<T, SidecarError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_includes_version() {
        let err = SidecarError::UnsupportedVersion(99);
        assert!(err.to_string().contains("99"));
    }
}
