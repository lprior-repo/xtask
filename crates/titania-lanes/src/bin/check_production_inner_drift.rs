//! Identifier-diff verifier for verification/verus/production_inner mirrors + BINDING LEDGERs.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-production-inner-drift.sh`. Run via
//! `cargo run --bin check_production_inner_drift --` from the repository root or via the
//! matching Moon task in `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "check_production_inner_drift/claims.rs"]
mod claims;
#[path = "check_production_inner_drift/externs.rs"]
mod externs;
#[path = "check_production_inner_drift/identifiers.rs"]
mod identifiers;
#[path = "check_production_inner_drift/mirror.rs"]
mod mirror;

use titania_core::TargetProject;
use titania_lanes::{LaneExit, LaneReport, current_target_project, exit};

const MIRROR_DIR: &str = "verification/verus/production_inner";

fn main() -> std::process::ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-production-inner-drift] target discovery failed: {error}");
            return exit(LaneExit::Failure);
        }
    };
    let mut report = LaneReport::new();
    run(&target, &mut report);
    print_and_exit(&report)
}

fn run(target: &TargetProject, report: &mut LaneReport) {
    let root = target.as_std_path();
    mirror::per_mirror_pass(root, MIRROR_DIR, report);
    externs::per_extern_pass(root, report);
}

fn print_and_exit(report: &LaneReport) -> std::process::ExitCode {
    let rendered = report.render();
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    if report.is_clean() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}
