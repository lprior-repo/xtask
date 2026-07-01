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
    lane::main_exit()
}
