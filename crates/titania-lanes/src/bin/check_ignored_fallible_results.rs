//! DISCARD-001..006 scanner for fallible-call ignores across crates/*/src + xtask/src.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-ignored-fallible-results.sh`. Run via
//! `cargo run --bin check_ignored_fallible_results --` from the repository root or via the
//! matching Moon task in `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "check_ignored_fallible_results/allow.rs"]
mod allow;
#[path = "check_ignored_fallible_results/scan.rs"]
mod scan;
#[path = "check_ignored_fallible_results/source.rs"]
mod source;

use std::{path::Path, process::ExitCode};

use titania_lanes::{LaneExit, LaneReport, current_target_project, exit};

fn main() -> ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-ignored-fallible-results] cannot resolve target project: {error}");
            return exit(LaneExit::Usage);
        }
    };
    let mut report = LaneReport::new();
    run(target.as_std_path(), &mut report);
    print_and_exit(&report)
}

fn run(root: &Path, report: &mut LaneReport) {
    let allow = allow::load_allow(root, report);
    scan::scan(root, &allow, report);
}

fn print_and_exit(report: &LaneReport) -> ExitCode {
    let rendered = report.render();
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    if report.is_clean() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}
