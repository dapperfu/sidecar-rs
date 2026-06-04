"""Well-known photography metadata key conventions.

These keys are not enforced by the SCAR format; they provide a shared vocabulary
for photography-specific sidecar fields. The values are defined once in Rust and
re-exported here under clean names.
"""

from __future__ import annotations

from ._sidecar_rs import (
    PHOTO_EXPOSURE_ISO as EXPOSURE_ISO,
    PHOTO_EXPOSURE_TIME_SEC as EXPOSURE_TIME_SEC,
    PHOTO_GPS_LATITUDE as GPS_LATITUDE,
    PHOTO_GPS_LONGITUDE as GPS_LONGITUDE,
    PHOTO_LENS_FOCAL_LENGTH_MM as LENS_FOCAL_LENGTH_MM,
    PHOTO_RATING as RATING,
    PHOTO_TAGS as TAGS,
)

__all__ = [
    "GPS_LATITUDE",
    "GPS_LONGITUDE",
    "EXPOSURE_TIME_SEC",
    "EXPOSURE_ISO",
    "LENS_FOCAL_LENGTH_MM",
    "RATING",
    "TAGS",
]
