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
#[test]
fn panic_surface_flags_assert_after_cfg_test_mod_inside_fn() -> TestResult {
    // Regression: the cfg(test) scope tracker used to leave the cfg
    // scope open past its closing `}` when the `#[cfg(test)] mod` was
    // nested inside a function body, so an `assert!(false)` placed
    // after the block slipped through silently.
    let fixture = workspace_fixture()?;
    let lib = "pub fn a() {\n\
               \x20\x20\x20\x20#[cfg(test)]\n\
               \x20\x20\x20\x20mod tests {\n\
               \x20\x20\x20\x20\x20\x20\x20\x20#[test]\n\
               \x20\x20\x20\x20\x20\x20\x20\x20fn x() {\n\
               \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20assert!(true);\n\
               \x20\x20\x20\x20\x20\x20\x20\x20}\n\
               \x20\x20\x20\x20}\n\
               \x20\x20\x20\x20assert!(false);\n\
               }\n";
    write_file(fixture.path().join("crates/example/src/lib.rs"), lib)?;

    let output = run_from_member(env!("CARGO_BIN_EXE_check-panic-surface"), &fixture)?;

    assert!(!output.status.success(), "scanner missed assert after cfg(test) mod");
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The assert OUTSIDE the cfg(test) block (line 9) must be reported
    // as a violation, with the macro name `assert!` in the message.
    assert!(stderr.contains("lib.rs:9"), "expected finding at lib.rs:9; stderr was: {stderr}");
    assert!(stderr.contains("PANIC-SURFACE-001"), "stderr was: {stderr}");
    // The assert INSIDE the cfg(test) block (line 5) must NOT appear.
    assert!(!stderr.contains("lib.rs:5"), "cfg(test) internals leaked: {stderr}");
    Ok(())
}

#[test]
fn panic_surface_does_not_flag_assert_inside_top_level_cfg_test_mod() -> TestResult {
    // Counterpart: a top-level (not nested) `#[cfg(test)] mod` must
    // suppress the inner `assert!` and still flag the outer one.
    let fixture = workspace_fixture()?;
    let lib = "#[cfg(test)]\n\
               mod tests {\n\
               \x20\x20\x20\x20#[test]\n\
               \x20\x20\x20\x20fn x() {\n\
               \x20\x20\x20\x20\x20\x20\x20\x20assert!(true);\n\
               \x20\x20\x20\x20}\n\
               }\n\
               assert!(false);\n";
    write_file(fixture.path().join("crates/example/src/lib.rs"), lib)?;

    let output = run_from_member(env!("CARGO_BIN_EXE_check-panic-surface"), &fixture)?;

    assert!(!output.status.success(), "scanner missed outer assert");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lib.rs:8"), "expected finding at lib.rs:8; stderr was: {stderr}");
    // Inner assert at line 5 must NOT be flagged.
    assert!(!stderr.contains("lib.rs:5"), "cfg(test) internals leaked: {stderr}");
    Ok(())
}

#[test]
fn nightly_features_single_line_allowed_feature_is_not_flagged() -> TestResult {
    // Regression: `push_closed_feature` used to slice up to the `)` of
    // the `)]` close, leaving the `]` out of the slice. The downstream
    // `trim_end_matches(")]")` then failed to strip the suffix, so a
    // single-line `#![feature(try_blocks)]` was reported as the
    // literal feature name `try_blocks)` and treated as disallowed.
    let fixture = workspace_fixture()?;
    write_file(
        fixture.path().join("crates/example/src/lib.rs"),
        "#![feature(try_blocks)]\npub fn a() {}\n",
    )?;

    // The nightly lane walks the current directory; the existing
    // helper `run_from_member` cd's into `member/`, so for this test
    // we run directly from the workspace root.
    let output = Command::new(env!("CARGO_BIN_EXE_check-nightly-features"))
        .current_dir(fixture.path())
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("try_blocks)"),
        "feature name carries stray ')'; stderr was: {stderr}"
    );
    assert!(stderr.contains("no disallowed feature attributes"), "stderr was: {stderr}");
    Ok(())
}

#[test]
fn nightly_features_multi_line_attribute_extracts_every_name() -> TestResult {
    // The two perf-only features span multiple lines. Each must be
    // extracted as its own feature (no leading whitespace, no `,`).
    let fixture = workspace_fixture()?;
    write_file(
        fixture.path().join("crates/example/src/lib.rs"),
        "#![feature(\n    allocator_api,\n    generic_const_exprs\n)]\npub fn a() {}\n",
    )?;
    let output = Command::new(env!("CARGO_BIN_EXE_check-nightly-features"))
        .current_dir(fixture.path())
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Both names must appear in their clean form.
    assert!(stderr.contains("`allocator_api`"), "stderr was: {stderr}");
    assert!(stderr.contains("`generic_const_exprs`"), "stderr was: {stderr}");
    Ok(())
}

#[test]
fn forbidden_scan_flags_expect_with_message() -> TestResult {
    // Regression: `expect()` (with both parens) never appears as a
    // literal substring in real Rust code (`.expect("msg")` only has
    // `expect(`). The token set now stores `expect` as a `Method` and
    // matches when followed by `(`.
    let fixture = workspace_fixture()?;
    write_file(
        fixture.path().join("crates/example/src/lib.rs"),
        "use std::fs;\npub fn a() -> String {\n    fs::read_to_string(\"/tmp/x\").expect(\"boom\")\n}\n",
    )?;
    let output =
        Command::new(env!("CARGO_BIN_EXE_forbidden-scan")).current_dir(fixture.path()).output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "scanner missed .expect()");
    assert!(stderr.contains("`expect`"), "stderr was: {stderr}");
    Ok(())
}

#[test]
fn forbidden_scan_does_not_flag_user_identifiers_named_like_tokens() -> TestResult {
    // The Method matcher requires `.` or `::` immediately before the
    // name and `(` immediately after, so user identifiers like
    // `myexpect()` are not flagged.
    let fixture = workspace_fixture()?;
    write_file(
        fixture.path().join("crates/example/src/lib.rs"),
        "fn myexpect() -> i32 { 1 }\nfn myunwrap() -> i32 { 2 }\npub fn a() -> i32 { myexpect() + myunwrap() }\n",
    )?;
    let output =
        Command::new(env!("CARGO_BIN_EXE_forbidden-scan")).current_dir(fixture.path()).output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "false positive: {stderr}");
    Ok(())
}

#[test]
fn forbidden_scan_flags_qualified_path_unwrap() -> TestResult {
    // `Result::unwrap(...)` is a forbidden method call. The Method
    // matcher accepts `::` as a receiver.
    let fixture = workspace_fixture()?;
    write_file(
        fixture.path().join("crates/example/src/lib.rs"),
        "use std::fs;\npub fn a() -> String {\n    let r: Result<String, _> = fs::read_to_string(\"/tmp/x\");\n    Result::unwrap(r)\n}\n",
    )?;
    let output =
        Command::new(env!("CARGO_BIN_EXE_forbidden-scan")).current_dir(fixture.path()).output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "scanner missed Result::unwrap");
    assert!(stderr.contains("`unwrap`"), "stderr was: {stderr}");
    Ok(())
}

#[test]
fn panic_surface_skips_block_comments() -> TestResult {
    // Regression: the old `is_comment` only checked `//`, so an
    // `assert!` inside a `/* ... */` block comment was flagged. The
    // panic-surface lane now uses the shared `SourceLine` parser,
    // which strips line/block comments and blanks string contents.
    let fixture = workspace_fixture()?;
    let lib = "pub fn a() {\n    /* assert!(true); */\n    let _x = 1;\n}\n";
    write_file(fixture.path().join("crates/example/src/lib.rs"), lib)?;
    let output = run_from_member(env!("CARGO_BIN_EXE_check-panic-surface"), &fixture)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "block comment leaked: {stderr}");
    Ok(())
}

#[test]
fn panic_surface_skips_assert_inside_string_literal() -> TestResult {
    // An `assert!` that is purely a string literal is not real code
    // and must not be flagged. The `SourceLine` parser blanks string
    // contents so the panic-macro check never sees them.
    let fixture = workspace_fixture()?;
    let lib = "pub const DOC: &str = \"do not call assert!(true) here\";\npub fn a() {}\n";
    write_file(fixture.path().join("crates/example/src/lib.rs"), lib)?;
    let output = run_from_member(env!("CARGO_BIN_EXE_check-panic-surface"), &fixture)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "string literal leaked: {stderr}");
    Ok(())
}
