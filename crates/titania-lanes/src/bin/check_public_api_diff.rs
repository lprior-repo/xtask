//! Diffs `cargo public-api` for every `vb_*` package against `origin/main`.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-public-api-diff.sh`. Run via
//! `cargo run --bin check-public-api-diff --` from the repository root or via
//! the matching Moon task in `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "check_public_api_diff/package_json.rs"]
mod package_json;

use titania_core::TargetProject;
use titania_lanes::{CommandIn, Finding, LaneExit, LaneReport, current_target_project, exit};

use package_json::extract_package_names;

const RULE_CARGO_MISSING: &str = "PUBAPI-CARGO-MISSING-001";
const RULE_METADATA: &str = "PUBAPI-METADATA-001";
const RULE_PUBLIC_API_DIFF: &str = "PUBAPI-DIFF-001";
const RULE_PUBLIC_API_TOOL: &str = "PUBAPI-TOOL-001";
const RULE_TARGET: &str = "PUBAPI-TARGET-001";
const TOOLCHAIN: &str = "nightly-2026-04-28";

fn filter_packages(discovered: Vec<String>) -> Vec<String> {
    let mut selected: Vec<String> = discovered
        .into_iter()
        .filter(|name| {
            name.starts_with("vb_") || name == "velvet-ballistics" || name == "velvet_ballistics"
        })
        .collect();
    selected.sort();
    selected.dedup();
    selected
}

fn discover_packages(target: &TargetProject) -> Result<Vec<String>, String> {
    let manifest = target.manifest_path();
    let mut command = CommandIn::new(target, "cargo")
        .map_err(|error| format!("cargo metadata failed to start: {error}"))?;
    command.inherit_env();
    command
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(manifest.as_str());
    let output = command
        .run_capture_raw()
        .map_err(|error| format!("cargo metadata failed to start: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo metadata failed: {stderr}"));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| format!("cargo metadata returned non-UTF8 JSON: {error}"))?;
    Ok(filter_packages(extract_package_names(&text)))
}

enum PublicApiDiff {
    Clean,
    Violation(String),
    Failure(String),
}

fn run_public_api_diff<'a>(
    target: &'a TargetProject,
    package: &'a str,
    manifest: &'a str,
) -> PublicApiDiff {
    let mut command = match CommandIn::new(target, "rustup") {
        Ok(command) => command,
        Err(error) => return PublicApiDiff::Failure(format!("rustup command invalid: {error}")),
    };
    command.inherit_env();
    add_public_api_args(&mut command, package, manifest);
    classify_public_api_output(package, command.run_capture_raw())
}

fn add_public_api_args<'a>(command: &mut CommandIn<'a>, package: &'a str, manifest: &'a str) {
    command
        .arg("run")
        .arg(TOOLCHAIN)
        .arg("cargo")
        .arg("public-api")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("-p")
        .arg(package)
        .arg("diff")
        .arg("origin/main..HEAD")
        .arg("--all-features")
        .arg("--deny")
        .arg("removed")
        .arg("--deny")
        .arg("changed");
}

fn classify_public_api_output(
    package: &str,
    output: Result<titania_lanes::CommandOutput, titania_lanes::LaneError>,
) -> PublicApiDiff {
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return PublicApiDiff::Failure(format!(
                "cargo public-api failed for {package}: {error}"
            ));
        }
    };
    if output.status.success() {
        return PublicApiDiff::Clean;
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let message = format!(
        "cargo public-api diff failed for {package} with code {:?}\n{stdout}{stderr}",
        output.status.code()
    );
    if is_public_api_missing(&message) {
        PublicApiDiff::Failure(message)
    } else {
        PublicApiDiff::Violation(message)
    }
}

fn is_public_api_missing(message: &str) -> bool {
    message.contains("no such command") || message.contains("cargo-public-api")
}

fn run_package_diffs(
    target: &TargetProject,
    packages: &[String],
    manifest: &str,
    report: &mut LaneReport,
) -> LaneExit {
    let mut exit_code = LaneExit::Clean;
    for package in packages {
        match run_public_api_diff(target, package, manifest) {
            PublicApiDiff::Clean => {}
            PublicApiDiff::Violation(message) => {
                report.push(Finding::new(RULE_PUBLIC_API_DIFF, package, 0, message));
                if exit_code == LaneExit::Clean {
                    exit_code = LaneExit::Violations;
                }
            }
            PublicApiDiff::Failure(message) => {
                report.push(Finding::new(RULE_PUBLIC_API_TOOL, package, 0, message));
                exit_code = LaneExit::Failure;
            }
        }
    }
    exit_code
}

fn usage_requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--help" || arg == "-h")
}

fn emit_usage() {
    eprintln!(
        "usage: check-public-api-diff\n\
         Discovers vb_* workspace packages and runs\n\
         `cargo public-api diff origin/main..HEAD` through CommandIn."
    );
}

fn resolve_target(report: &mut LaneReport) -> Result<TargetProject, LaneExit> {
    current_target_project().map_err(|error| {
        report.push(Finding::new(
            RULE_TARGET,
            "Cargo.toml",
            0,
            format!("target discovery failed: {error}"),
        ));
        eprint!("{}", report.render());
        LaneExit::Usage
    })
}

fn resolve_package_list(
    target: &TargetProject,
    report: &mut LaneReport,
) -> Result<Vec<String>, LaneExit> {
    discover_packages(target).map_err(|error| {
        let is_missing = error.contains("failed to start");
        let rule = if is_missing { RULE_CARGO_MISSING } else { RULE_METADATA };
        report.push(Finding::new(rule, "Cargo.toml", 0, error));
        eprint!("{}", report.render());
        if is_missing { LaneExit::Failure } else { LaneExit::Violations }
    })
}

fn report_no_packages() -> LaneExit {
    eprintln!(
        "NotApplicable: no vb_* or velvet-ballistics packages discovered in workspace metadata"
    );
    LaneExit::NotApplicable
}

fn run_diffs_and_emit(
    target: &TargetProject,
    packages: &[String],
    report: &mut LaneReport,
) -> LaneExit {
    let manifest = target.manifest_path();
    let exit_code = run_package_diffs(target, packages, manifest.as_str(), report);
    if exit_code != LaneExit::Clean {
        eprint!("{}", report.render());
    }
    exit_code
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if usage_requested(&args) {
        emit_usage();
        return exit(LaneExit::Usage);
    }

    let mut report = LaneReport::new();
    let target = match resolve_target(&mut report) {
        Ok(target) => target,
        Err(code) => return exit(code),
    };
    let packages = match resolve_package_list(&target, &mut report) {
        Ok(packages) => packages,
        Err(code) => return exit(code),
    };
    if packages.is_empty() {
        return exit(report_no_packages());
    }
    exit(run_diffs_and_emit(&target, &packages, &mut report))
}
