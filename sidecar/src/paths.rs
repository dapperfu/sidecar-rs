//! Symlink-aware sidecar path resolution.

use std::path::{Path, PathBuf};

use crate::{sidecar_path_for_media, SIDECAR_EXTENSION};

/// Resolve the sidecar path for a user-supplied media or sidecar path.
///
/// When the input is a media symlink, checks for an existing sidecar beside the
/// link target first, then beside the symlink. When no sidecar exists yet, returns
/// the symlink-adjacent path (first candidate for non-`.scar` media input).
pub fn resolve_sidecar_path(path: &Path) -> PathBuf {
    let candidates = sidecar_candidates_for_input(path);
    find_existing_sidecar(&candidates).unwrap_or_else(|| {
        candidates
            .first()
            .cloned()
            .unwrap_or_else(|| path.to_path_buf())
    })
}

/// Build ordered sidecar path candidates for lookup.
pub fn sidecar_candidates_for_input(path: &Path) -> Vec<PathBuf> {
    if is_sidecar_path(path) {
        return vec![path.to_path_buf()];
    }

    let local = sidecar_path_for_media(path);
    if let Some(target) = symlink_target(path) {
        if std::fs::metadata(path).is_ok() {
            let target_sidecar = sidecar_path_for_media(&target);
            if target_sidecar != local {
                return vec![target_sidecar, local];
            }
        }
    }

    vec![local]
}

/// Return the first existing non-empty sidecar from `candidates`.
pub fn find_existing_sidecar(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|candidate| sidecar_exists(candidate)).cloned()
}

fn is_sidecar_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(SIDECAR_EXTENSION))
}

fn sidecar_exists(path: &Path) -> bool {
    path.metadata()
        .ok()
        .is_some_and(|meta| meta.is_file() && meta.len() > 0)
}

/// If `path` is a symlink, return the resolved target path.
fn symlink_target(path: &Path) -> Option<PathBuf> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_symlink() {
        return None;
    }

    let link_target = std::fs::read_link(path).ok()?;
    Some(if link_target.is_absolute() {
        link_target
    } else {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        parent.join(link_target)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn write_sidecar(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"\xa1\x01a\x01").unwrap();
    }

    #[test]
    fn non_symlink_returns_local_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("photo.jpg");
        fs::write(&media, b"data").unwrap();

        let resolved = resolve_sidecar_path(&media);
        assert_eq!(resolved, dir.path().join("photo.scar"));
    }

    #[test]
    fn direct_scar_path_is_single_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let scar = dir.path().join("photo.scar");
        write_sidecar(&scar);

        assert_eq!(resolve_sidecar_path(&scar), scar);
    }

    #[test]
    fn finds_target_adjacent_sidecar_for_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        let album = dir.path().join("album");
        fs::create_dir_all(&vault).unwrap();
        fs::create_dir_all(&album).unwrap();

        let target_media = vault.join("photo.jpg");
        fs::write(&target_media, b"data").unwrap();
        let link_media = album.join("link.jpg");
        symlink(&target_media, &link_media).unwrap();

        let target_scar = vault.join("photo.scar");
        write_sidecar(&target_scar);

        assert_eq!(resolve_sidecar_path(&link_media), target_scar);
    }

    #[test]
    fn finds_symlink_adjacent_sidecar_when_target_missing() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        let album = dir.path().join("album");
        fs::create_dir_all(&vault).unwrap();
        fs::create_dir_all(&album).unwrap();

        let target_media = vault.join("photo.jpg");
        fs::write(&target_media, b"data").unwrap();
        let link_media = album.join("link.jpg");
        symlink(&target_media, &link_media).unwrap();

        let local_scar = album.join("link.scar");
        write_sidecar(&local_scar);

        assert_eq!(resolve_sidecar_path(&link_media), local_scar);
    }

    #[test]
    fn target_adjacent_wins_when_both_exist() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        let album = dir.path().join("album");
        fs::create_dir_all(&vault).unwrap();
        fs::create_dir_all(&album).unwrap();

        let target_media = vault.join("photo.jpg");
        fs::write(&target_media, b"data").unwrap();
        let link_media = album.join("link.jpg");
        symlink(&target_media, &link_media).unwrap();

        write_sidecar(&vault.join("photo.scar"));
        write_sidecar(&album.join("link.scar"));

        assert_eq!(resolve_sidecar_path(&link_media), vault.join("photo.scar"));
    }

    #[test]
    fn broken_symlink_falls_back_to_local_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let album = dir.path().join("album");
        fs::create_dir_all(&album).unwrap();

        let link_media = album.join("link.jpg");
        symlink(dir.path().join("missing.jpg"), &link_media).unwrap();

        assert_eq!(resolve_sidecar_path(&link_media), album.join("link.scar"));
    }
}
