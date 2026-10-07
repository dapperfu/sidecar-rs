//! CBOR sidecar format for media metadata.

pub mod conventions;
pub mod document;
pub mod error;
pub mod locked_io;
pub mod paths;
pub mod value;

pub use document::SidecarDocument;
pub use error::{Result, SidecarError};
pub use locked_io::{
    clear_lock, lock_path_for, lock_unlock_sidecar, lock_unlock_sidecar_phases, open_for_edit,
    save_sidecar,
    update_path, update_path_with_timeout, with_exclusive_lock, ExclusiveLock, LockPhases,
    DEFAULT_LOCK_TIMEOUT,
};
pub use paths::resolve_sidecar_path;
pub use value::Value;

/// Reserved catalog key for linked media basename.
pub const MEDIA_BASENAME_KEY: &str = "_media.basename";

/// Default sidecar file extension.
pub const SIDECAR_EXTENSION: &str = "scar";

/// Derive sidecar path from a media file path (`photo.jpg` → `photo.scar`).
pub fn sidecar_path_for_media(media_path: &std::path::Path) -> std::path::PathBuf {
    media_path.with_extension(SIDECAR_EXTENSION)
}
