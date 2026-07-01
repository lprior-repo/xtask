use std::{
    error::Error,
    fs,
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn run_cargo(cwd: &Path, args: &[&str]) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_run-cargo")).args(args).current_dir(cwd).output()
}

fn package(name: &str, lib_rs: &str, main_rs: &str) -> Result<TempDir, std::io::Error> {
    let temp = tempfile::tempdir()?;
    fs::write(
        temp.path().join("Cargo.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    )?;
    fs::create_dir_all(temp.path().join("src"))?;
    fs::write(temp.path().join("src/lib.rs"), lib_rs)?;
    fs::write(temp.path().join("src/main.rs"), main_rs)?;
    Ok(temp)
}

fn package_with_lock(name: &str, lib_rs: &str, main_rs: &str) -> Result<TempDir, Box<dyn Error>> {
    let temp = package(name, lib_rs, main_rs)?;
    let status = Command::new("cargo")
        .arg("generate-lockfile")
        .arg("--manifest-path")
        .arg(temp.path().join("Cargo.toml"))
        .status()?;
    if status.success() {
        Ok(temp)
    } else {
        Err(std::io::Error::other("cargo generate-lockfile failed").into())
    }
}

fn append_manifest(root: &Path, text: &str) -> Result<(), std::io::Error> {
    let manifest = root.join("Cargo.toml");
    let mut content = fs::read_to_string(&manifest)?;
    content.push_str(text);
    fs::write(manifest, content)
}

fn write_project_file(root: &Path, relative: &str, text: &str) -> Result<(), std::io::Error> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn stderr_text(output: &Output) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(output.stderr.clone())
}

#[test]
fn run_cargo_without_subcommand_returns_usage() -> TestResult {
    let output = run_cargo(Path::new("."), &[])?;
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_text(&output)?.contains("usage: run-cargo"));
    Ok(())
}

#[test]
fn run_cargo_unknown_subcommand_returns_usage() -> TestResult {
    let output = run_cargo(Path::new("."), &["frobnicate"])?;
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_text(&output)?.contains("fmt|compile|clippy|test|build"));
    Ok(())
}

#[test]
fn run_cargo_fmt_clean_project_returns_clean() -> TestResult {
    let temp =
        package("fmt_clean_project", "pub fn value() -> u8 {\n    1\n}\n", "fn main() {}\n")?;

    let output = run_cargo(temp.path(), &["fmt"])?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output)?, "");
    Ok(())
}

#[test]
fn run_cargo_fmt_bad_project_reports_diff_hunk() -> TestResult {
    let temp = package("fmt_bad_project", "pub fn value()->u8{1}\n", "fn main(){}\n")?;

    let output = run_cargo(temp.path(), &["fmt"])?;
    let stderr = stderr_text(&output)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-FMT-001"));
    assert!(stderr.contains("rustfmt diff hunk"));
    Ok(())
}

#[test]
fn run_cargo_clippy_clean_project_returns_clean() -> TestResult {
    let temp = package_with_lock(
        "clippy_clean_project",
        "pub fn value() -> u8 {\n    1\n}\n",
        "fn main() {}\n",
    )?;

    let output = run_cargo(temp.path(), &["clippy"])?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output)?, "");
    Ok(())
}

#[test]
fn run_cargo_clippy_bad_project_reports_diagnostic() -> TestResult {
    let temp = package_with_lock(
        "clippy_bad_project",
        "#![warn(clippy::needless_bool)]\npub fn flag() -> bool {\n    if true { true } else { false }\n}\n",
        "fn main() {}\n",
    )?;

    let output = run_cargo(temp.path(), &["clippy"])?;
    let stderr = stderr_text(&output)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-CLIPPY-001"));
    assert!(stderr.contains("needless_bool") || stderr.contains("bool"));
    Ok(())
}

#[test]
fn run_cargo_clippy_checks_examples() -> TestResult {
    let temp = package_with_lock(
        "clippy_example_bad_project",
        "pub fn value() -> u8 {\n    1\n}\n",
        "fn main() {}\n",
    )?;
    write_project_file(
        temp.path(),
        "examples/bad.rs",
        "fn main() {\n    let _value: u8 = \"bad\";\n}\n",
    )?;

    let output = run_cargo(temp.path(), &["clippy"])?;
    let stderr = stderr_text(&output)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-CLIPPY-001"));
    assert!(stderr.contains("mismatched types"));
    Ok(())
}

#[test]
fn run_cargo_compile_clean_project_returns_clean() -> TestResult {
    let temp = package_with_lock(
        "compile_clean_project",
        "pub fn value() -> u8 {\n    1\n}\n",
        "fn main() {}\n",
    )?;

    let output = run_cargo(temp.path(), &["compile"])?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output)?, "");
    Ok(())
}

#[test]
fn run_cargo_compile_bad_project_reports_diagnostic() -> TestResult {
    let temp = package_with_lock(
        "compile_bad_project",
        "pub fn value() -> String {\n    1\n}\n",
        "fn main() {}\n",
    )?;

    let output = run_cargo(temp.path(), &["compile"])?;
    let stderr = stderr_text(&output)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-COMPILE-001"));
    assert!(stderr.contains("mismatched types"));
    Ok(())
}

#[test]
fn run_cargo_compile_checks_integration_tests() -> TestResult {
    let temp = package_with_lock(
        "compile_integration_bad_project",
        "pub fn value() -> u8 {\n    1\n}\n",
        "fn main() {}\n",
    )?;
    write_project_file(
        temp.path(),
        "tests/compile_only.rs",
        "#[test]\nfn compile_breaks() {\n    let _value: u8 = \"bad\";\n}\n",
    )?;

    let output = run_cargo(temp.path(), &["compile"])?;
    let stderr = stderr_text(&output)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-COMPILE-001"));
    assert!(stderr.contains("mismatched types"));
    Ok(())
}

#[test]
fn run_cargo_test_clean_project_returns_clean() -> TestResult {
    let temp = package_with_lock(
        "test_clean_project",
        "pub fn value() -> u8 {\n    1\n}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn passes() {\n        assert_eq!(super::value(), 1);\n    }\n}\n",
        "fn main() {}\n",
    )?;

    let output = run_cargo(temp.path(), &["test"])?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output)?, "");
    Ok(())
}

#[test]
fn run_cargo_test_bad_project_reports_failed_test() -> TestResult {
    let temp = package_with_lock(
        "test_bad_project",
        "pub fn value() -> u8 {\n    1\n}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn fails() {\n        assert_eq!(super::value(), 2);\n    }\n}\n",
        "fn main() {}\n",
    )?;

    let output = run_cargo(temp.path(), &["test"])?;
    let stderr = stderr_text(&output)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-TEST-001"));
    assert!(stderr.contains("test failed: tests::fails"));
    Ok(())
}

#[test]
fn run_cargo_test_checks_all_features() -> TestResult {
    let temp = package_with_lock(
        "test_feature_bad_project",
        "pub fn value() -> u8 {\n    1\n}\n#[cfg(all(test, feature = \"failure\"))]\nmod tests {\n    #[test]\n    fn fails_when_feature_enabled() {\n        assert_eq!(super::value(), 2);\n    }\n}\n",
        "fn main() {}\n",
    )?;
    append_manifest(temp.path(), "\n[features]\nfailure = []\n")?;

    let output = run_cargo(temp.path(), &["test"])?;
    let stderr = stderr_text(&output)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-TEST-001"));
    assert!(stderr.contains("test failed: tests::fails_when_feature_enabled"));
    Ok(())
}

#[test]
fn run_cargo_build_clean_project_returns_clean() -> TestResult {
    let temp = package_with_lock(
        "build_clean_project",
        "pub fn value() -> u8 {\n    1\n}\n",
        "fn main() {}\n",
    )?;

    let output = run_cargo(temp.path(), &["build"])?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output)?, "");
    Ok(())
}

#[test]
fn run_cargo_build_bad_project_reports_diagnostic() -> TestResult {
    let temp = package_with_lock(
        "build_bad_project",
        "pub fn value() -> String {\n    1\n}\n",
        "fn main() {}\n",
    )?;

    let output = run_cargo(temp.path(), &["build"])?;
    let stderr = stderr_text(&output)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("CARGO-BUILD-001"));
    assert!(stderr.contains("mismatched types"));
    Ok(())
}
