use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use titania_core::TargetProject;
use titania_lanes::LaneExit;

use super::{Vcs, check};

pub(super) fn run() -> LaneExit {
    match run_fixtures() {
        Ok(()) => {
            eprintln!("SelfTest:check-test-integrity:PASS");
            LaneExit::Clean
        }
        Err(error) => {
            eprintln!("SelfTest:check-test-integrity:FAIL {error}");
            LaneExit::Violations
        }
    }
}

fn run_fixtures() -> Result<(), String> {
    let scratch = scratch_dir()?;
    let result = with_initialized_repo(&scratch, |target| {
        assert_clean_fixture(target)?;
        assert_untracked_ignored_fixture(target)
    });
    let cleanup = fs::remove_dir_all(&scratch).map_err(|error| format!("cleanup failed: {error}"));
    result.and(cleanup)
}

fn with_initialized_repo<F>(root: &Path, f: F) -> Result<(), String>
where
    F: FnOnce(&TargetProject) -> Result<(), String>,
{
    fs::create_dir_all(root).map_err(|error| format!("create scratch repo failed: {error}"))?;
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n")
        .map_err(|error| format!("write Cargo.toml failed: {error}"))?;
    run_git(root, &["init", "-q"])?;
    run_git(root, &["config", "user.email", "lane@example.invalid"])?;
    run_git(root, &["config", "user.name", "Lane Test"])?;
    run_git(root, &["add", "Cargo.toml"])?;
    run_git(root, &["commit", "-q", "-m", "base"])?;
    let target = TargetProject::try_from_path(root)
        .map_err(|error| format!("target project construction failed: {error}"))?;
    f(&target)
}

fn assert_clean_fixture(target: &TargetProject) -> Result<(), String> {
    match check(target, "HEAD", Vcs::Git)? {
        0 => Ok(()),
        code => Err(format!("clean fixture returned {code}")),
    }
}

fn assert_untracked_ignored_fixture(target: &TargetProject) -> Result<(), String> {
    let tests_dir = target.as_std_path().join("tests");
    fs::create_dir_all(&tests_dir).map_err(|error| format!("create tests dir failed: {error}"))?;
    fs::write(
        tests_dir.join("untracked_ignored.rs"),
        "#[test]\n#[ignore]\nfn covered_behavior() {\n    assert_eq!(2 + 2, 4);\n}\n",
    )
    .map_err(|error| format!("write untracked test failed: {error}"))?;
    match check(target, "HEAD", Vcs::Git)? {
        1 => Ok(()),
        code => Err(format!("untracked ignored fixture returned {code}")),
    }
}

fn run_git(root: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| format!("git {args:?} failed to start: {error}"))?;
    if status.success() { Ok(()) } else { Err(format!("git {args:?} exited with {status}")) }
}

fn scratch_dir() -> Result<PathBuf, String> {
    let root = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system time before epoch: {error}"))?
        .as_nanos();
    Ok(root.join(format!("titania-check-test-integrity-{stamp}")))
}
