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
    SidecarEdit,
    SidecarError,
    clear_sidecar_lock,
    resolve_sidecar_path,
)

__all__ = [
    "DEFAULT_LOCK_TIMEOUT_SECS",
    "SidecarDocument",
    "SidecarEdit",
    "SidecarError",
    "LockTimeout",
    "MEDIA_BASENAME_KEY",
    "SIDECAR_EXTENSION",
    "clear_sidecar_lock",
    "conventions",
    "resolve_sidecar_path",
]
