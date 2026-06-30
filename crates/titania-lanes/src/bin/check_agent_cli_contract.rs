//! Validates agent-CLI contract literals in `crates/vb_cli/src` and the
//! master plan doc.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-agent-cli-contract.sh`. Run via
//! `cargo run --bin check-agent-cli-contract --` from the repository root or
//! via the matching Moon task in `.moon/tasks/all.yml`.
//!
//! Scan domain: `crates/vb_cli/src` (recursive) plus the
//! `velvet-ballistics-MASTER.md` plan document. Exclusions: build outputs,
//! `.git`, and `target/`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "check_agent_cli_contract/lane.rs"]
mod lane;

fn main() -> std::process::ExitCode {
    lane::main_exit()
}
