# sidecar-rs

Binary sidecar files (`.scar`) for media metadata. Each `.scar` file is a **pure CBOR map** at the root — self-describing, tool-agnostic, and parseable by any CBOR reader. Values include scalars, bytes, heterogeneous arrays, and nested maps.

## Quick start

```bash
make build
./target/release/sidecar create photo.jpg
./target/release/sidecar set photo.jpg --f64 photo.gps.latitude=37.7749
./target/release/sidecar set photo.jpg --json config='{"window":{"width":800,"height":600}}'
./target/release/sidecar get photo.jpg photo.gps.latitude
./target/release/sidecar list photo.jpg
./target/release/sidecar inspect photo.jpg
```

The `set`, `get`, `list`, and `inspect` commands accept either the media file
(e.g. `photo.jpg`) or the sidecar itself (`photo.scar`). When given a non-`.scar`
path, the matching `.scar` sidecar is resolved automatically by swapping the
extension.

Run the library example:

```bash
cargo run --example write_photo_metadata -p sidecar
```

## Python bindings (`sidecar_rs`)

The whole library is exposed to Python via PyO3 as the `sidecar_rs` module. The
project is installable from the repository root with `pip` or `uv pip`:

```bash
uv pip install .            # or: pip install .
```

```python
import sidecar_rs
from sidecar_rs import SidecarDocument, conventions

doc = SidecarDocument()
doc.set(conventions.GPS_LATITUDE, 37.7749)   # types are inferred
doc.set(conventions.TAGS, ["spring", "outdoor"])
doc.set("config", {"window": {"width": 800, "height": 600}})  # nested dicts
doc.set_media_basename("photo.jpg")

doc.to_path("photo.scar")
blob = doc.to_bytes()

restored = SidecarDocument.from_path("photo.scar")
print(restored[conventions.GPS_LATITUDE])     # 37.7749
print(dict(restored.entries()))
```

`SidecarDocument` behaves like a mapping (`len`, `in`, `doc[key]`, `del doc[key]`).
Generic `set()` infers CBOR types from Python (`None`, `bool`, `int`, `float`,
`str`, `bytes`, `list`, `dict`). Typed setters (`set_f32`, `set_u64`, ...) remain
for convenience but values round-trip as canonical CBOR types (`Integer`, `Float`).
Invalid data raises `sidecar_rs.SidecarError`.

A runnable walkthrough lives in [notebooks/sidecar_demo.ipynb](notebooks/sidecar_demo.ipynb).

### Python development

```bash
make py-install      # uv pip install . into venv_sidecar-rs
make py-test         # run the pytest suite
make py-wheel        # build a release wheel
make py-notebook-run # execute the demo notebook headlessly
```

## CBOR file format

A `.scar` sidecar is a single CBOR-encoded map:

```
{ "photo.gps.latitude": 37.7749, "_media.basename": "photo.jpg", ... }
```

There is no custom header or magic bytes — the file is valid CBOR that any CBOR
tool can read. Map keys are UTF-8 strings; values are standard CBOR types
(null, bool, integer, float, text, bytes, array, map).

### Media association

- Default path: `photo.jpg` → `photo.scar` (same directory, `.scar` extension)
- Optional catalog key `_media.basename` records the linked media filename

## Photography conventions

The library provides well-known keys under `sidecar::conventions::photo` (e.g. `photo.gps.latitude`, `photo.exposure.iso`). These are optional naming conventions, not enforced by the format.

## Workspace crates

| Crate | Purpose |
|-------|---------|
| `sidecar` | Library: read/write CBOR sidecar documents |
| `sidecar-cli` | Command-line tool (`sidecar` binary) |
| `sidecar-py` | PyO3 bindings for the `sidecar_rs` Python module |

## Development

```bash
make check    # cargo check
make test     # cargo test
make lint     # clippy
make format   # rustfmt --check
make build    # release build
make clean    # remove artifacts
```

## Limitations

- Map keys must be strings (CBOR text keys)
- Integer values are stored as CBOR integers (canonical `Integer` type on read)
- `set_f32` / `set_f64` both round-trip as CBOR floats; `set_i64` / `set_u64` as integers
- No concurrent write coordination (last writer wins)
- No XMP import/export
- Not compatible with v0.1.0 custom SCAR binary files
