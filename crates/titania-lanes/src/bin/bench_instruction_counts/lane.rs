use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use titania_core::TargetProject;
use titania_lanes::{CommandIn, LaneExit, current_target_project, exit};

/// Default benches the bash lane exercised. Override by passing names on argv.
const DEFAULT_BENCHES: &[&str] = &["ir_traversal", "action_dispatch", "timer_wheel_tick"];

/// Toolchain pinned to the bash original.
const RUSTUP_TOOLCHAIN: &str = "nightly-2026-04-28";

/// `-p` target that owns the criterion benches.
const BENCH_PACKAGE: &str = "velvet-ballistics-workspace-tests";

const USAGE: &str = "usage: bench_instruction_counts [bench-name ...]\n  \
     default benches: ir_traversal, action_dispatch, timer_wheel_tick";

enum BenchPlan {
    Run(Vec<String>),
    NotApplicable(String),
}

pub(crate) fn main_exit(args: Vec<String>) -> ExitCode {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("{USAGE}");
        return exit(LaneExit::Clean);
    }
    let target = match target_project() {
        Ok(target) => target,
        Err(code) => return code,
    };
    let benches = match runnable_benches(&target, args) {
        Ok(benches) => benches,
        Err(code) => return code,
    };
    run_bench_plan(&target, &benches)
}

fn target_project() -> Result<TargetProject, ExitCode> {
    current_target_project().map_err(|err| {
        eprintln!("[bench-instruction-counts] cannot resolve target project: {err}");
        exit(LaneExit::Usage)
    })
}

fn runnable_benches(target: &TargetProject, args: Vec<String>) -> Result<Vec<String>, ExitCode> {
    match bench_plan(target, args) {
        Ok(BenchPlan::Run(benches)) => Ok(benches),
        Ok(BenchPlan::NotApplicable(reason)) => Err(not_applicable_exit(&reason)),
        Err(err) => Err(usage_error_exit(&err)),
    }
}

fn not_applicable_exit(reason: &str) -> ExitCode {
    eprintln!("[bench-instruction-counts] NotApplicable: {reason}");
    exit(LaneExit::NotApplicable)
}

fn usage_error_exit(err: &str) -> ExitCode {
    eprintln!("[bench-instruction-counts] {err}");
    exit(LaneExit::Usage)
}

fn run_bench_plan(target: &TargetProject, benches: &[String]) -> ExitCode {
    if let Err(code) = require_perf(target) {
        return code;
    }
    let Some((target_dir, evidence_dir)) = prepare_evidence_dirs(target) else {
        return exit(LaneExit::Failure);
    };
    benches
        .iter()
        .map(|bench| run_one_bench(target, &target_dir, &evidence_dir, bench))
        .find(|code| *code != LaneExit::Clean)
        .map_or_else(|| exit(LaneExit::Clean), exit)
}

fn require_perf(target: &TargetProject) -> Result<(), ExitCode> {
    let mut perf_check = CommandIn::new(target, "perf").map_err(|err| {
        eprintln!("[bench-instruction-counts] failed to prepare perf check: {err}");
        exit(LaneExit::Failure)
    })?;
    perf_check.inherit_env().arg("--version");
    perf_check.run_capture_raw().map(|_| ()).map_err(|_| missing_perf_exit())
}

fn missing_perf_exit() -> ExitCode {
    eprintln!("Missing required instruction counter: perf");
    exit(LaneExit::Failure)
}

fn prepare_evidence_dirs(target: &TargetProject) -> Option<(PathBuf, PathBuf)> {
    let target_dir = target.as_std_path().join("target/bench-instruction-counts");
    let evidence_dir = target_dir.join("evidence");
    if let Err(e) = fs::create_dir_all(&evidence_dir) {
        eprintln!("[bench-instruction-counts] could not create evidence dir: {e}");
        return None;
    }
    Some((target_dir, evidence_dir))
}

fn run_one_bench(
    target: &TargetProject,
    target_dir: &Path,
    evidence_dir: &Path,
    bench: &str,
) -> LaneExit {
    if bench.is_empty() {
        eprintln!("Empty benchmark name is not allowed.");
        return LaneExit::Usage;
    }
    let log_file = evidence_dir.join(format!("{bench}.perf.log"));
    eprintln!("[bench-instruction-counts] running {bench}");
    if let Err(code) = run_compile(target, target_dir, bench, &[])
        .and_then(|()| run_perf_stat(target, target_dir, bench, &log_file))
    {
        eprintln!("[bench-instruction-counts] {bench} failed: {code}");
        return LaneExit::Violations;
    }
    if is_non_empty(&log_file) { LaneExit::Clean } else { empty_log_exit(&log_file) }
}

fn empty_log_exit(log_file: &Path) -> LaneExit {
    eprintln!("Instruction-count log is empty: {}", log_file.display());
    LaneExit::Violations
}

fn bench_plan(target: &TargetProject, args: Vec<String>) -> Result<BenchPlan, String> {
    if !package_manifest(target).is_file() {
        return Ok(BenchPlan::NotApplicable(
            "benchmark package velvet-ballistics-workspace-tests is absent".to_owned(),
        ));
    }
    let requested = requested_benches(args)?;
    let available = available_benches(target, requested);
    if available.is_empty() {
        Ok(BenchPlan::NotApplicable(
            "target project has no requested instruction-count benches".to_owned(),
        ))
    } else {
        Ok(BenchPlan::Run(available))
    }
}

fn package_manifest(target: &TargetProject) -> PathBuf {
    target.as_std_path().join("crates/velvet-ballistics-workspace-tests/Cargo.toml")
}

fn requested_benches(args: Vec<String>) -> Result<Vec<String>, String> {
    let requested = if args.is_empty() {
        DEFAULT_BENCHES.iter().map(|bench| (*bench).to_owned()).collect()
    } else {
        args
    };
    if requested.iter().any(|bench| bench.is_empty()) {
        Err("empty benchmark name is not allowed".to_owned())
    } else {
        Ok(requested)
    }
}

fn available_benches(target: &TargetProject, requested: Vec<String>) -> Vec<String> {
    let benches_root =
        target.as_std_path().join("crates/velvet-ballistics-workspace-tests/benches");
    requested
        .into_iter()
        .filter(|bench| benches_root.join(format!("{bench}.rs")).is_file())
        .collect()
}

/// `cargo bench --bench NAME --all-features --no-run` (via rustup).
fn run_compile(
    target: &TargetProject,
    target_dir: &Path,
    bench: &str,
    extra: &[&str],
) -> Result<(), String> {
    let target_dir_value = target_dir.display().to_string();
    let mut cmd = CommandIn::new(target, "rustup")
        .map_err(|e| format!("failed to prepare cargo bench --no-run: {e}"))?;
    cmd.inherit_env();
    append_compile_args(&mut cmd, bench, &target_dir_value, extra);
    run_with_status(&cmd, "cargo bench --no-run")
}

fn append_compile_args<'a>(
    cmd: &mut CommandIn<'a>,
    bench: &'a str,
    target_dir: &'a str,
    extra: &'a [&'a str],
) {
    cmd.arg("run").arg(RUSTUP_TOOLCHAIN).arg("cargo").arg("bench");
    cmd.args(&["-p", BENCH_PACKAGE]).arg("--bench").arg(bench);
    cmd.args(&["--all-features"]).arg("--no-run");
    cmd.env("CARGO_TARGET_DIR", target_dir).args(extra);
}

/// `perf stat -x, -e instructions -- rustup run nightly-… cargo bench -- --bench`
fn run_perf_stat(
    target: &TargetProject,
    target_dir: &Path,
    bench: &str,
    log_file: &Path,
) -> Result<(), String> {
    let target_dir_value = target_dir.display().to_string();
    let log_file_value = log_file.display().to_string();
    let mut cmd = CommandIn::new(target, "perf")
        .map_err(|e| format!("failed to prepare perf stat cargo bench: {e}"))?;
    cmd.inherit_env();
    append_perf_args(&mut cmd, bench, &target_dir_value, &log_file_value);
    run_with_status(&cmd, "perf stat cargo bench")
}

fn append_perf_args<'a>(
    cmd: &mut CommandIn<'a>,
    bench: &'a str,
    target_dir: &'a str,
    log_file: &'a str,
) {
    cmd.args(&["stat", "-x,", "-e", "instructions", "-o"]).arg(log_file);
    cmd.arg("--").arg("rustup").arg("run").arg(RUSTUP_TOOLCHAIN);
    cmd.arg("cargo").arg("bench").args(&["-p", BENCH_PACKAGE]);
    cmd.arg("--bench").arg(bench).args(&["--all-features"]);
    cmd.arg("--").arg("--bench").env("CARGO_TARGET_DIR", target_dir);
}

fn run_with_status(cmd: &CommandIn<'_>, label: &str) -> Result<(), String> {
    let status = cmd.run_status_raw().map_err(|e| format!("failed to spawn {label}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed with exit {:?}", status.code()))
    }
}

fn is_non_empty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.len() > 0)
}
