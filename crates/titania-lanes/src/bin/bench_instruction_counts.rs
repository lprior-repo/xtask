//! perf-stat-driven instruction-count benchmark runner with evidence dir.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/bench-instruction-counts.sh`. Run via
//! `cargo run --bin bench_instruction_counts -- [bench-name ...]` from the
//! repository root or via the matching Moon task in `.moon/tasks/all.yml`.
//!
//! Exit codes: 0 = all benches produced non-empty `.perf.log` files or the
//! lane is not applicable, 1 = perf log empty, 2 = usage error, 3 = missing perf binary.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

#[path = "bench_instruction_counts/lane.rs"]
mod lane;

fn main() -> std::process::ExitCode {
    lane::main_exit(std::env::args().skip(1).collect())
}
