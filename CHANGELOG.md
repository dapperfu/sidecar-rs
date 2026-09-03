# Changelog

All notable changes to **sidecar-rs** are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
The workspace crates (`sidecar`, `sidecar-cli`, `sidecar-py` / Python package
`sidecar-rs`) share a single version from the root `Cargo.toml`.

## [0.2.4] - 2026-09-02

Patch release for **locked sidecar edits** (workspace `0.2.2` + `0.0.2`).
Leftover `{path}.lock` files are gone after every `update_path` call, the
locked write path is faster, lock waits are bounded at **10 seconds**, and a
vault-scale lock/unlock benchmark is included.

This is an **on-disk compatible** patch. Existing CBOR `.scar` files from 0.2.x
do not need conversion. Callers of `SidecarDocument::update_path` keep working;
Python gains an optional `lock_timeout_s` argument (default `10.0`) and a
`LockTimeout` exception.

### Why this release exists

`SidecarDocument::update_path` (added in 0.2.1) serializes concurrent writers
with an exclusive `flock` on a sibling file named `{sidecar}.lock`. That is
the right shape for this format: the sidecar itself is replaced with
`rename(2)`, so locking the data file inode is not enough. The 0.2.1
implementation opened and locked `{path}.lock`, loaded the latest CBOR map,
ran the updater, wrote a temp file, renamed it into place, and then dropped
the lock fd.

Dropping the fd **releases** the advisory lock. It does **not** remove the
directory entry. Every successful (and failed) edit therefore left a
zero-length `photo.scar.lock` next to the sidecar. In a photography vault that
is a real mess: directory listings and sync tools treat the lock as another
sidecar artifact, crash recovery looks like a writer is still running, and
operators cannot tell a live lock from a leftover.

The same path blocked forever if another writer never released the lock, and
it was slower than it needed to be for high-frequency merges:

1. The locked write reused `SidecarDocument::to_path`, which `fsync`s the
   entire temp file (`sync_all`) before `rename`. That is appropriate for an
   explicit save. It is the wrong default inside a short critical section.
2. The Python binding held the interpreter GIL for the whole operation,
   including `flock` wait and disk I/O.
3. The binding cloned the in-memory document into the `PySidecarDocument`
   wrapper and cloned it back after the callback.
4. Every lock acquire called `create_dir_all` on the parent directory even
   when the vault folder already existed (expensive on large network trees).

### Fixed

- **Lock files are unlinked after every `update_path`**, including when the
  updater returns an error, when the sidecar cannot be decoded, and when the
  temp write fails. Cleanup is implemented in `ExclusiveLock::Drop`: the path
  is removed **while the flock is still held**, then the fd is closed.
- **Stale inode retry.** After lock acquisition, Unix builds compare `dev`/`ino`
  of the fd against the current directory entry. On mismatch or `ENOENT`,
  acquisition retries so waiters do not proceed on an unlinked lock file.
- **Failed writes still delete** `{path}.tmp.{pid}` and still drop the lock
  file.
- **`create_dir_all` runs only when writing** a sidecar, not on lock-only
  acquire. Vault-scale lock/unlock went from ~8 ms/file to ~98 µs/file on
  `/tun/pictures` after this change.

### Lock timeout (default 10s)

- `update_path` waits up to [`DEFAULT_LOCK_TIMEOUT`] (10 seconds) using
  non-blocking `flock` (`LOCK_EX | LOCK_NB`) and a 1 ms poll.
- `SidecarDocument::update_path_with_timeout` takes an explicit
  `std::time::Duration`.
- On timeout, Rust returns `SidecarError::LockTimeout { path, timeout }`.
  Python raises **`LockTimeout`** (subclass of `SidecarError`). The sidecar is
  not written. Catch it and retry later:

```python
from sidecar_rs import SidecarDocument, LockTimeout

try:
    SidecarDocument.update_path(path, updater, lock_timeout_s=10)
except LockTimeout:
    ...  # queue / retry
```

- `sidecar_rs.DEFAULT_LOCK_TIMEOUT_SECS` is `10.0`.

### Performance

- **Locked writes no longer `fsync`.** `update_path` encodes CBOR into a
  `Vec<u8>`, writes `{path}.tmp.{pid}`, then `rename`s. Explicit `to_path`
  still calls `sync_all`.
- **Python `update_path` detaches from the GIL** for lock wait, load, encode,
  and rename; re-attaches only for the user callback. The document is **moved**
  into the wrapper, not cloned.

### Benchmark and pinch points

Example: `make bench-locks` (default `DIR=/tun/pictures`). It lock/unlocks
`.scar` files only; it does **not** rewrite vault contents.

Measured on `/tun/pictures` (**99,479** sidecars, release build):

| n | lock time | µs/file | files/s |
|---|-----------|---------|---------|
| 1,000 | 0.096s | 96 | 10,410 |
| 10,000 | 0.955s | 96 | 10,470 |
| 99,479 | 9.773s | 98 | 10,179 |

Pinch on lock/unlock: **open+create lock file ~48%**, **unlink+close ~47%**,
`flock` ~0.8%, inode identity ~3%. Directory walk is ~1.1s for 99k files.
Identity `update_path` (read+write CBOR) on 200 temp copies is ~6.7 ms/file
(~70× slower than lock/unlock).

### API and compatibility

| Surface | Change |
|---------|--------|
| On-disk `.scar` (CBOR map) | Unchanged |
| `SidecarDocument::update_path` (Rust) | Same signature; 10s lock timeout |
| `update_path_with_timeout` | **New** |
| `SidecarDocument.update_path` (Python) | Optional `lock_timeout_s=10` |
| `LockTimeout` | **New** Python exception |
| `SidecarDocument::to_path` | Unchanged; still `fsync`s |
| `{path}.lock` lifetime | Created for the critical section, then removed |
| Windows | Lock file created and removed; `flock` still Unix-only |

Upgrade from 0.2.1 or 0.2.2: install **0.2.4**. Existing `update_path(path,
updater)` calls keep working. Optionally delete leftover `*.scar.lock` from
older runs. A lock file that persists after 0.2.4 is a writer still inside
`update_path` (or a crash during the critical section).

### Tests

- Rust: lock file removal, apply-failure cleanup, concurrent merges, lock
  timeout while another holder is active, lock/unlock helper
- Python: lock file absence, callback errors, `LockTimeout` on a held lock,
  `DEFAULT_LOCK_TIMEOUT_SECS == 10.0`

### Upgrade checklist

1. Install sidecar-rs **0.2.4**.
2. Catch `LockTimeout` in Python workers that previously could hang forever.
3. Optionally sweep leftover locks: `find . -name '*.scar.lock' -delete` only
   when no writers are running.
4. Keep using `update_path` for concurrent namespace merges; use `to_path`
   when you want a durable fsynced snapshot.

## [0.2.2] - 2026-06-05

- Symlink-aware sidecar resolution: when the media path is a symlink, look
  beside the **target** first, then beside the symlink. Writes go back to
  whichever `.scar` was found. `sidecar create` still writes beside the path
  given. Direct `.scar` inputs are used as-is.
- New `resolve_sidecar_path` in the library, CLI, and Python module
  (`sidecar_rs.resolve_sidecar_path`).
- Rust and pytest coverage for target-adjacent vs symlink-adjacent files.

## [0.2.1] - 2026-06-05

- `SidecarDocument::update_path` / `SidecarDocument.update_path`: exclusive
  `{path}.lock`, load, callback, atomic replace. Safe merge of independent
  namespaces (for example pose vs face) across processes.
- Python type stub for `update_path`.
- **Known issues (fixed in 0.2.4):** the lock file was not deleted after the
  edit; the locked write fsynced on every merge; lock wait was unbounded.

## [0.2.0] - 2026-06-04

- Breaking on-disk change: `.scar` is a **pure CBOR map** (ciborium). No custom
  SCAR header or magic. Not compatible with 0.1.0 files.
- Nested maps and heterogeneous arrays; photography convention keys unchanged
  in name.

## [0.1.0]

- Initial custom binary SCAR format (superseded by 0.2.0).

[0.2.4]: https://github.com/dapperfu/sidecar-rs/compare/v0.2.1...v0.2.4
[0.2.2]: https://github.com/dapperfu/sidecar-rs/compare/v0.2.1...39065d0
[0.2.1]: https://github.com/dapperfu/sidecar-rs/releases/tag/v0.2.1
[0.2.0]: https://github.com/dapperfu/sidecar-rs/releases/tag/v0.2.0
[0.1.0]: https://github.com/dapperfu/sidecar-rs/releases/tag/v0.1.0
