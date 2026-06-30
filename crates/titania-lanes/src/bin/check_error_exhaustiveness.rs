//! Compares fuzz oracle function bodies vs production error enum definitions.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-error-exhaustiveness.sh`. Run via
//! `cargo run --bin check-error-exhaustiveness --` from the repository root or via the
//! matching Moon task in `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "check_error_exhaustiveness/check.rs"]
mod check;
#[path = "check_error_exhaustiveness/model.rs"]
mod model;
#[path = "check_error_exhaustiveness/parser.rs"]
mod parser;

use std::process::ExitCode;

use titania_lanes::{LaneExit, LaneReport, current_target_project, exit};

fn main() -> ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-error-exhaustiveness] target discovery failed: {error}");
            return exit(LaneExit::Failure);
        }
    };
    let mut report = LaneReport::new();
    check::run(&target, &mut report);
    print_and_exit(&report)
}

fn print_and_exit(report: &LaneReport) -> ExitCode {
    let rendered = report.render();
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    if report.is_clean() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}
