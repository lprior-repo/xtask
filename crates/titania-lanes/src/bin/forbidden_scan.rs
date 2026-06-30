//! Walks `crates/*/src` for the Holzman Rust forbidden surface.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/forbidden-scan.sh`. Run via
//! `cargo run --bin forbidden-scan --` from the repository root or via
//! the matching Moon task in `.moon/tasks/all.yml`.
//!
//! ## Behavior parity
//! The bash lane spins up a throwaway cargo project that re-uses
//! `xtask::forbidden_scan` as a module. The Rust re-implementation
//! folds the same logic into this binary so there is no longer a
//! dependency on `xtask` being buildable at scan time. We scan
//! `crates/*/src` for the default forbidden set:
//!
//! - `panic!`              (Holzman rule 2)
//! - `unwrap()`            (Holzman rule 4)
//! - `expect()`            (Holzman rule 4)
//! - `todo!`               (Holzman rule 5)
//! - `unimplemented!`      (Holzman rule 5)
//! - `dbg!`                (Holzman rule 9)
//!
//! Pass `--forbidden=token1,token2,...` to override the default set.
//!
//! Each match becomes a typed `Finding`. Findings flow through
//! `LaneReport::render` so the lane output is identical to every other
//! titania lane.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "forbidden_scan/lane.rs"]
mod lane;
#[path = "forbidden_scan/source_line.rs"]
mod source_line;

fn main() -> std::process::ExitCode {
    lane::main_exit(std::env::args().skip(1).collect())
}
