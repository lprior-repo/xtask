mod scan;
mod self_test;
mod vcs;

use std::env;

use titania_core::TargetProject;
use titania_lanes::{Finding, LaneExit, LaneReport, current_target_project, exit};

const RULE_TEST_INTEGRITY: &str = "TEST-INTEGRITY-001";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vcs {
    Git,
    Jj,
}

#[derive(Debug, Clone, Copy)]
struct RootInfo {
    vcs: Vcs,
}

pub(crate) fn main_exit() -> std::process::ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        eprintln!(
            "usage: check-test-integrity [--self-test] [--base <rev>]\n\
             Validates that changes since <rev> do not delete tests, weaken\n\
             assertions, or add #[ignore] / compile-only replacements."
        );
        return exit(LaneExit::Usage);
    }
    if args.iter().any(|arg| arg == "--self-test") {
        return exit(self_test::run());
    }
    exit(run_for_args(&args))
}

fn run_for_args(args: &[String]) -> LaneExit {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("test integrity: ERROR cannot resolve target project: {error}");
            return LaneExit::Usage;
        }
    };
    let root = match vcs::root_dir(&target) {
        Ok(info) => info,
        Err(error) => {
            eprintln!("test integrity: ERROR {error}");
            return LaneExit::Failure;
        }
    };
    let base = match argument_value(args, "--base") {
        Some(value) => value,
        None => vcs::default_base(&target, root.vcs),
    };
    match check(&target, &base, root.vcs) {
        Ok(0) => LaneExit::Clean,
        Ok(_) => LaneExit::Violations,
        Err(error) => {
            eprintln!("test integrity: ERROR {error}");
            LaneExit::Usage
        }
    }
}

fn check(target: &TargetProject, base: &str, vcs: Vcs) -> Result<i32, String> {
    vcs::validate_base_revision(target, base, vcs)?;
    let mut findings = deleted_file_findings(&vcs::changed_files(target, base, vcs)?);
    findings.extend(scan::scan_diff(&vcs::diff_text(target, base, vcs)?));
    if findings.is_empty() {
        eprintln!("test integrity: PASS base={base}");
        Ok(0)
    } else {
        render_findings(&findings);
        Ok(1)
    }
}

fn deleted_file_findings(entries: &[(String, String)]) -> Vec<(String, String, String)> {
    entries
        .iter()
        .filter(|(status, path)| status.starts_with('D') && scan::is_test_path(path))
        .map(|(_status, path)| {
            (
                "DeletedTestFile".to_owned(),
                path.clone(),
                "deleted file contained tests or test assertions".to_owned(),
            )
        })
        .collect()
}

fn render_findings(findings: &[(String, String, String)]) {
    eprintln!("test integrity: FAIL");
    let mut report = LaneReport::new();
    findings.iter().for_each(|(kind, path, detail)| {
        push_finding(&mut report, kind, path.clone(), detail.clone());
    });
    eprint!("{}", report.render());
    eprintln!("Add equal-or-stronger replacement coverage or bead-linked justification.");
}

fn push_finding(report: &mut LaneReport, kind: &str, path: String, detail: String) {
    let rule = match kind {
        "DeletedTestFile" => "TEST-INTEGRITY-DEL-001",
        "IgnoredOrSkippedTest" => "TEST-INTEGRITY-IGNORE-001",
        "CompileOnlyReplacement" => "TEST-INTEGRITY-COMPILE-001",
        "DeletedTestDeclaration" => "TEST-INTEGRITY-DECL-001",
        "WeakenedAssertion" => "TEST-INTEGRITY-WEAK-001",
        _ => RULE_TEST_INTEGRITY,
    };
    report.push(Finding::new(rule, path, 0, format!("{kind}: {detail}")));
}

fn argument_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2).find_map(|window| {
        let first = window.first()?;
        let second = window.get(1)?;
        (first == flag).then(|| second.clone())
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use tempfile::TempDir;
    use titania_core::TargetProject;

    use super::{Vcs, check};

    fn run_git(root: &std::path::Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
        let status = Command::new("git").args(args).current_dir(root).status()?;
        assert!(status.success(), "git {args:?} failed with {status}");
        Ok(())
    }

    fn initialized_repo() -> Result<(TempDir, TargetProject), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let root = temp.path();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n")?;
        run_git(root, &["init", "-q"])?;
        run_git(root, &["config", "user.email", "lane@example.invalid"])?;
        run_git(root, &["config", "user.name", "Lane Test"])?;
        run_git(root, &["add", "Cargo.toml"])?;
        run_git(root, &["commit", "-q", "-m", "base"])?;
        let target = TargetProject::try_from_path(root)?;
        Ok((temp, target))
    }

    #[test]
    fn check_reports_untracked_new_behavior_tests() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, target) = initialized_repo()?;
        let tests_dir = target.as_std_path().join("tests");
        fs::create_dir_all(&tests_dir)?;
        fs::write(
            tests_dir.join("new_behavior.rs"),
            "#[test]\n#[ignore]\nfn tracks_behavior() {\n    assert_eq!(2 + 2, 4);\n}\n",
        )?;

        assert_eq!(check(&target, "HEAD", Vcs::Git)?, 1);
        Ok(())
    }
}
