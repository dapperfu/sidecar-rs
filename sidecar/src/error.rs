use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SidecarError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "timed out after {:.3}s waiting for exclusive lock on {}",
        timeout.as_secs_f64(),
        path.display()
    )]
    LockTimeout { path: PathBuf, timeout: Duration },

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
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn error_display_includes_context() {
        let err = SidecarError::NotACborMap;
        assert!(err.to_string().contains("CBOR map"));
    }

    #[test]
    fn lock_timeout_display_includes_path_and_seconds() {
        let err = SidecarError::LockTimeout {
            path: PathBuf::from("/tmp/photo.scar.lock"),
            timeout: Duration::from_secs(10),
        };
        let text = err.to_string();
        assert!(text.contains("10"));
        assert!(text.contains("photo.scar.lock"));
    }
}
