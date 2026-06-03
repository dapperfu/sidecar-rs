# Sidecar-rs: Binary Media Sidecar Format (Rust)

## Context

`sidecar-rs` is currently an empty repo (only `.cursor/rules/`). This plan bootstraps the full project from scratch.

**Confirmed scope:**
- **Data model:** Generic typed container first, plus well-known photography key conventions (e.g. `photo.gps.latitude`).
- **Deliverables:** Library + CLI + examples/tests.

## Format Design: SCAR v1

Custom binary format (not JSON/XML) for native IEEE-754 floats, compact layout, and fast parse/write.

### File layout

| Section | Purpose |
|---------|---------|
| **Header** | Magic `b"SCAR"`, format version `u16`, flags `u16`, catalog byte length `u32`, payload byte length `u32` |
| **Catalog** | Top-level **dictionary** describing every stored field |
| **Payload** | Contiguous binary blob arena referenced by catalog entries |

### Catalog entry (one per key)

Each entry in the catalog dictionary:

- **key** — UTF-8 string
- **kind** — `u8` type tag: `Null`, `Bool`, `I64`, `U64`, `F32`, `F64`, `String`, `Bytes`, `Array { element_kind }`
- **storage** — either **inline** or **payload ref** `{ offset: u32, len: u32 }`

Scalars use fixed-width native little-endian encoding. Strings/bytes are length-prefixed (`u32` len + bytes).

### Sidecar ↔ media association

- **Filename convention:** `<media_basename>.scar` adjacent to the media file.
- **Optional catalog key:** `_media.basename` records linked media basename.

## Implementation Status

- [x] Plan saved
- [x] Workspace skeleton
- [x] Library format codec
- [x] Photography conventions
- [x] CLI
- [x] Examples and integration tests
- [x] README and polish
