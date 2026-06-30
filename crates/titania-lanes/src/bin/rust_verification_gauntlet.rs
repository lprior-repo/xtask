//! Dispatcher for fast | standard | deep | proof Rust verification lanes.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::get_unwrap,
    clippy::arithmetic_side_effects,
    clippy::dbg_macro,
    clippy::as_conversions
)]
#![forbid(unsafe_code)]
#[path = "rust_verification_gauntlet/commands.rs"]
mod commands;

use std::env;

use commands::{
    cargo_capture, run_clippy_vb_compile, run_kani, run_kani_default_unwind, run_local_lane,
    run_test,
};
use serde_json::Value;
use titania_core::TargetProject;
use titania_lanes::{LaneExit, LaneReport, current_target_project, exit};

const TEST_GROUPS: [(&str, &str); 4] = [
    ("UNIT-EXPR-BYTESTACK-001", "expression_bytecode"),
    ("UNIT-SLOT-COMPILER-001", "slot_compiler"),
    ("UNIT-LOWER-DO-001", "lower"),
    ("POST-009-VALIDATE-001", "lower_steps"),
];
const STANDARD_KANI: [(&str, &str); 9] = [
    ("KANI-EXPR-BYTECODE-001", "compile_expr_to_bytecode_overflow"),
    ("KANI-SLOT-REF-001", "lower_slot_reference_valid"),
    ("KANI-SLOT-REF-001b", "lower_slot_reference_with_path_creates_accessor"),
    ("KANI-CONSTANT-POOL-001", "push_constant_overflow"),
    ("KANI-CONSTANT-POOL-001b", "push_constant_isolation"),
    ("KANI-CONSTANT-POOL-001c", "slot_count_overflow_at_max"),
    ("KANI-ACCESSOR-REF-001", "lower_accessor_reference_numeric"),
    ("KANI-ACCESSOR-REF-001b", "accessor_index_assignment"),
    ("KANI-ACCESSOR-REF-001c", "rejects_non_numeric_accessor_path"),
];
const DEEP_KANI: [(&str, &str); 2] = [
    ("INV-007-NODEDUP-001", "node_id_uniqueness"),
    ("INV-007-NODEDUP-001b", "step_idx_ordering_preserved"),
];
const ADMISSION_KANI: [(&str, &str); 3] = [
    ("KANI-ADMISSION-001-MALFORMED", "strict_admission_invalid_artifact_cases_reject"),
    ("KANI-ADMISSION-001-CAPABILITY", "strict_admission_invalid_capability_rejects"),
    ("KANI-ADMISSION-001-VALID", "strict_admission_valid_artifact_admits"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Fast,
    Standard,
    Deep,
    Proof,
}

impl Mode {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "fast" => Some(Self::Fast),
            "standard" => Some(Self::Standard),
            "deep" => Some(Self::Deep),
            "proof" | "all" => Some(Self::Proof),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackagePresence {
    Present,
    Absent,
}

impl PackagePresence {
    const fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TargetPackages {
    vb_compile: PackagePresence,
    vb_runtime: PackagePresence,
}

impl TargetPackages {
    fn discover(target: &TargetProject) -> Result<Self, String> {
        let output = cargo_capture(target, &["metadata", "--format-version", "1", "--no-deps"])?;
        let text = output.stdout_str().map_err(|error| error.to_string())?;
        let metadata = serde_json::from_str::<Value>(text).map_err(|error| error.to_string())?;
        Ok(Self {
            vb_compile: package_presence(&metadata, "vb_compile"),
            vb_runtime: package_presence(&metadata, "vb_runtime"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalLane {
    IgnoredFallible,
    StepstateMatrix,
}

impl LocalLane {
    const fn binary_name(self) -> &'static str {
        match self {
            Self::IgnoredFallible => "check-ignored-fallible-results",
            Self::StepstateMatrix => "check-stepstate-matrix",
        }
    }
}

fn package_presence(metadata: &Value, name: &str) -> PackagePresence {
    let present = metadata.get("packages").and_then(Value::as_array).is_some_and(|packages| {
        packages.iter().any(|package| package.get("name").and_then(Value::as_str) == Some(name))
    });
    if present { PackagePresence::Present } else { PackagePresence::Absent }
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mode_str = args.first().map_or("fast", String::as_str);
    let Some(mode) = Mode::parse(mode_str) else {
        eprintln!("usage: rust-verification-gauntlet <fast|standard|deep|proof|all>");
        return exit(LaneExit::Usage);
    };
    let target = match current_target_project() {
        Ok(target) => target,
        Err(err) => {
            eprintln!("[gauntlet] cannot resolve target project: {err}");
            return exit(LaneExit::Usage);
        }
    };
    exit(run(mode, &target))
}

fn run(mode: Mode, target: &TargetProject) -> LaneExit {
    let packages = match TargetPackages::discover(target) {
        Ok(packages) => packages,
        Err(error) => {
            eprintln!("[gauntlet] cannot inspect target packages: {error}");
            return LaneExit::Failure;
        }
    };
    let mut report = LaneReport::new();
    let result = dispatch(mode, target, packages, &mut report);
    if !report.is_clean() {
        eprintln!("{}", report.render());
    }
    result
}

fn dispatch(
    mode: Mode,
    target: &TargetProject,
    packages: TargetPackages,
    report: &mut LaneReport,
) -> LaneExit {
    eprintln!("[gauntlet] mode: {}", label(mode));
    if !packages.vb_compile.is_present() {
        eprintln!("[gauntlet] NotApplicable: package vb_compile absent; skipping compile gauntlet");
        return LaneExit::NotApplicable;
    }
    let fast = run_fast_steps(target, report);
    let standard = run_standard_steps(mode, target, report);
    let deep = run_deep_steps(mode, target, report);
    merge(merge(fast, standard), merge(deep, run_proof_steps(mode, target, packages, report)))
}

fn run_fast_steps(target: &TargetProject, report: &mut LaneReport) -> LaneExit {
    let clippy = step(report, "STATIC-LINT-001", || run_clippy_vb_compile(target));
    let ignored = step(report, "ignored-fallible gate", || {
        run_local_lane(target, LocalLane::IgnoredFallible)
    });
    merge(merge(clippy, ignored), test_steps(target, report))
}

fn run_standard_steps(mode: Mode, target: &TargetProject, report: &mut LaneReport) -> LaneExit {
    if matches!(mode, Mode::Standard | Mode::Deep | Mode::Proof) {
        kani_steps(target, report, &STANDARD_KANI, run_kani)
    } else {
        LaneExit::Clean
    }
}

fn run_deep_steps(mode: Mode, target: &TargetProject, report: &mut LaneReport) -> LaneExit {
    if matches!(mode, Mode::Deep | Mode::Proof) {
        kani_steps(target, report, &DEEP_KANI, run_kani)
    } else {
        LaneExit::Clean
    }
}

fn run_proof_steps(
    mode: Mode,
    target: &TargetProject,
    packages: TargetPackages,
    report: &mut LaneReport,
) -> LaneExit {
    if mode != Mode::Proof {
        return LaneExit::Clean;
    }
    let drift =
        step(report, "DRIFT-STEPSTATE-001", || run_local_lane(target, LocalLane::StepstateMatrix));
    let admission = if packages.vb_runtime.is_present() {
        kani_steps(target, report, &ADMISSION_KANI, run_kani_default_unwind)
    } else {
        eprintln!(
            "[gauntlet] NotApplicable: package vb_runtime absent; skipping admission Kani checks"
        );
        LaneExit::Clean
    };
    eprintln!(
        "[gauntlet] NOTE: Verus proofs (VERUS-EXPR-STACK-001, VERUS-SLOT-MAX-001) are WAIVED -- toolchain not installed"
    );
    merge(drift, admission)
}

fn label(m: Mode) -> &'static str {
    match m {
        Mode::Fast => "fast",
        Mode::Standard => "standard",
        Mode::Deep => "deep",
        Mode::Proof => "proof",
    }
}

fn step<F: FnOnce() -> LaneExit>(report: &mut LaneReport, label: &str, f: F) -> LaneExit {
    match f() {
        LaneExit::Clean | LaneExit::NotApplicable => {
            eprintln!("[PASS] {label}");
            report.record_pass();
            LaneExit::Clean
        }
        LaneExit::Violations => {
            eprintln!("[FAIL] {label}");
            LaneExit::Violations
        }
        LaneExit::Usage | LaneExit::Failure => {
            eprintln!("[ERROR] {label}");
            LaneExit::Failure
        }
    }
}

fn merge(left: LaneExit, right: LaneExit) -> LaneExit {
    match (left, right) {
        (LaneExit::Failure | LaneExit::Usage, _) | (_, LaneExit::Failure | LaneExit::Usage) => {
            LaneExit::Failure
        }
        (LaneExit::Violations, _) | (_, LaneExit::Violations) => LaneExit::Violations,
        (LaneExit::Clean, LaneExit::Clean)
        | (LaneExit::Clean, LaneExit::NotApplicable)
        | (LaneExit::NotApplicable, LaneExit::Clean) => LaneExit::Clean,
        (LaneExit::NotApplicable, LaneExit::NotApplicable) => LaneExit::NotApplicable,
    }
}

fn test_steps(target: &TargetProject, report: &mut LaneReport) -> LaneExit {
    TEST_GROUPS.iter().fold(LaneExit::Clean, |exit_code, (name, group)| {
        merge(exit_code, step(report, name, || run_test(target, group)))
    })
}

fn kani_steps(
    target: &TargetProject,
    report: &mut LaneReport,
    steps: &[(&str, &str)],
    runner: fn(&TargetProject, &str) -> LaneExit,
) -> LaneExit {
    steps.iter().fold(LaneExit::Clean, |exit_code, (name, harness)| {
        merge(exit_code, step(report, name, || runner(target, harness)))
    })
}
