"""Python bindings for the SCAR binary sidecar format.

The native extension module (``_sidecar_rs``) is implemented in Rust via PyO3.
This package re-exports its public API and the photography key conventions.
"""

from __future__ import annotations

from . import conventions
from ._sidecar_rs import (
    MEDIA_BASENAME_KEY,
    SIDECAR_EXTENSION,
    SidecarDocument,
    SidecarError,
)

__all__ = [
    "SidecarDocument",
    "SidecarError",
    "MEDIA_BASENAME_KEY",
    "SIDECAR_EXTENSION",
    "conventions",
]
