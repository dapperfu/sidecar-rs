# sidecar-rs

Binary sidecar files (`.scar`) for media metadata. SCAR (Sidecar Archive) v1 stores a typed catalog dictionary plus an optional binary payload arena — fast to parse, native IEEE-754 floats, no JSON/XML overhead.

## Quick start

```bash
make build
./target/release/sidecar create photo.jpg
./target/release/sidecar set photo.scar --f64 photo.gps.latitude=37.7749
./target/release/sidecar get photo.scar photo.gps.latitude
./target/release/sidecar list photo.scar
./target/release/sidecar inspect photo.scar
```

Run the library example:

```bash
cargo run --example write_photo_metadata -p sidecar
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
