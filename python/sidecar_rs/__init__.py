"""Python bindings for the SCAR binary sidecar format.

The native extension module (``_sidecar_rs``) is implemented in Rust via PyO3.
This package re-exports its public API and the photography key conventions.
"""

from __future__ import annotations

from . import conventions
from ._sidecar_rs import (
    DEFAULT_LOCK_TIMEOUT_SECS,
    MEDIA_BASENAME_KEY,
    SIDECAR_EXTENSION,
    LockTimeout,
    SidecarDocument,
    SidecarError,
    resolve_sidecar_path,
)

__all__ = [
    "DEFAULT_LOCK_TIMEOUT_SECS",
    "SidecarDocument",
    "SidecarError",
    "LockTimeout",
    "MEDIA_BASENAME_KEY",
    "SIDECAR_EXTENSION",
    "conventions",
    "resolve_sidecar_path",
]
