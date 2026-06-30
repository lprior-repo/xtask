//! Runs TLC model checker over every `verification/tla/*.tla` with matching
//! `.cfg` companion.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/run-tlc-checks.sh`. Run via
//! `cargo run --bin run-tlc-checks --` from the repository root or via
//! the matching Moon task in `.moon/tasks/all.yml`.
//!
//! ## Behavior parity
//! For each `verification/tla/<name>.cfg` that has a matching
//! `<name>.tla`, spawn `java -cp <jar> tlc2.TLC -seed 0 -config <cfg> <tla>`
//! and print the last 3 lines of the output. The bash's hard-coded jar
//! path is preserved; the lane is a thin CLI wrapper that does not need
//! the rest of the workspace to be buildable.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "run_tlc_checks/lane.rs"]
mod lane;

fn main() -> std::process::ExitCode {
    lane::main_exit()
}
