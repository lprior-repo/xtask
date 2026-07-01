//! Verifies StepState enum variants cover VALID_TRANSITIONS matrix fully.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-stepstate-matrix.sh`. Run via
//! `cargo run --bin check-stepstate-matrix --` from the repository root or via the
//! matching Moon task in `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "check_stepstate_matrix/lane.rs"]
mod lane;

fn main() -> std::process::ExitCode {
    // Stage 4 Pattern D: validate every RULE_* literal at startup. If any
    // rule id is malformed (typo, missing underscore, too long, etc.) we
    // fail-closed here rather than discovering the typo at the first
    // Finding::push site.
    if let Err((index, error)) = titania_core::RuleId::validate_many(&[lane::RULE_STEPSTATE]) {
        eprintln!("[check-stepstate-matrix] invalid rule id at index {index}: {error}");
        return std::process::ExitCode::FAILURE;
    }
    lane::main_exit()
}
