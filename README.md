# sidecar-rs

Binary sidecar files (`.scar`) for media metadata. SCAR (Sidecar Archive) v1 stores a typed catalog dictionary plus an optional binary payload arena — fast to parse, native IEEE-754 floats, no JSON/XML overhead.

## Quick start

```bash
make build
./target/release/sidecar create photo.jpg
./target/release/sidecar set photo.jpg --f64 photo.gps.latitude=37.7749
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
doc.set_f32(conventions.LENS_FOCAL_LENGTH_MM, 50.0)  # typed setter for precision
doc.set(conventions.TAGS, ["spring", "outdoor"])
doc.set_media_basename("photo.jpg")

doc.to_path("photo.scar")
blob = doc.to_bytes()

restored = SidecarDocument.from_path("photo.scar")
print(restored[conventions.GPS_LATITUDE])     # 37.7749
print(dict(restored.entries()))
```

`SidecarDocument` behaves like a mapping (`len`, `in`, `doc[key]`, `del doc[key]`).
Generic `set()` infers the SCAR type (`bool`, `int` -> `I64`/`U64`, `float` -> `F64`,
`str`, `bytes`, homogeneous `list`); typed setters (`set_f32`, `set_u64`, ...) give
exact control. Invalid data raises `sidecar_rs.SidecarError`.

A runnable walkthrough lives in [notebooks/sidecar_demo.ipynb](notebooks/sidecar_demo.ipynb).

### Python development

```bash
make py-install      # uv pip install . into venv_sidecar-rs
make py-test         # run the pytest suite
make py-wheel        # build a release wheel
make py-notebook-run # execute the demo notebook headlessly
```

## SCAR v1 file layout

```
+--------+------------------+-----------+
| Header | Catalog (dict)   | Payload   |
+--------+------------------+-----------+
```

### Header (16 bytes, little-endian)

| Offset | Size | Field |
|--------|------|-------|
| 0 | 4 | Magic `SCAR` |
| 4 | 2 | Format version (`1`) |
| 6 | 2 | Flags (reserved, `0`) |
| 8 | 4 | Catalog section length |
| 12 | 4 | Payload section length |

### Catalog

A length-prefixed dictionary. Each entry contains:

- Key (UTF-8 string)
- Value kind tag (`Null`, `Bool`, `I64`, `U64`, `F32`, `F64`, `String`, `Bytes`, `Array`)
- Element kind tag (for arrays)
- Storage mode: inline scalar/data, or payload reference `{offset, len}`

Strings and byte blobs larger than 64 bytes are stored in the payload arena.

### Media association

- Default path: `photo.jpg` → `photo.scar` (same directory, `.scar` extension)
- Optional catalog key `_media.basename` records the linked media filename

## Photography conventions

The library provides well-known keys under `sidecar::conventions::photo` (e.g. `photo.gps.latitude`, `photo.exposure.iso`). These are optional naming conventions, not enforced by the format.

## Workspace crates

| Crate | Purpose |
|-------|---------|
| `sidecar` | Library: read/write SCAR documents |
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

## Limitations (v1)

- Flat catalog only (no nested maps)
- Homogeneous arrays only
- No concurrent write coordination (last writer wins)
- No XMP import/export
