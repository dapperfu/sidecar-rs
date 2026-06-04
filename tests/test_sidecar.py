"""Pytest suite for the sidecar_rs Python bindings."""

from __future__ import annotations

import math
from pathlib import Path

import pytest

import sidecar_rs
from sidecar_rs import SidecarDocument, SidecarError, conventions


def test_scalar_roundtrip_via_bytes() -> None:
    doc = SidecarDocument()
    doc.set_null("null_field")
    doc.set("bool_field", True)
    doc.set("i64_field", -42)
    doc.set("f64_field", 0.008)
    doc.set("str_field", "hello")
    doc.set("bytes_field", b"\x00\x01\x02")

    restored = SidecarDocument.from_bytes(doc.to_bytes())

    assert restored.get("null_field") is None
    assert restored.get("bool_field") is True
    assert restored.get("i64_field") == -42
    assert restored.get("f64_field") == 0.008
    assert restored.get("str_field") == "hello"
    assert restored.get("bytes_field") == b"\x00\x01\x02"


def test_bool_is_not_treated_as_int() -> None:
    doc = SidecarDocument()
    doc.set("flag", True)
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    value = restored.get("flag")
    assert value is True
    assert isinstance(value, bool)


def test_typed_setters_store_canonical_cbor_types() -> None:
    doc = SidecarDocument()
    doc.set_f32("f32", 0.5)
    doc.set_f64("f64", 0.5)
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    assert math.isclose(restored["f32"], 0.5)
    assert restored["f64"] == 0.5


def test_u64_large_value() -> None:
    big = 2**63 + 7
    doc = SidecarDocument()
    doc.set("big", big)
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    assert restored["big"] == big


def test_overflow_beyond_i128_raises() -> None:
    doc = SidecarDocument()
    with pytest.raises(OverflowError):
        doc.set("too_big", 2**128)


def test_string_array_roundtrip() -> None:
    doc = SidecarDocument()
    doc.set("tags", ["spring", "outdoor", "macro"])
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    assert restored["tags"] == ["spring", "outdoor", "macro"]


def test_heterogeneous_array_roundtrip() -> None:
    doc = SidecarDocument()
    doc.set("mixed", [1, "two", 3.0])
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    assert restored["mixed"] == [1, "two", 3.0]


def test_nested_dict_roundtrip() -> None:
    doc = SidecarDocument()
    doc.set("config", {"window": {"width": 800, "height": 600}, "enabled": True})
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    assert restored["config"] == {
        "window": {"width": 800, "height": 600},
        "enabled": True,
    }


def test_set_map_explicit() -> None:
    doc = SidecarDocument()
    doc.set_map("meta", {"a": 1, "b": "two"})
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    assert restored["meta"] == {"a": 1, "b": "two"}


def test_empty_array_roundtrip() -> None:
    doc = SidecarDocument()
    doc.set("empty", [])
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    assert restored["empty"] == []


def test_mapping_dunders() -> None:
    doc = SidecarDocument()
    doc["a"] = 1
    doc["b"] = "two"
    assert len(doc) == 2
    assert "a" in doc
    assert doc["a"] == 1
    del doc["a"]
    assert "a" not in doc
    assert len(doc) == 1
    with pytest.raises(KeyError):
        _ = doc["missing"]
    with pytest.raises(KeyError):
        del doc["missing"]


def test_get_missing_returns_none() -> None:
    doc = SidecarDocument()
    assert doc.get("nope") is None


def test_remove_returns_value_or_none() -> None:
    doc = SidecarDocument()
    doc.set("x", 10)
    assert doc.remove("x") == 10
    assert doc.remove("x") is None


def test_keys_and_entries() -> None:
    doc = SidecarDocument()
    doc.set("k1", 1)
    doc.set("k2", "v")
    assert set(doc.keys()) == {"k1", "k2"}
    assert doc.entries() == {"k1": 1, "k2": "v"}


def test_media_basename() -> None:
    doc = SidecarDocument()
    doc.set_media_basename("IMG_1234.jpg")
    restored = SidecarDocument.from_bytes(doc.to_bytes())
    assert restored.media_basename() == "IMG_1234.jpg"


def test_header_fields() -> None:
    doc = SidecarDocument()
    doc.set("k", 1)
    header = doc.header()
    assert header["format"] == "cbor"
    assert header["entry_count"] == 1
    assert header["byte_len"] > 0


def test_file_roundtrip(tmp_path: Path) -> None:
    path = tmp_path / "photo.scar"
    doc = SidecarDocument()
    doc.set(conventions.GPS_LATITUDE, 37.7749)
    doc.set(conventions.EXPOSURE_ISO, 400)
    doc.to_path(path)

    restored = SidecarDocument.from_path(path)
    assert restored[conventions.GPS_LATITUDE] == 37.7749
    assert restored[conventions.EXPOSURE_ISO] == 400


def test_photography_conventions_values() -> None:
    assert conventions.GPS_LATITUDE == "photo.gps.latitude"
    assert conventions.EXPOSURE_ISO == "photo.exposure.iso"
    assert conventions.TAGS == "photo.tags"


def test_bad_bytes_raise_sidecar_error() -> None:
    with pytest.raises(SidecarError):
        SidecarDocument.from_bytes(b"not valid cbor")


def test_repr() -> None:
    doc = SidecarDocument()
    doc.set("k", 1)
    assert repr(doc) == "SidecarDocument(entries=1)"


def test_module_constants() -> None:
    assert sidecar_rs.SIDECAR_EXTENSION == "scar"
    assert sidecar_rs.MEDIA_BASENAME_KEY == "_media.basename"
