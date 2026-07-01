use std::{
    env,
    error::Error,
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn write_project_file(root: &Path, relative: &str, text: &str) -> Result<(), std::io::Error> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn target_project_with_verus_target() -> Result<TempDir, std::io::Error> {
    let temp = tempfile::tempdir()?;
    write_project_file(
        temp.path(),
        "Cargo.toml",
        "[package]\nname = \"verus_target_project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write_project_file(
        temp.path(),
        "contracts/proof_obligations.yaml",
        "version: 1\ntargets:\n  - path: verification/verus/failing.rs\n",
    )?;
    write_project_file(temp.path(), "verification/verus/failing.rs", "fn main() {}\n")?;
    Ok(temp)
}
fn target_project_with_registry(registry: &str) -> Result<TempDir, std::io::Error> {
    let temp = tempfile::tempdir()?;
    write_project_file(
        temp.path(),
        "Cargo.toml",
        "[package]\nname = \"verus_target_project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write_project_file(temp.path(), "contracts/proof_obligations.yaml", registry)?;
    Ok(temp)
}

fn target_project_without_registry() -> Result<TempDir, std::io::Error> {
    let temp = tempfile::tempdir()?;
    write_project_file(
        temp.path(),
        "Cargo.toml",
        "[package]\nname = \"verus_target_project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    Ok(temp)
}

fn fake_verus_bin() -> Result<TempDir, std::io::Error> {
    let temp = tempfile::tempdir()?;
    let bin = temp.path().join("verus");
    fs::write(
        &bin,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'verus fake'\n  exit 0\nfi\necho 'fake verifier failure' >&2\nexit 42\n",
    )?;
    let mut permissions = fs::metadata(&bin)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(bin, permissions)?;
    Ok(temp)
}

fn fake_successful_verus_bin() -> Result<TempDir, std::io::Error> {
    let temp = tempfile::tempdir()?;
    let bin = temp.path().join("verus");
    fs::write(
        &bin,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'verus fake'\n  exit 0\nfi\necho 'verified fake target'\nexit 0\n",
    )?;
    let mut permissions = fs::metadata(&bin)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(bin, permissions)?;
    Ok(temp)
}

fn run_verify_verus_with_path(
    cwd: &Path,
    path: &std::ffi::OsStr,
) -> Result<Output, Box<dyn Error>> {
    Ok(Command::new(env!("CARGO_BIN_EXE_verify-verus"))
        .current_dir(cwd)
        .env("PATH", path)
        .output()?)
}

fn run_verify_verus(cwd: &Path, fake_bin: &Path) -> Result<Output, Box<dyn Error>> {
    let old_path = env::var_os("PATH").ok_or_else(|| std::io::Error::other("PATH missing"))?;
    let mut path = fake_bin.as_os_str().to_os_string();
    path.push(":");
    path.push(old_path);
    run_verify_verus_with_path(cwd, &path)
}

fn stderr_text(output: &Output) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(output.stderr.clone())
}

#[test]
fn verify_verus_failed_target_returns_violation_and_failure_summary() -> TestResult {
    let target = target_project_with_verus_target()?;
    let fake_bin = fake_verus_bin()?;

    let output = run_verify_verus(target.path(), fake_bin.path())?;
    let stderr = stderr_text(&output)?;
    let summary = fs::read_to_string(target.path().join(".evidence/verus/summary.txt"))?;

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("VERUS-TARGET-001"));
    assert!(
        stderr.contains("fake verifier failure")
            || stderr.contains("verification/verus/failing.rs")
    );
    assert!(summary.contains("VERUS_TARGET_FAILED verification/verus/failing.rs"));
    assert!(summary.contains("VERUS_REGISTRY_FAILED"));
    assert!(!summary.contains("VERUS_REGISTRY_OK"));
    Ok(())
}

#[test]
fn verify_verus_missing_binary_returns_failure() -> TestResult {
    let target = target_project_with_verus_target()?;
    let empty_path = tempfile::tempdir()?;

    let output = run_verify_verus_with_path(target.path(), empty_path.path().as_os_str())?;
    let stderr = stderr_text(&output)?;

    assert_eq!(output.status.code(), Some(3));
    assert!(stderr.contains("verus not on PATH"));
    assert!(stderr.contains("formal verification cannot run"));
    Ok(())
}

#[test]
fn verify_verus_missing_registry_returns_usage_error() -> TestResult {
    let target = target_project_without_registry()?;
    let fake_bin = fake_verus_bin()?;

    let output = run_verify_verus(target.path(), fake_bin.path())?;
    let stderr = stderr_text(&output)?;

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("registry missing or empty"));
    assert!(stderr.contains("formal obligations are required"));
    Ok(())
}

#[test]
fn verify_verus_empty_registry_returns_usage_error() -> TestResult {
    let target = target_project_with_registry("version: 1\ntargets: []\n")?;
    let fake_bin = fake_verus_bin()?;

    let output = run_verify_verus(target.path(), fake_bin.path())?;
    let stderr = stderr_text(&output)?;

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("no targets discovered"));
    assert!(stderr.contains("formal obligations are required"));
    Ok(())
}

#[test]
fn verify_verus_external_marker_without_waiver_returns_violation() -> TestResult {
    let target = target_project_with_registry(
        "version: 1\ntargets:\n  - path: verification/verus/external_marker.rs\n",
    )?;
    write_project_file(
        target.path(),
        "verification/verus/external_marker.rs",
        "#[verifier::external_body]\nfn trusted_shell() {}\nfn main() {}\n",
    )?;
    let fake_bin = fake_successful_verus_bin()?;

    let output = run_verify_verus(target.path(), fake_bin.path())?;
    let stderr = stderr_text(&output)?;
    let trust_scan = fs::read_to_string(target.path().join(".evidence/verus/trust-scan.txt"))?;
    let summary = fs::read_to_string(target.path().join(".evidence/verus/summary.txt"))?;

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("VERUS-EXTERNAL-001"));
    assert!(trust_scan.contains("verification/verus/external_marker.rs:1"));
    assert!(summary.contains("VERUS_EXTERNAL_MARKER_FAILURE_COUNT 1"));
    assert!(summary.contains("VERUS_REGISTRY_FAILED"));
    assert!(!summary.contains("VERUS_REGISTRY_OK"));
    Ok(())
}

#[test]
fn verify_verus_smoke_only_fixture_returns_usage_not_registry_ok() -> TestResult {
    let target = target_project_with_registry(
        "version: 1\ntargets:\n  - path: verification/verus/formal_setup_smoke.rs\n",
    )?;
    write_project_file(
        target.path(),
        "verification/verus/formal_setup_smoke.rs",
        "// titania-verus-binding: fixture-smoke\nfn main() {}\n",
    )?;
    let fake_bin = fake_successful_verus_bin()?;

    let output = run_verify_verus(target.path(), fake_bin.path())?;
    let stderr = stderr_text(&output)?;
    let summary = fs::read_to_string(target.path().join(".evidence/verus/summary.txt"))?;

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("only fixture smoke obligations"));
    assert!(summary.contains("VERUS_REGISTRY_NOT_APPLICABLE fixture-smoke-only"));
    assert!(!summary.contains("VERUS_REGISTRY_OK"));
    Ok(())
}
