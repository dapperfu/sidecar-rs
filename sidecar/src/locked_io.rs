//! Locked read-modify-write for sidecar files (cross-process safe on Unix).

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use crate::document::SidecarDocument;
use crate::Result;

/// Apply `apply` to the document at `path`, loading the latest on-disk state under an
/// exclusive lock file (`{path}.lock`) and atomically replacing the sidecar on success.
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
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)?;

    lock_exclusive(&lock_file)?;

    let mut doc = if path.is_file() && path.metadata()?.len() > 0 {
        SidecarDocument::from_path(path)?
    } else {
        SidecarDocument::new()
    };

    apply(&mut doc)?;

    let tmp_path = temp_path_for(path);
    let write_result = (|| {
        doc.to_path(&tmp_path)?;
        std::fs::rename(&tmp_path, path)?;
        Ok::<(), crate::SidecarError>(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    write_result
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
    }
}
