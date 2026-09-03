//! Locked read-modify-write for sidecar files (cross-process safe on Unix).

use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::document::SidecarDocument;
use crate::{Result, SidecarError};

/// Default time to wait for `{path}.lock` before `update_path` fails.
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Wall-clock breakdown of one lock/unlock cycle (for benchmarks).
#[derive(Debug, Default, Clone, Copy)]
pub struct LockPhases {
    pub open: Duration,
    pub flock: Duration,
    pub identity: Duration,
    pub release: Duration,
}

/// Apply `apply` to the document at `path`, loading the latest on-disk state under an
/// exclusive lock file (`{path}.lock`) and atomically replacing the sidecar on success.
///
/// Waits up to [`DEFAULT_LOCK_TIMEOUT`] (10s) for the lock. The lock file is unlinked
/// before the lock is released. The sidecar is written without `fsync` so the critical
/// section stays short.
pub fn update_path<P, F>(path: P, apply: F) -> Result<()>
where
    P: AsRef<Path>,
    F: FnMut(&mut SidecarDocument) -> Result<()>,
{
    update_path_with_timeout(path, DEFAULT_LOCK_TIMEOUT, apply)
}

/// Same as [`update_path`], but callers choose how long to wait for `{path}.lock`.
pub fn update_path_with_timeout<P, F>(path: P, timeout: Duration, mut apply: F) -> Result<()>
where
    P: AsRef<Path>,
    F: FnMut(&mut SidecarDocument) -> Result<()>,
{
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    with_exclusive_lock(path, timeout, || {
        let mut doc = match std::fs::metadata(path) {
            Ok(meta) if meta.is_file() && meta.len() > 0 => SidecarDocument::from_path(path)?,
            Ok(_) => SidecarDocument::new(),
            Err(err) if err.kind() == ErrorKind::NotFound => SidecarDocument::new(),
            Err(err) => return Err(err.into()),
        };

        apply(&mut doc)?;

        let tmp_path = temp_path_for(path);
        let write_result = write_sidecar_atomic(path, &tmp_path, &doc);
        if write_result.is_err() {
            let _ = std::fs::remove_file(&tmp_path);
        }
        write_result
    })
}

/// Hold `{sidecar}.lock` for the duration of `f`, then unlink and release.
pub fn with_exclusive_lock<P, F, T>(sidecar_path: P, timeout: Duration, f: F) -> Result<T>
where
    P: AsRef<Path>,
    F: FnOnce() -> Result<T>,
{
    let sidecar_path = sidecar_path.as_ref();
    let lock_path = lock_path_for(sidecar_path);
    let _lock = acquire_exclusive_lock(&lock_path, timeout)?;
    f()
}

/// Create, exclusive-lock, unlock, and delete `{path}.lock` without reading the sidecar.
pub fn lock_unlock_sidecar(path: &Path, timeout: Duration) -> Result<()> {
    with_exclusive_lock(path, timeout, || Ok(()))
}

/// Same as [`lock_unlock_sidecar`], returning per-phase timings.
pub fn lock_unlock_sidecar_phases(path: &Path, timeout: Duration) -> Result<LockPhases> {
    let lock_path = lock_path_for(path);
    let (lock, mut phases) = acquire_exclusive_lock_timed(&lock_path, timeout)?;
    let release_start = Instant::now();
    drop(lock);
    phases.release = release_start.elapsed();
    Ok(phases)
}

/// Exclusive flock plus path identity check. Unlink while the fd is still locked;
/// waiters that locked a stale inode retry against the current directory entry.
fn acquire_exclusive_lock(lock_path: &Path, timeout: Duration) -> Result<ExclusiveLock> {
    let (lock, _) = acquire_exclusive_lock_timed(lock_path, timeout)?;
    Ok(lock)
}

fn acquire_exclusive_lock_timed(
    lock_path: &Path,
    timeout: Duration,
) -> Result<(ExclusiveLock, LockPhases)> {
    let deadline = Instant::now() + timeout;
    let mut phases = LockPhases::default();
    loop {
        let open_start = Instant::now();
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        phases.open += open_start.elapsed();

        let flock_start = Instant::now();
        let acquired = try_lock_exclusive(&file)?;
        phases.flock += flock_start.elapsed();
        if !acquired {
            drop(file);
            let now = Instant::now();
            if now >= deadline {
                return Err(SidecarError::LockTimeout {
                    path: lock_path.to_path_buf(),
                    timeout,
                });
            }
            std::thread::sleep(LOCK_POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
            continue;
        }

        let identity_start = Instant::now();
        let current = lock_file_is_current(&file, lock_path)?;
        phases.identity += identity_start.elapsed();
        if current {
            return Ok((
                ExclusiveLock {
                    path: lock_path.to_path_buf(),
                    _file: file,
                },
                phases,
            ));
        }
    }
}

fn lock_file_is_current(file: &File, lock_path: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let fd_meta = file.metadata()?;
        match std::fs::metadata(lock_path) {
            Ok(path_meta) => {
                Ok(path_meta.dev() == fd_meta.dev() && path_meta.ino() == fd_meta.ino())
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err.into()),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        let _ = lock_path;
        Ok(true)
    }
}

struct ExclusiveLock {
    path: PathBuf,
    _file: File,
}

impl Drop for ExclusiveLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn write_sidecar_atomic(path: &Path, tmp_path: &Path, doc: &SidecarDocument) -> Result<()> {
    let mut buf = Vec::new();
    doc.to_writer(&mut buf)?;
    std::fs::write(tmp_path, buf)?;
    std::fs::rename(tmp_path, path)?;
    Ok(())
}

fn lock_path_for(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.lock", path.display()))
}

fn temp_path_for(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.tmp.{}", path.display(), std::process::id()))
}

#[cfg(unix)]
fn try_lock_exclusive(file: &File) -> Result<bool> {
    use std::os::unix::io::AsRawFd;

    loop {
        let fd = file.as_raw_fd();
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            return Ok(true);
        }
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(code) if code == libc::EINTR => continue,
            Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => return Ok(false),
            _ => return Err(err.into()),
        }
    }
}

#[cfg(not(unix))]
fn try_lock_exclusive(_file: &File) -> Result<bool> {
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn update_path_merges_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.scar");

        update_path(&path, |doc| {
            doc.set("pose.yolo.model", Value::Text("a".into()))?;
            Ok(())
        })
        .unwrap();

        update_path(&path, |doc| {
            doc.set("face.scrfd.version", Value::Text("1.0.0".into()))?;
            Ok(())
        })
        .unwrap();

        let doc = SidecarDocument::from_path(&path).unwrap();
        assert!(doc.get("pose.yolo.model").is_some());
        assert!(doc.get("face.scrfd.version").is_some());
    }

    #[test]
    fn concurrent_update_path_preserves_both_namespaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = Arc::new(dir.path().join("photo.scar"));
        let errors = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let path_pose = Arc::clone(&path);
            let errors_pose = Arc::clone(&errors);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    if update_path(path_pose.as_path(), |doc| {
                        doc.set("pose.yolo.model", Value::Text("yolo".into()))?;
                        Ok(())
                    })
                    .is_err()
                    {
                        errors_pose.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }));

            let path_face = Arc::clone(&path);
            let errors_face = Arc::clone(&errors);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    if update_path(path_face.as_path(), |doc| {
                        doc.set("face.scrfd.version", Value::Text("1.0.0".into()))?;
                        Ok(())
                    })
                    .is_err()
                    {
                        errors_face.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(errors.load(Ordering::Relaxed), 0);
        let doc = SidecarDocument::from_path(path.as_path()).unwrap();
        assert!(doc.get("pose.yolo.model").is_some());
        assert!(doc.get("face.scrfd.version").is_some());
        assert!(!lock_path_for(path.as_path()).exists());
    }

    #[test]
    fn update_path_removes_lock_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.scar");

        update_path(&path, |doc| {
            doc.set("k", Value::Integer(1))?;
            Ok(())
        })
        .unwrap();

        assert!(path.is_file());
        assert!(!lock_path_for(&path).exists());
        assert!(!temp_path_for(&path).exists());
    }

    #[test]
    fn update_path_removes_lock_file_when_apply_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.scar");

        let err = update_path(&path, |_| Err(crate::SidecarError::EmptyKey));
        assert!(err.is_err());
        assert!(!path.exists());
        assert!(!lock_path_for(&path).exists());
    }

    #[test]
    fn lock_timeout_when_another_holder_is_active() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.scar");
        update_path(&path, |doc| {
            doc.set("k", Value::Integer(1))?;
            Ok(())
        })
        .unwrap();

        let path_held = path.clone();
        let started = Arc::new(std::sync::Barrier::new(2));
        let started_holder = Arc::clone(&started);
        let handle = thread::spawn(move || {
            with_exclusive_lock(&path_held, Duration::from_secs(5), || {
                started_holder.wait();
                thread::sleep(Duration::from_millis(400));
                Ok(())
            })
        });
        started.wait();
        let err =
            update_path_with_timeout(&path, Duration::from_millis(80), |_| Ok(())).unwrap_err();
        match err {
            crate::SidecarError::LockTimeout { .. } => {}
            other => panic!("expected LockTimeout, got {other:?}"),
        }
        handle.join().unwrap().unwrap();
        assert!(!lock_path_for(&path).exists());
    }

    #[test]
    fn lock_unlock_sidecar_leaves_no_lock_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.scar");
        std::fs::write(&path, []).unwrap();
        lock_unlock_sidecar(&path, DEFAULT_LOCK_TIMEOUT).unwrap();
        assert!(!lock_path_for(&path).exists());
    }
}
