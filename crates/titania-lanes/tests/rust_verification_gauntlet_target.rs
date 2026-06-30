use std::{
    error::Error,
    fs,
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn plain_target() -> Result<TempDir, std::io::Error> {
    let temp = tempfile::tempdir()?;
    write_package(temp.path(), "plain_target")?;
    Ok(temp)
}

fn write_package(root: &Path, name: &str) -> Result<(), std::io::Error> {
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    )?;
    fs::write(root.join("src/lib.rs"), "pub fn value() -> u8 {\n    1\n}\n")?;
    Ok(())
}

fn run_gauntlet(cwd: &Path, mode: &str) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_rust-verification-gauntlet"))
        .arg(mode)
        .current_dir(cwd)
        .output()
}

fn stderr_text(output: &Output) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(output.stderr.clone())
}

#[test]
fn plain_target_without_vb_compile_is_cleanly_not_applicable() -> TestResult {
    let target = plain_target()?;

    let output = run_gauntlet(target.path(), "fast")?;
    let stderr = stderr_text(&output)?;

    assert!(!stderr.contains("titania-lanes` not found"), "{stderr}");
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(stderr.contains("NotApplicable: package vb_compile absent"), "{stderr}");
    Ok(())
}
