//! Walks `crates/*/src` for forbidden API surface in hot paths.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-hot-cold-forbidden-apis.sh`. Run via
//! `cargo run --bin check-hot-cold-forbidden-apis -- [--self-test]` from
//! the repository root, or via the matching Moon task in
//! `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]
#![allow(clippy::filter_map_bool_then)]
#![allow(clippy::manual_contains)]
#![allow(clippy::type_complexity)]

#[path = "check_hot_cold_forbidden_apis/allow_file.rs"]
mod allow_file;
#[path = "check_hot_cold_forbidden_apis/model.rs"]
mod model;
#[path = "check_hot_cold_forbidden_apis/scan.rs"]
mod scan;
#[path = "check_hot_cold_forbidden_apis/selftest.rs"]
mod selftest;
#[path = "check_hot_cold_forbidden_apis/syntax.rs"]
mod syntax;

use std::process::ExitCode;

use model::{FindingData, HOT_CRATES};
use titania_lanes::{Finding, LaneExit, LaneReport, current_target_project, exit};

const RULE_INVALID_INVOCATION: &str = "HC-INVOCATION-001";
const RULE_VIOLATION: &str = "HC-VIOLATION-001";
const RULE_FIXTURE: &str = "HC-FIXTURE-001";

fn push_finding(report: &mut LaneReport, finding: &FindingData, rule: &'static str) {
    report.push(Finding::new(
        rule,
        finding.rel_path.clone(),
        finding.line_no_as_u32(),
        format!("{}: {}", finding.class_id, finding.text),
    ));
}

fn print_help() {
    eprintln!(
        "usage: check-hot-cold-forbidden-apis [--self-test]\n\
         Scans crates/<boundary>/src for forbidden API surface in hot\n\
         paths. Honors scripts/hot-cold-forbidden-apis.allow."
    );
}

fn usage_error(error: impl std::fmt::Display) -> ExitCode {
    let mut report = LaneReport::new();
    report.push(Finding::new(
        RULE_INVALID_INVOCATION,
        ".",
        0,
        format!("InvalidInvocation: cannot resolve target project: {error}"),
    ));
    eprint!("{}", report.render());
    exit(LaneExit::Usage)
}

fn fixture_error(error: String) -> ExitCode {
    let mut report = LaneReport::new();
    report.push(Finding::new(RULE_FIXTURE, ".", 0, error));
    eprint!("{}", report.render());
    exit(LaneExit::Failure)
}

fn print_scan_results(classified: &[String], justified: &[FindingData]) {
    classified.iter().for_each(|line| println!("{line}"));
    justified.iter().for_each(|finding| {
        println!(
            "JustifiedException|{}|{}|line={}",
            finding.class_id, finding.rel_path, finding.line_no
        );
    });
}

fn print_summary(classified: &[String], violations: &[FindingData], justified: &[FindingData]) {
    println!(
        "ScanSummary|hot_crates={}|classified={}|violations={}|justified={}",
        HOT_CRATES.join(","),
        classified.len(),
        violations.len(),
        justified.len()
    );
}

fn finish_scan(
    classified: Vec<String>,
    violations: Vec<FindingData>,
    justified: Vec<FindingData>,
) -> ExitCode {
    print_scan_results(&classified, &justified);
    let mut report = LaneReport::new();
    violations.iter().for_each(|finding| push_finding(&mut report, finding, RULE_VIOLATION));
    eprint!("{}", report.render());
    print_summary(&classified, &violations, &justified);
    if violations.is_empty() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}

fn run_lane() -> ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => return usage_error(error),
    };
    match scan::scan(target.as_std_path()) {
        Ok((classified, violations, justified)) => finish_scan(classified, violations, justified),
        Err(error) => fixture_error(error),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return exit(LaneExit::Usage);
    }
    if args.iter().any(|arg| arg == "--self-test") {
        return exit(if selftest::self_test() == 0 {
            LaneExit::Clean
        } else {
            LaneExit::Violations
        });
    }
    run_lane()
}
