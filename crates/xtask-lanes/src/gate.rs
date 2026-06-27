//! Gate runner: orchestrates lane execution for a given scope.

use xtask_core::{
    Finding, GateScope, Lane, LaneFailure, LaneOutcome, QualityReceipt, Report, SkipReason,
};

use crate::LaneRunner;

/// Runs all lanes for the given scope and produces a `Report`.
#[must_use]
pub fn run_gate(scope: GateScope, runners: &[Box<dyn LaneRunner>]) -> Report {
    let lanes = scope.lanes();
    let mut outcomes: Vec<(Lane, LaneOutcome)> = Vec::with_capacity(lanes.len());
    let mut code_findings: Vec<Finding> = Vec::new();
    let mut gate_failures: Vec<LaneFailure> = Vec::new();
    let mut compilation_failed = false;

    for &lane in lanes {
        let outcome = run_single_lane(lane, runners, compilation_failed);

        // Track compilation failure for short-circuiting
        if lane == Lane::Check && !matches!(outcome, LaneOutcome::Clean { .. }) {
            compilation_failed = true;
        }

        // Collect findings/failures
        match &outcome {
            LaneOutcome::Findings(findings) => {
                for f in findings {
                    if matches!(f.effect, xtask_core::FindingEffect::Reject) {
                        code_findings.push(f.clone());
                    }
                }
            }
            LaneOutcome::Failed(failure) => {
                gate_failures.push(failure.clone());
            }
            LaneOutcome::Clean { .. } | LaneOutcome::Skipped(_) => {}
        }

        outcomes.push((lane, outcome));
    }

    let per_lane: Box<[LaneOutcome]> = outcomes.iter().map(|(_, o)| o.clone()).collect();

    if code_findings.is_empty() && gate_failures.is_empty() {
        let receipt = QualityReceipt {
            schema_version: 1,
            scope,
            source_digest: xtask_core::Digest::from_bytes(b"placeholder-source"),
            cargo_lock_digest: xtask_core::Digest::from_bytes(b"placeholder-lock"),
            policy_digest: xtask_core::Digest::from_bytes(b"placeholder-policy"),
            toolchain_digest: xtask_core::Digest::from_bytes(b"placeholder-toolchain"),
            dependency_source_digest: None,
            advisory_db_digest: None,
            feature_profile_digest: None,
            mutation_baseline_digest: None,
            lanes: outcomes
                .iter()
                .filter_map(|(lane, outcome)| match outcome {
                    LaneOutcome::Clean { .. } => Some(xtask_core::LaneReceipt {
                        lane: *lane,
                        evidence_digest: xtask_core::Digest::from_bytes(b"placeholder"),
                        clean: true,
                    }),
                    _ => None,
                })
                .collect(),
        };
        Report::Pass { receipt, per_lane }
    } else {
        Report::Reject {
            code_findings: code_findings.into_boxed_slice(),
            gate_failures: gate_failures.into_boxed_slice(),
            per_lane,
        }
    }
}

fn run_single_lane(
    lane: Lane,
    runners: &[Box<dyn LaneRunner>],
    compilation_failed: bool,
) -> LaneOutcome {
    if compilation_failed && lane.depends_on_compilation() {
        return LaneOutcome::Skipped(SkipReason::PriorCompilationFailure);
    }

    for runner in runners {
        if runner.lane() == lane {
            return runner.run();
        }
    }

    LaneOutcome::Skipped(SkipReason::NotSelectedByScope)
}
