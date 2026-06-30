use std::path::{Path, PathBuf};

use tempfile::tempdir;
use titania_core::{TargetProject, TargetProjectError, discover_target};

fn cargo_manifest(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

#[test]
fn target_project_public_api_reports_exact_shape_errors() {
    let empty = TargetProject::try_from_path(Path::new("")).unwrap_err();
    assert_eq!(empty, TargetProjectError::Empty);

    let relative = TargetProject::try_from_path(Path::new("relative/path")).unwrap_err();
    assert_eq!(relative, TargetProjectError::NonAbsolute("relative/path".to_owned()));
}

#[cfg(unix)]
#[test]
fn target_project_public_api_rejects_non_utf8_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let path = PathBuf::from(OsString::from_vec(vec![b'/', b't', b'm', b'p', b'/', 0xff]));
    let err = TargetProject::try_from_path(&path).unwrap_err();
    assert_eq!(err, TargetProjectError::NotUtf8);
}

#[test]
fn target_project_public_api_rejects_manifest_directory() {
    let tmp = tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("Cargo.toml")).unwrap();

    let err = TargetProject::try_from_path(tmp.path()).unwrap_err();
    assert_eq!(err, TargetProjectError::CargoTomlNotFile);
}

#[test]
fn discover_target_public_api_resolves_workspace_root_from_member_subdir() {
    let tmp = tempdir().unwrap();
    let workspace = tmp.path().join("workspace");
    let member_src = workspace.join("crates").join("app").join("src");
    std::fs::create_dir_all(&member_src).unwrap();
    std::fs::write(workspace.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/app\"]\n")
        .unwrap();
    std::fs::write(workspace.join("crates").join("app").join("Cargo.toml"), cargo_manifest("app"))
        .unwrap();

    let target = discover_target(&member_src).unwrap();
    assert_eq!(target.as_std_path(), workspace.as_path());
    assert_eq!(target.manifest_path().as_std_path(), workspace.join("Cargo.toml").as_path());
}

#[test]
fn discover_target_public_api_resolves_nested_single_crate_root() {
    let tmp = tempdir().unwrap();
    let crate_root = tmp.path().join("single");
    let nested = crate_root.join("src").join("bin");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(crate_root.join("Cargo.toml"), cargo_manifest("single")).unwrap();

    let target = discover_target(&nested).unwrap();
    assert_eq!(target.as_std_path(), crate_root.as_path());
}

#[test]
fn discover_target_public_api_reports_exact_errors() {
    let relative = discover_target(Path::new("relative")).unwrap_err();
    assert_eq!(relative, TargetProjectError::NonAbsolute("relative".to_owned()));

    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("missing");
    std::fs::create_dir_all(&missing).unwrap();
    let no_manifest = discover_target(&missing).unwrap_err();
    assert_eq!(no_manifest, TargetProjectError::NoCargoToml);

    let with_manifest_dir = tmp.path().join("manifest_dir");
    std::fs::create_dir_all(with_manifest_dir.join("Cargo.toml")).unwrap();
    let manifest_dir = discover_target(&with_manifest_dir).unwrap_err();
    assert_eq!(manifest_dir, TargetProjectError::CargoTomlNotFile);
}

#[test]
fn target_project_public_api_accepts_absolute_manifest_directory() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("crate");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("Cargo.toml"), cargo_manifest("crate")).unwrap();

    let target = TargetProject::try_from_path(&root).unwrap();

    assert_eq!(target.as_std_path(), root.as_path());
    assert_eq!(target.manifest_path().as_std_path(), root.join("Cargo.toml").as_path());
    assert_eq!(target.to_string(), root.display().to_string());
}

#[test]
fn target_project_public_api_rejects_nonexistent_directory() {
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("missing");

    let err = TargetProject::try_from_path(&missing).unwrap_err();

    assert_eq!(err, TargetProjectError::NotFound);
}

#[test]
fn target_project_public_api_rejects_file_as_root() {
    let tmp = tempdir().unwrap();
    let file = tmp.path().join("Cargo.toml");
    std::fs::write(&file, cargo_manifest("not_root")).unwrap();

    let err = TargetProject::try_from_path(&file).unwrap_err();

    assert_eq!(err, TargetProjectError::NotADirectory);
}

#[test]
fn target_project_public_api_rejects_missing_manifest() {
    let tmp = tempdir().unwrap();

    let err = TargetProject::try_from_path(tmp.path()).unwrap_err();

    assert_eq!(err, TargetProjectError::NoCargoToml);
}

#[test]
fn target_project_public_api_json_round_trips_absolute_root() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("json_root");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("Cargo.toml"), cargo_manifest("json_root")).unwrap();
    let target = TargetProject::try_from_path(&root).unwrap();

    let json = serde_json::to_string(&target).unwrap();
    let back: TargetProject = serde_json::from_str(&json).unwrap();

    assert_eq!(target, back);
}

#[test]
fn target_project_public_api_json_rejects_invalid_root() {
    let relative = "\"src/foo\"";
    let err: Result<TargetProject, _> = serde_json::from_str(relative);
    assert!(err.is_err());

    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("missing");
    let missing_json = serde_json::to_string(&missing.to_string_lossy()).unwrap();
    let missing_err: Result<TargetProject, _> = serde_json::from_str(&missing_json);
    assert!(missing_err.is_err());
}

#[test]
fn discover_target_public_api_ignores_non_workspace_tables() {
    let tmp = tempdir().unwrap();
    let outer = tmp.path().join("outer");
    let inner = outer.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(outer.join("Cargo.toml"), "[workspace.metadata]\nkind = \"not-root\"\n")
        .unwrap();
    std::fs::write(inner.join("Cargo.toml"), cargo_manifest("inner")).unwrap();

    let target = discover_target(&inner).unwrap();

    assert_eq!(target.as_std_path(), inner.as_path());
}

#[test]
fn discover_target_public_api_ignores_workspace_text_outside_table_header() {
    let tmp = tempdir().unwrap();
    let outer = tmp.path().join("outer");
    let inner = outer.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(
        outer.join("Cargo.toml"),
        "# [workspace]\ndescription = '''\n[workspace]\n'''\n",
    )
    .unwrap();
    std::fs::write(inner.join("Cargo.toml"), cargo_manifest("inner")).unwrap();

    let target = discover_target(&inner).unwrap();

    assert_eq!(target.as_std_path(), inner.as_path());
}

#[test]
fn discover_target_public_api_falls_back_after_invalid_toml() {
    let tmp = tempdir().unwrap();
    let outer = tmp.path().join("outer");
    let inner = outer.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(outer.join("Cargo.toml"), "[workspace\nmembers = [\"inner\"]\n").unwrap();
    std::fs::write(inner.join("Cargo.toml"), cargo_manifest("inner")).unwrap();

    let target = discover_target(&inner).unwrap();

    assert_eq!(target.as_std_path(), inner.as_path());
}

#[test]
fn discover_target_public_api_rejects_malformed_selected_manifest() {
    let tmp = tempdir().unwrap();
    let crate_root = tmp.path().join("malformed");
    let nested = crate_root.join("src");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(crate_root.join("Cargo.toml"), "[package\nname = \"broken\"\n").unwrap();

    let err = discover_target(&nested).unwrap_err();

    assert_eq!(
        err,
        TargetProjectError::MalformedCargoToml {
            path: crate_root.join("Cargo.toml").display().to_string()
        }
    );
}
