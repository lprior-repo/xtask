//! End-to-end tests for the v1 domain model.
//!
//! These exercise the full pipeline: construction → serialization → deserialization → classification.

#![allow(clippy::needless_borrow)]
#![allow(clippy::useless_vec)]
#![allow(clippy::as_conversions)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::type_complexity)]
#![allow(clippy::map_identity)]

use titania_core::{Digest, RuleId, TextRange, WorkspacePath};

fn make_finding_for_e2e(lane: &str) -> titania_core::Finding {
    let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
    let loc = WorkspacePath::new("src/lib.rs").unwrap();
    titania_core::Finding::new(
        lane.parse().unwrap(),
        rule_id,
        titania_core::Location::Span {
            file: loc,
            line_start: 1,
            col_start: 0,
            line_end: 1,
            col_end: 10,
        },
        "use iterators".to_string(),
        titania_core::RepairHint::use_iterator_pipeline("use .into_iter()".to_string()),
        titania_core::FindingEffect::Reject,
    )
}

fn make_lane_evidence() -> titania_core::LaneEvidence {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "fmt".to_string()].into_boxed_slice(),
    )
    .unwrap();
    titania_core::LaneEvidence::new(
        cmd,
        "rustfmt 1.84.0".to_string(),
        titania_core::ProcessTermination::exited(0),
        Digest::from_bytes(b"evidence"),
    )
}

fn make_quality_receipt(lanes: Vec<titania_core::LaneReceipt>) -> titania_core::QualityReceipt {
    let digest = Digest::from_bytes(b"test");
    titania_core::QualityReceipt::new(
        1,
        titania_core::GateScope::Edit,
        digest.clone(),
        digest.clone(),
        digest.clone(),
        digest.clone(),
        lanes.into_boxed_slice(),
    )
    .unwrap()
}

// ===========================================================================
// E1: Build Report::Reject with 2 findings + 1 gate failure, serialize,
//     deserialize, verify reject_kind() → Mixed, verify all lanes
// ===========================================================================

#[test]
fn e2e_build_report_reject_round_trip() {
    // Build a complex Reject report
    let finding1 = make_finding_for_e2e("Fmt");
    let finding2 = make_finding_for_e2e("Clippy");
    let findings: Box<[titania_core::Finding]> = Box::new([finding1, finding2]);

    let failure = titania_core::LaneFailure::tool_failure(
        "cargo".to_string(),
        titania_core::ProcessTermination::timed_out(),
    );
    let failures: Box<[titania_core::LaneFailure]> = Box::new([failure]);

    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([
        titania_core::LaneOutcome::findings(vec![make_finding_for_e2e("Fmt")].into_boxed_slice()),
        titania_core::LaneOutcome::clean(make_lane_evidence()).unwrap(),
        titania_core::LaneOutcome::failed(titania_core::LaneFailure::infra_failure(
            "clippy".to_string(),
            "not found".to_string(),
        )),
    ]);

    let report = titania_core::Report::reject(findings, failures, per_lane).unwrap();

    // Verify reject_kind before serialization
    assert_eq!(report.reject_kind(), Some(titania_core::RejectKind::Mixed));

    // Serialize to JSON
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.is_empty(), "JSON must not be empty");

    // Deserialize back
    let back: titania_core::Report = serde_json::from_str(&json).unwrap();

    // Verify reject_kind after round-trip
    assert_eq!(back.reject_kind(), Some(titania_core::RejectKind::Mixed));

    // Verify the structure is preserved (via Debug output comparison)
    assert_eq!(
        format!("{:?}", report),
        format!("{:?}", back),
        "Report structure must be preserved through round-trip"
    );

    // Verify we can inspect the Reject contents
    match back {
        titania_core::Report::Reject { per_lane: ref pl, .. } => {
            assert_eq!(pl.len(), 3);
        }
        _ => panic!("expected Reject, got {:?}", back),
    }
}

// ===========================================================================
// E2: Build Report::Pass with QualityReceipt (3 lanes), serialize,
//     deserialize, verify scope and lanes count
// ===========================================================================

#[test]
fn e2e_build_report_pass_round_trip() {
    let lanes = vec![
        titania_core::LaneReceipt {
            lane: "Fmt".parse().unwrap(),
            evidence_digest: Digest::from_bytes(b"lane1"),
            clean: true,
        },
        titania_core::LaneReceipt {
            lane: "Compile".parse().unwrap(),
            evidence_digest: Digest::from_bytes(b"lane2"),
            clean: true,
        },
        titania_core::LaneReceipt {
            lane: "Clippy".parse().unwrap(),
            evidence_digest: Digest::from_bytes(b"lane3"),
            clean: false,
        },
    ];
    let receipt = make_quality_receipt(lanes);

    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([
        titania_core::LaneOutcome::clean(make_lane_evidence()).unwrap(),
        titania_core::LaneOutcome::clean(make_lane_evidence()).unwrap(),
        titania_core::LaneOutcome::clean(make_lane_evidence()).unwrap(),
    ]);

    let report = titania_core::Report::pass(receipt, per_lane).unwrap();

    // Verify before serialization
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.is_empty());

    // Deserialize
    let back: titania_core::Report = serde_json::from_str(&json).unwrap();

    // Verify Pass variant
    match back {
        titania_core::Report::Pass { per_lane: ref pl, .. } => {
            assert_eq!(pl.len(), 3);
        }
        _ => panic!("expected Pass, got {:?}", back),
    }

    // Verify reject_kind is None for Pass
    assert!(matches!(back.reject_kind(), None));
}

// ===========================================================================
// E3: Build Report::Reject with LaneFailure::ToolFailure containing
//     ProcessTermination::TimedOut, serialize, deserialize, verify failure details
// ===========================================================================

#[test]
fn e2e_lane_failure_propagates_through_gate() {
    let finding = make_finding_for_e2e("Fmt");
    let findings: Box<[titania_core::Finding]> = Box::new([finding]);

    // Build a ToolFailure with TimedOut termination
    let failure = titania_core::LaneFailure::tool_failure(
        "clippy".to_string(),
        titania_core::ProcessTermination::timed_out(),
    );
    let failures: Box<[titania_core::LaneFailure]> = Box::new([failure]);

    let per_lane: Box<[titania_core::LaneOutcome]> =
        Box::new([titania_core::LaneOutcome::failed(titania_core::LaneFailure::tool_failure(
            "clippy".to_string(),
            titania_core::ProcessTermination::timed_out(),
        ))]);

    let report = titania_core::Report::reject(findings, failures, per_lane).unwrap();

    // Serialize and deserialize
    let json = serde_json::to_string(&report).unwrap();
    let back: titania_core::Report = serde_json::from_str(&json).unwrap();

    // Verify the failure details are preserved
    match back {
        titania_core::Report::Reject { gate_failures: ref gf, per_lane: ref pl, .. } => {
            assert_eq!(gf.len(), 1);
            assert_eq!(pl.len(), 1);

            // Verify the tool name is preserved
            match &gf[0] {
                titania_core::LaneFailure::ToolFailure { tool, termination } => {
                    assert_eq!(tool, "clippy");
                    assert!(matches!(**termination, titania_core::ProcessTermination::TimedOut));
                }
                other => panic!("expected ToolFailure, got {:?}", other),
            }

            // Verify the per_lane outcome
            match &pl[0] {
                titania_core::LaneOutcome::Failed(titania_core::LaneFailure::ToolFailure {
                    tool,
                    termination,
                }) => {
                    assert_eq!(tool, "clippy");
                    assert!(matches!(**termination, titania_core::ProcessTermination::TimedOut));
                }
                other => panic!("expected Failed(ToolFailure), got {:?}", other),
            }
        }
        _ => panic!("expected Reject, got {:?}", back),
    }

    // Verify reject_kind is CodeOnly (findings non-empty, failures non-empty = Mixed)
    // Actually: code_findings has 1 item, gate_failures has 1 item → Mixed
    assert_eq!(back.reject_kind(), Some(titania_core::RejectKind::Mixed));
}
