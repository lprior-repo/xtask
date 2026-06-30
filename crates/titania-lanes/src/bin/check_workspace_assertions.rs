//! Validates Cargo workspace shape: members, package names, and forbidden
//! dependencies / feature names.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-workspace-assertions.sh`. Run via
//! `cargo run --bin check-workspace-assertions --` from the repository root
//! or via the matching Moon task in `.moon/tasks/all.yml`.
//!
//! Scan domain: the workspace `Cargo.toml` and one manifest per workspace
//! member under `crates/`. Exclusions: `target/`, `target/miri-tmp`, and
//! any `vb_ui` / `fuzz` artifacts carried over from the velvet-ballistics
//! legacy tree.
//!
//! This port inherits the same rule shape as the velvet-ballistics original
//! (boundary crates may not depend on UI crates, runtime format crates, or
//! use a fixed list of forbidden feature names) but reads its expected
//! member list, package-name map, and feature table from the live titania
//! workspace. That keeps the lane a pure structural assertion: it does
//! NOT depend on the velvet-ballistics `vb_*` crate set.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]
#![allow(clippy::filter_map_bool_then)]

#[path = "check_workspace_assertions/mod.rs"]
mod workspace_assertions;

fn main() -> std::process::ExitCode {
    workspace_assertions::main_exit()
}
