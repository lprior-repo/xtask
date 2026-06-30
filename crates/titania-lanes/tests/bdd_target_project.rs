use std::{
    error::Error,
    fs,
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;
use titania_core::{
    Digest, LaneDigest, LaneName, QualityReceipt, ReceiptDigests, ReceiptLaneExit, ReceiptPeriod,
    TargetProject, TargetProjectError, discover_target,
};

type TestResult = Result<(), Box<dyn Error>>;

fn run_cargo(cwd: &Path, args: &[&str]) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_run-cargo")).args(args).current_dir(cwd).output()
}

fn single_crate(name: &str, lib_rs: &str) -> Result<TempDir, std::io::Error> {
    let temp = tempfile::tempdir()?;
    write_package(temp.path(), name, lib_rs)?;
    Ok(temp)
}

fn workspace_with_member(
    member_lib_rs: &str,
) -> Result<(TempDir, std::path::PathBuf), std::io::Error> {
    let temp = tempfile::tempdir()?;
    fs::write(
        temp.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/foo\"]\nresolver = \"3\"\n",
    )?;
    let member = temp.path().join("crates/foo");
    write_package(&member, "foo", member_lib_rs)?;
    Ok((temp, member))
}

fn write_package(root: &Path, name: &str, lib_rs: &str) -> Result<(), std::io::Error> {
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    )?;
    fs::write(root.join("src/lib.rs"), lib_rs)?;
    Ok(())
}

fn stderr_text(output: &Output) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(output.stderr.clone())
}

fn stable_digest(label: &'static [u8]) -> Digest {
    Digest::from_bytes(label)
}

#[test]
fn scenario_workspace_discovery_from_subcrate_reports_member_diff() -> TestResult {
    // Given: cwd is a sub-crate of a workspace with badly formatted Rust.
    let (_workspace, member) = workspace_with_member("pub fn value()->u8{1}\n")?;

    // When: run-cargo fmt is invoked from the member directory.
    let output = run_cargo(&member, &["fmt"])?;
    let stderr = stderr_text(&output)?;

    // Then: the lane discovers the workspace target and reports the member diff.
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-FMT-001"));
    assert!(stderr.contains("crates/foo/src/lib.rs"));
    Ok(())
}

#[test]
fn scenario_single_crate_root_uses_cwd_as_target() -> TestResult {
    // Given: cwd is a standalone Cargo package root.
    let target = single_crate("single_crate_target", "pub fn value()->u8{1}\n")?;

    // When: run-cargo fmt is invoked from that root.
    let output = run_cargo(target.path(), &["fmt"])?;
    let stderr = stderr_text(&output)?;

    // Then: the lane reports the file inside that single-crate target.
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-FMT-001"));
    assert!(stderr.contains("src/lib.rs"));
    Ok(())
}

#[test]
fn scenario_missing_cargo_toml_returns_usage_with_typed_error() -> TestResult {
    // Given: cwd has no Cargo.toml in itself or its temporary ancestors.
    let target = tempfile::tempdir()?;

    // When: run-cargo fmt is invoked there.
    let output = run_cargo(target.path(), &["fmt"])?;
    let stderr = stderr_text(&output)?;

    // Then: discovery fails closed as a usage/config error with the typed message.
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("target discovery failed"));
    assert!(stderr.contains("target project directory does not contain a Cargo.toml file"));
    Ok(())
}

#[test]
fn scenario_completed_lane_receipt_records_resolved_target_root() -> TestResult {
    // Given: a clean standalone Cargo package and a successful lane run.
    let target_dir = single_crate("receipt_target", "pub fn value() -> u8 {\n    1\n}\n")?;
    let output = run_cargo(target_dir.path(), &["fmt"])?;
    assert_eq!(output.status.code(), Some(0));

    // When: a receipt is built for the completed lane.
    let target = discover_target(target_dir.path())?;
    let receipt = QualityReceipt::new(
        target,
        ReceiptPeriod::new(1, 2)?,
        vec![LaneDigest::new(LaneName::new("fmt")?, ReceiptLaneExit::Clean, 1, 1, 0)?],
        ReceiptDigests::new(
            stable_digest(b"source"),
            stable_digest(b"lock"),
            stable_digest(b"policy"),
            stable_digest(b"toolchain"),
        ),
    )?;
    let json = serde_json::to_string(&receipt)?;

    // Then: the serialized receipt includes the resolved target_root.
    assert!(json.contains("\"target_root\""));
    assert!(json.contains(&target_dir.path().display().to_string()));
    Ok(())
}

#[test]
fn scenario_empty_target_input_returns_typed_error_without_panic() -> TestResult {
    // Given: an empty target path.
    let empty = Path::new("");

    // When: the TargetProject constructor validates it.
    let err = TargetProject::try_from_path(empty)
        .err()
        .ok_or_else(|| std::io::Error::other("empty target path was accepted"))?;

    // Then: construction returns the exact typed error.
    assert_eq!(err, TargetProjectError::Empty);
    Ok(())
}
