use std::{fs, path::Path};

use titania_core::TargetProject;
use titania_lanes::{Finding, LaneExit, LaneReport};

use super::{VerificationInputs, evidence, trust, verus_tool};

const TRUST_FILE: &str = "trust-scan.txt";
const FORBIDDEN_FILE: &str = "trust-forbidden.txt";
const RULE_VERUS_TARGET: &str = "VERUS-TARGET-001";

pub(super) fn run_production_targets(
    report: &mut LaneReport,
    target: &TargetProject,
    evidence_dir: &Path,
    inputs: &VerificationInputs,
) -> LaneExit {
    let target_failures = collect_target_failures(target, &inputs.targets, evidence_dir, report);
    let forbidden = collect_forbidden_trust(target, evidence_dir, report);
    let external_markers = collect_external_markers(evidence_dir, target);
    let waiver_exists = trust::trusted_base_waiver_exists(evidence_dir);
    handle_external_markers(report, &external_markers, waiver_exists, evidence_dir);
    append_final_summary(
        &inputs.summary_path,
        &target_failures,
        forbidden.len(),
        &external_markers,
        waiver_exists,
    );
    if report.is_clean() { LaneExit::Clean } else { LaneExit::Violations }
}

fn collect_target_failures(
    target: &TargetProject,
    targets: &[super::registry::ProofTarget],
    evidence_dir: &Path,
    report: &mut LaneReport,
) -> Vec<String> {
    targets
        .iter()
        .filter_map(|proof_target| verus_target_failure(target, proof_target, evidence_dir, report))
        .collect()
}

fn verus_target_failure(
    target: &TargetProject,
    proof_target: &super::registry::ProofTarget,
    evidence_dir: &Path,
    report: &mut LaneReport,
) -> Option<String> {
    match verus_tool::run_verus_target(target, proof_target, evidence_dir) {
        Ok(()) => None,
        Err(e) => {
            record_target_failure(report, proof_target, &e);
            Some(format!("{}: {e}", proof_target.path()))
        }
    }
}

fn record_target_failure(
    report: &mut LaneReport,
    proof_target: &super::registry::ProofTarget,
    error: &str,
) {
    eprintln!("[verify-verus] target {} failed: {error}", proof_target.path());
    report.push(Finding::new(
        RULE_VERUS_TARGET,
        proof_target.path().to_owned(),
        0,
        error.to_owned(),
    ));
}

fn collect_forbidden_trust(
    target: &TargetProject,
    evidence_dir: &Path,
    report: &mut LaneReport,
) -> Vec<String> {
    let forbidden = trust::scan_forbidden_trust(target, report);
    if !forbidden.is_empty() {
        emit_forbidden_trust_file(evidence_dir, &forbidden);
    }
    forbidden
}

fn emit_forbidden_trust_file(evidence_dir: &Path, forbidden: &[String]) {
    let path = evidence_dir.join(FORBIDDEN_FILE);
    if let Err(e) = fs::write(&path, forbidden.join("\n")) {
        eprintln!("[verify-verus] cannot write forbidden-trust file {}: {e}", path.display());
    }
    eprintln!("[verify-verus] forbidden trust markers found; see {}", path.display());
}

fn collect_external_markers(evidence_dir: &Path, target: &TargetProject) -> Vec<String> {
    let external_markers = trust::scan_external_markers(target);
    if let Err(e) =
        evidence::write_external_marker_inventory(evidence_dir, TRUST_FILE, &external_markers)
    {
        eprintln!("[verify-verus] external-marker inventory failed: {e}");
    }
    external_markers
}

fn handle_external_markers(
    report: &mut LaneReport,
    external_markers: &[String],
    waiver_exists: bool,
    evidence_dir: &Path,
) {
    if external_markers.is_empty() || waiver_exists {
        return;
    }
    eprintln!(
        "[verify-verus] external Verus markers require trusted-base waiver artifact {}; see {}",
        evidence_dir.join(trust::TRUSTED_BASE_WAIVER_FILE).display(),
        evidence_dir.join(TRUST_FILE).display()
    );
    trust::report_unwaived_external_markers(report, external_markers);
}

fn append_final_summary(
    summary_path: &Path,
    target_failures: &[String],
    forbidden_count: usize,
    external_markers: &[String],
    waiver_exists: bool,
) {
    let status = evidence::SummaryStatus {
        target_failures,
        forbidden_count,
        external_marker_count: external_markers.len(),
        external_markers_waived: waiver_exists,
    };
    if let Err(e) = evidence::append_summary_status(summary_path, status) {
        eprintln!("[verify-verus] cannot append summary: {e}");
    }
}
