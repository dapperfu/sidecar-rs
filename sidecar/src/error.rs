use thiserror::Error;

#[derive(Error, Debug)]
pub enum SidecarError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CBOR encode error: {0}")]
    Encode(String),

    #[error("CBOR decode error: {0}")]
    Decode(String),

    #[error("sidecar document must be a CBOR map at the root")]
    NotACborMap,

    #[error("map key must be a UTF-8 string, got {0:?}")]
    NonStringMapKey(String),

    #[error("empty key")]
    EmptyKey,

    #[error("integer value out of range for i128")]
    IntegerOutOfRange,
}

pub type Result<T> = std::result::Result<T, SidecarError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_includes_context() {
        let err = SidecarError::NotACborMap;
        assert!(err.to_string().contains("CBOR map"));
    }
}
