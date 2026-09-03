//! Locked read-modify-write for sidecar files (cross-process safe on Unix).

use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::document::SidecarDocument;
use crate::Result;

/// Apply `apply` to the document at `path`, loading the latest on-disk state under an
/// exclusive lock file (`{path}.lock`) and atomically replacing the sidecar on success.
///
/// The lock file is unlinked before the lock is released so successful (and failed)
/// edits do not leave `{path}.lock` behind. The sidecar is written without `fsync`
/// so the critical section stays short.
pub fn update_path<P, F>(path: P, mut apply: F) -> Result<()>
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

    let lock_path = lock_path_for(path);
    let _lock = acquire_exclusive_lock(&lock_path)?;

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
}

/// Exclusive flock plus path identity check. Unlink while the fd is still locked;
/// waiters that locked a stale inode retry against the current directory entry.
fn acquire_exclusive_lock(lock_path: &Path) -> Result<ExclusiveLock> {
    loop {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock_exclusive(&file)?;
        if lock_file_is_current(&file, lock_path)? {
            return Ok(ExclusiveLock {
                path: lock_path.to_path_buf(),
                _file: file,
            });
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
fn lock_exclusive(file: &File) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    let fd = file.as_raw_fd();
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn lock_exclusive(_file: &File) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

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
}
