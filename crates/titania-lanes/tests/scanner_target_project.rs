use std::{
    error::Error,
    fs,
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn hotpath_scan_from_member_scans_workspace_root() -> TestResult {
    let fixture = workspace_fixture()?;
    write_file(
        fixture.path().join("crates/vb_core/src/lib.rs"),
        "use std::collections::HashMap;\n",
    )?;

    let output = run_from_member(env!("CARGO_BIN_EXE_hotpath-scan"), &fixture)?;

    assert!(!output.status.success(), "scanner missed root hotpath token");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("crates/vb_core/src/lib.rs"), "stderr was: {stderr}");
    assert!(stderr.contains("token HashMap on hot path"), "stderr was: {stderr}");
    Ok(())
}

#[test]
fn panic_surface_from_member_scans_workspace_root() -> TestResult {
    let fixture = workspace_fixture()?;
    write_file(
        fixture.path().join("crates/example/src/lib.rs"),
        "pub fn guard(value: bool) {\n    assert!(value);\n}\n",
    )?;

    let output = run_from_member(env!("CARGO_BIN_EXE_check-panic-surface"), &fixture)?;

    assert!(!output.status.success(), "scanner missed root panic macro");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("crates/example/src/lib.rs"), "stderr was: {stderr}");
    assert!(stderr.contains("PANIC-SURFACE-001"), "stderr was: {stderr}");
    Ok(())
}

#[test]
fn production_inner_drift_from_member_scans_workspace_root() -> TestResult {
    let fixture = workspace_fixture()?;
    write_file(
        fixture.path().join("crates/example/src/lib.rs"),
        "pub struct MissingIdentifier;\n",
    )?;
    write_file(
        fixture.path().join("verification/verus/production_inner/example.rs"),
        concat!(
            "// DRIFT POLICY: `crates/example/src/lib.rs`\n",
            "// Production source: `crates/example/src/lib.rs:1-1`\n",
            "pub struct PresentIdentifier;\n",
        ),
    )?;

    let output = run_from_member(env!("CARGO_BIN_EXE_check-production-inner-drift"), &fixture)?;

    assert!(!output.status.success(), "scanner missed root production-inner drift");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("verification/verus/production_inner/example.rs"),
        "stderr was: {stderr}"
    );
    assert!(stderr.contains("missing identifiers"), "stderr was: {stderr}");
    assert!(stderr.contains("MissingIdentifier"), "stderr was: {stderr}");
    Ok(())
}

fn workspace_fixture() -> TestResult<TempDir> {
    let fixture = TempDir::new()?;
    write_file(
        fixture.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\nresolver = \"2\"\n",
    )?;
    write_file(
        fixture.path().join("member/Cargo.toml"),
        "[package]\nname = \"member\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    write_file(fixture.path().join("member/src/lib.rs"), "pub fn member() {}\n")?;
    Ok(fixture)
}

fn run_from_member(binary: &str, fixture: &TempDir) -> TestResult<Output> {
    Ok(Command::new(binary).current_dir(fixture.path().join("member")).output()?)
}

fn write_file(path: impl AsRef<Path>, text: &str) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}
