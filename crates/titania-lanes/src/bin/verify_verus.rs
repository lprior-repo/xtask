//! Runs verus over the production proof registry and emits trust-boundary reports.
//!
//! `contracts/proof_obligations.yaml` is authoritative only when it contains at
//! least one production proof obligation. The `formal_setup_smoke.rs` fixture may
//! remain in-tree as a Verus installation probe, but a smoke-only registry exits
//! with usage status instead of reporting production proof success.
//!
//! Strict Holzman Rust: no `unwrap`, no `expect`, no `panic`, no `unsafe`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::string_slice)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::arithmetic_side_effects)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::as_conversions)]
#![forbid(unsafe_code)]

#[path = "verify_verus/evidence.rs"]
mod evidence;
#[path = "verify_verus/outcome.rs"]
mod outcome;
#[path = "verify_verus/registry.rs"]
mod registry;
#[path = "verify_verus/trust.rs"]
mod trust;
#[path = "verify_verus/verus_tool.rs"]
mod verus_tool;
#[path = "verify_verus/walk.rs"]
mod walk;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use outcome::run_production_targets;
use registry::ProofTarget;
use titania_core::TargetProject;
use titania_lanes::{LaneExit, LaneReport, current_target_project, exit};

const REGISTRY_DEFAULT: &str = "contracts/proof_obligations.yaml";
const EVIDENCE_DIR_DEFAULT: &str = ".evidence/verus";
const SUMMARY_FILE: &str = "summary.txt";

struct LanePaths {
    registry: PathBuf,
    evidence_dir: PathBuf,
}

struct VerificationInputs {
    targets: Vec<ProofTarget>,
    summary_path: PathBuf,
}

fn main() -> std::process::ExitCode {
    let mut report = LaneReport::new();
    match run(&mut report) {
        LaneExit::Clean => exit(LaneExit::Clean),
        LaneExit::Violations => {
            eprintln!("{}", report.render());
            exit(LaneExit::Violations)
        }
        other => exit(other),
    }
}

fn run(report: &mut LaneReport) -> LaneExit {
    match run_checked(report) {
        Ok(exit_code) | Err(exit_code) => exit_code,
    }
}

fn run_checked(report: &mut LaneReport) -> Result<LaneExit, LaneExit> {
    let target = resolve_target()?;
    ensure_verus_on_path(&target)?;
    let paths = lane_paths(&target);
    let inputs = prepare_inputs(&target, &paths)?;
    if registry::contains_only_fixture_smoke(&inputs.targets) {
        return Ok(smoke_only_usage(&inputs.summary_path));
    }
    Ok(run_production_targets(report, &target, &paths.evidence_dir, &inputs))
}

fn resolve_target() -> Result<TargetProject, LaneExit> {
    match current_target_project() {
        Ok(target) => Ok(target),
        Err(error) => {
            eprintln!("[verify-verus] target discovery failed: {error}");
            Err(LaneExit::Usage)
        }
    }
}

fn ensure_verus_on_path(target: &TargetProject) -> Result<(), LaneExit> {
    if verus_tool::verus_on_path(target) {
        Ok(())
    } else {
        eprintln!("[verify-verus] verus not on PATH; formal verification cannot run");
        Err(LaneExit::Failure)
    }
}

fn prepare_inputs(
    target: &TargetProject,
    paths: &LanePaths,
) -> Result<VerificationInputs, LaneExit> {
    ensure_registry_has_content(&paths.registry)?;
    ensure_evidence_dir(&paths.evidence_dir)?;
    let targets = load_registry_targets(target, &paths.registry)?;
    ensure_targets_present(&targets, &paths.registry)?;
    let summary_path = paths.evidence_dir.join(SUMMARY_FILE);
    write_initial_summary(&summary_path, targets.len())?;
    Ok(VerificationInputs { targets, summary_path })
}

fn ensure_registry_has_content(registry: &Path) -> Result<(), LaneExit> {
    if registry::registry_path_is_nonempty(registry) {
        Ok(())
    } else {
        eprintln!(
            "[verify-verus] registry missing or empty: {}; formal obligations are required",
            registry.display()
        );
        Err(LaneExit::Usage)
    }
}

fn ensure_evidence_dir(evidence_dir: &Path) -> Result<(), LaneExit> {
    match fs::create_dir_all(evidence_dir) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("[verify-verus] cannot create evidence dir: {e}");
            Err(LaneExit::Failure)
        }
    }
}

fn load_registry_targets(
    target: &TargetProject,
    registry: &Path,
) -> Result<Vec<ProofTarget>, LaneExit> {
    match registry::parse_registry_targets(registry, target) {
        Ok(targets) => Ok(targets),
        Err(e) => {
            eprintln!("[verify-verus] registry parse failed: {e}");
            Err(LaneExit::Failure)
        }
    }
}

fn ensure_targets_present(targets: &[ProofTarget], registry: &Path) -> Result<(), LaneExit> {
    if targets.is_empty() {
        eprintln!(
            "[verify-verus] no targets discovered in {}; formal obligations are required",
            registry.display()
        );
        Err(LaneExit::Usage)
    } else {
        Ok(())
    }
}

fn write_initial_summary(summary_path: &Path, target_count: usize) -> Result<(), LaneExit> {
    match evidence::write_summary_header(summary_path, target_count) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("[verify-verus] cannot write summary: {e}");
            Err(LaneExit::Failure)
        }
    }
}

fn lane_paths(target: &TargetProject) -> LanePaths {
    LanePaths {
        registry: target_path(target, &env_value("VERUS_PROOF_REGISTRY", REGISTRY_DEFAULT)),
        evidence_dir: target_path(target, &env_value("VERUS_EVIDENCE_DIR", EVIDENCE_DIR_DEFAULT)),
    }
}

fn env_value(key: &str, default: &str) -> String {
    match env::var(key) {
        Ok(value) => value,
        Err(_) => default.to_owned(),
    }
}

fn target_path(target: &TargetProject, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() { path.to_path_buf() } else { target.as_std_path().join(path) }
}

fn smoke_only_usage(summary_path: &Path) -> LaneExit {
    eprintln!(
        "[verify-verus] only fixture smoke obligations discovered; production Verus obligations are required"
    );
    if let Err(e) = evidence::append_not_applicable(summary_path, "fixture-smoke-only") {
        eprintln!("[verify-verus] cannot append smoke-only summary: {e}");
        return LaneExit::Failure;
    }
    LaneExit::Usage
}
