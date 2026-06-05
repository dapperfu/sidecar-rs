//! Integration tests for symlink-aware sidecar path resolution.

use std::fs;
use std::os::unix::fs::symlink;

use sidecar::{resolve_sidecar_path, SidecarDocument, Value};

fn write_sidecar(path: &std::path::Path, key: &str, value: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut doc = SidecarDocument::new();
    doc.set(key, Value::Text(value.into())).unwrap();
    doc.to_path(path).unwrap();
}

#[test]
fn resolve_finds_target_adjacent_sidecar_for_symlink() {
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
    write_sidecar(&target_scar, "source", "target");

    let resolved = resolve_sidecar_path(&link_media);
    assert_eq!(resolved, target_scar);

    let doc = SidecarDocument::from_path(&resolved).unwrap();
    assert_eq!(
        doc.get("source"),
        Some(&Value::Text("target".into()))
    );
}

#[test]
fn resolve_finds_symlink_adjacent_sidecar_when_target_missing() {
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
    write_sidecar(&local_scar, "source", "local");

    assert_eq!(resolve_sidecar_path(&link_media), local_scar);
}

#[test]
fn target_adjacent_wins_when_both_sidecars_exist() {
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
    let local_scar = album.join("link.scar");
    write_sidecar(&target_scar, "winner", "target");
    write_sidecar(&local_scar, "winner", "local");

    let resolved = resolve_sidecar_path(&link_media);
    assert_eq!(resolved, target_scar);

    let doc = SidecarDocument::from_path(&resolved).unwrap();
    assert_eq!(
        doc.get("winner"),
        Some(&Value::Text("target".into()))
    );
}

#[test]
fn write_updates_sidecar_where_it_was_found() {
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
    write_sidecar(&target_scar, "version", "1");

    let resolved = resolve_sidecar_path(&link_media);
    let mut doc = SidecarDocument::from_path(&resolved).unwrap();
    doc.set("version", Value::Text("2".into())).unwrap();
    doc.to_path(&resolved).unwrap();

    let target_doc = SidecarDocument::from_path(&target_scar).unwrap();
    assert_eq!(
        target_doc.get("version"),
        Some(&Value::Text("2".into()))
    );

    let local_scar = album.join("link.scar");
    assert!(!local_scar.exists());
}

#[test]
fn non_symlink_behavior_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let media = dir.path().join("photo.jpg");
    fs::write(&media, b"data").unwrap();

    let scar = dir.path().join("photo.scar");
    write_sidecar(&scar, "k", "v");

    assert_eq!(resolve_sidecar_path(&media), scar);
}

#[test]
fn broken_symlink_falls_back_to_local_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let album = dir.path().join("album");
    fs::create_dir_all(&album).unwrap();

    let link_media = album.join("link.jpg");
    symlink(dir.path().join("missing.jpg"), &link_media).unwrap();

    assert_eq!(resolve_sidecar_path(&link_media), album.join("link.scar"));
}

#[test]
fn direct_scar_path_skips_alternate_lookup() {
    let dir = tempfile::tempdir().unwrap();
    let scar = dir.path().join("photo.scar");
    write_sidecar(&scar, "k", "v");

    assert_eq!(resolve_sidecar_path(&scar), scar);
}
