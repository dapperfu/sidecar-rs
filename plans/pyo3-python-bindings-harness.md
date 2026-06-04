# PyO3 Python Bindings Harness for sidecar-rs

## Goal

Expose the entire `sidecar` library to Python as the `sidecar_rs` module, via a dedicated PyO3 crate built with maturin. The core `sidecar` crate stays PyO3-free; all binding code lives in a new `sidecar-py` workspace member. The project is installable from the repository root with `pip install .` / `uv pip install .`.

## Confirmed decisions

- Import name: `import sidecar_rs`
- Type mapping: auto-infer for a generic `set()`, plus explicit typed setters for precision
- ABI: `abi3-py310` (one wheel for Python 3.10+); PyO3 0.28
- Deliverables: `.pyi` stubs, pytest suite, Jupyter notebook, Makefile targets, README, exposed photo conventions
- Version: 0.1.0 everywhere; single annotated git tag `v0.1.0`

## Layout

```
sidecar-rs/                      # repo root
├── pyproject.toml               # maturin build backend; root entry point for pip/uv pip
├── Cargo.toml                   # cargo workspace (adds sidecar-py member)
├── python/
│   └── sidecar_rs/
│       ├── __init__.py
│       ├── __init__.pyi
│       ├── conventions.py
│       ├── conventions.pyi
│       └── py.typed
├── tests/
│   └── test_sidecar.py          # pytest (Python)
├── notebooks/
│   └── sidecar_demo.ipynb       # full read/write example
└── sidecar-py/                  # PyO3 binding crate
    ├── Cargo.toml
    └── src/
        └── lib.rs               # PyO3 module _sidecar_rs
```

## Crate wiring

Root `Cargo.toml`: add `sidecar-py` member and pyo3 to `[workspace.dependencies]`
(`pyo3 = { version = "0.28", features = ["abi3-py310"] }`).

`sidecar-py/Cargo.toml`: `[lib] name = "_sidecar_rs"`, `crate-type = ["cdylib"]`, deps on
`sidecar` (path) and `pyo3` (workspace). `extension-module` feature gated so
`cargo build --workspace` still links.

## maturin / pyproject (root-level)

Root `pyproject.toml`, build-backend maturin, `module-name = "sidecar_rs._sidecar_rs"`,
`python-source = "python"`, `manifest-path = "sidecar-py/Cargo.toml"`,
`features = ["extension-module"]`, `dynamic = ["version"]`.

Install flows from root: `uv pip install .`, `uv pip install -e .` / `maturin develop`,
`maturin build --release` for wheels.

## Public Python API (`SidecarDocument`)

Construction (`SidecarDocument()`, `from_path`, `from_bytes`), serialization
(`to_path`, `to_bytes`), generic `set`/`get`/`remove`/`keys`/`entries`, typed setters
(`set_null/set_bool/set_i64/set_u64/set_f32/set_f64/set_str/set_bytes/set_array`),
media helpers, dunders (`__len__/__contains__/__getitem__/__setitem__/__delitem__/__repr__`),
`header()`.

## Value conversion

- Python -> Value (infer): None->Null, bool->Bool (before int), int->I64 (or U64 if needed; else OverflowError), float->F64, str->String, bytes/bytearray->Bytes, list->Array (homogeneous; empty -> element_kind Null).
- Value -> Python: Null->None, Bool->bool, I64/U64->int, F32/F64->float, String->str, Bytes->bytes, Array->list.
- Caveat: generic `set(float)` stores F64; use `set_f32` for F32.

## Errors

`create_exception!(_sidecar_rs, SidecarError, PyException)`; map `sidecar::SidecarError` -> `SidecarError`.

## Conventions

Rust pymodule exports photo key constants; `python/sidecar_rs/conventions.py` re-exports
clean names (`GPS_LATITUDE`, ...), single source of truth in Rust.

## Tooling (uv + Makefile)

`py-venv`, `py-install` (`uv pip install .`), `py-build` (`maturin develop`), `py-test`
(`pytest tests`), `py-wheel`, `py-notebook`, `py-notebook-run` (nbconvert `--execute`).
`.gitignore`: `__pycache__/`, `*.so`, `venv_*/`, `dist/`, `*.egg-info/`, `.ipynb_checkpoints/`.

## Tests

Roundtrips, typed setters, arrays, dunders, conventions, error cases.

## Jupyter notebook

`notebooks/sidecar_demo.ipynb`: write (inference + typed + conventions), persist via
`to_path` and `to_bytes`, read back via `from_path`/`from_bytes` with asserts, inspect,
error handling, cleanup. Verified via `nbconvert --execute`.

## Cleanup, sanity check, release tag (v0.1.0)

- Cleanup: resolve Cargo.lock/.gitignore inconsistency (commit lock, drop from .gitignore),
  remove dead no-op placeholders, complete .gitignore, no tracked artifacts.
- Sanity check: Rust fmt/clippy/test/build; Python install/pytest/notebook execute.
- Tag: confirm 0.1.0; `git tag -a v0.1.0`; push if remote exists (none currently).

## Implementation Status

- [x] Plan saved
- [x] Workspace wiring
- [x] Crate bootstrap
- [x] Bindings
- [x] pyproject
- [x] Python package
- [x] Tests
- [x] Notebook
- [x] Tooling and docs
- [x] Cleanup
- [x] Sanity check
- [x] Release tag v0.1.0
