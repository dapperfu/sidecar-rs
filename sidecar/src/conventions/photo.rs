//! Well-known photography metadata key conventions.
//!
//! These keys are not enforced by the SCAR format; they provide a shared vocabulary
//! for photography-specific sidecar fields.

/// GPS latitude in decimal degrees (WGS-84).
pub const GPS_LATITUDE: &str = "photo.gps.latitude";

/// GPS longitude in decimal degrees (WGS-84).
pub const GPS_LONGITUDE: &str = "photo.gps.longitude";

/// Exposure time in seconds (e.g. 0.008 for 1/125s).
pub const EXPOSURE_TIME_SEC: &str = "photo.exposure.time_sec";

/// ISO sensitivity value.
pub const EXPOSURE_ISO: &str = "photo.exposure.iso";

/// Lens focal length in millimeters.
pub const LENS_FOCAL_LENGTH_MM: &str = "photo.lens.focal_length_mm";

/// User rating (typically 0–5).
pub const RATING: &str = "photo.rating";

/// Tags associated with the image (`Array<String>`).
pub const TAGS: &str = "photo.tags";
