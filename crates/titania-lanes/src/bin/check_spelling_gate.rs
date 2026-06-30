//! Mechanical gate: rejects the wrong spelling `velvet-ballistics` in
//! active code/docs. Canonical is `velvet-ballistics`.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-spelling-gate.sh`. Run via
//! `cargo run --bin check-spelling-gate --` from the repository root or
//! via the matching Moon task in `.moon/tasks/all.yml`.
//!
//! ## Behavior parity
//! Mirrors the bash's two-stage filter:
//!
//! 1. **Path exclusions** — skip `.beads/`, `.jj/`, `.evidence/`,
//!    `target/`, `tests/`, `benches/`, the master file itself, the
//!    script itself, JJ workspaces, naming-scan fixtures, and other
//!    helper files. (`*/tests/*`, `*/benches/*`, `*/naming_scan/*` etc.)
//! 2. **Content allowlist** — even when the path is included, lines
//!    that contain the master file reference, the source checkout
//!    path, `FORBIDDEN_FEATURE_NAMES`, the rule statement
//!    `"velvet-ballistics"` is invalid, the dolthub remote URL, or the
//!    `velvet-ballistics/v2` test data are skipped.
//!
//! File extensions match the bash: `*.rs`, `*.toml`, `*.yaml`, `*.yml`,
//! `*.md`, `*.sh`, `*.py`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "check_spelling_gate/lane.rs"]
mod lane;

fn main() -> std::process::ExitCode {
    lane::main_exit()
}
