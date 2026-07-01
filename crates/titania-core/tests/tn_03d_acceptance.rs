//! Acceptance tests for the v1 domain model — directly from bead §4.
//!
//! Each test maps to a specific acceptance criterion from the bead spec.

#![allow(clippy::needless_borrow)]
#![allow(clippy::useless_vec)]
#![allow(clippy::as_conversions)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::type_complexity)]
#![allow(clippy::map_identity)]

use titania_core::{Digest, RuleId, TextRange, WorkspacePath};

// ===========================================================================
// A1. Happy path: Lane::AstGrep serde round-trip
// ===========================================================================

#[test]
fn a1_lane_astgrep_serde_round_trip() {
    let lane = "AstGrep".parse::<titania_core::Lane>().unwrap();
    let json = serde_json::to_string(&lane).unwrap();
    // Lane serializes to PascalCase string
    assert_eq!(json, "\"AstGrep\"");
    let back: titania_core::Lane = serde_json::from_str(&json).unwrap();
    assert_eq!(lane, back);
}

// ===========================================================================
// A2. Happy path: GateScope::Release serde round-trip
// ===========================================================================

#[test]
fn a2_gate_scope_release_serde_round_trip() {
    let scope = titania_core::GateScope::Release;
    let json = serde_json::to_string(&scope).unwrap();
    // GateScope serializes to snake_case string
    assert_eq!(json, "\"release\"");
    let back: titania_core::GateScope = serde_json::from_str(&json).unwrap();
    assert_eq!(scope, back);
}

// ===========================================================================
// A3. Happy path: Report::Reject with one Finding returns RejectKind::CodeOnly
// ===========================================================================

fn make_test_finding() -> titania_core::Finding {
    let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
    let loc = WorkspacePath::new("src/lib.rs").unwrap();
    titania_core::Finding::new(
        "Fmt".parse().unwrap(),
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

#[test]
fn a3_report_reject_code_findings_only_returns_code_only() {
    let findings: Box<[titania_core::Finding]> = Box::new([make_test_finding()]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let r = titania_core::Report::reject(findings, failures, per_lane).unwrap();
    assert!(matches!(r, titania_core::Report::Reject { .. }));
    assert_eq!(r.reject_kind(), Some(titania_core::RejectKind::CodeOnly));
}

// ===========================================================================
// A4. Error path: Report::Reject with empty code_findings and empty gate_failures
// ===========================================================================

#[test]
fn a4_report_reject_both_empty_is_rejected() {
    let findings: Box<[titania_core::Finding]> = Box::new([]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let result = titania_core::Report::reject(findings, failures, per_lane);
    assert!(
        matches!(result, Err(titania_core::ReportError::BothEmpty)),
        "BothEmpty must be returned when both collections are empty"
    );
}

// ===========================================================================
// A5. Error path: unknown lane name returns typed error
// ===========================================================================

#[test]
fn a5_unknown_lane_name_returns_error() {
    let result = "NonExistentLane".parse::<titania_core::Lane>();
    assert!(result.is_err());
    assert!(
        matches!(result, Err(titania_core::LaneError::Unknown(_))),
        "unknown lane must produce LaneError::Unknown"
    );
}

#[test]
fn a5_empty_lane_name_returns_error() {
    let result = "".parse::<titania_core::Lane>();
    assert!(result.is_err());
    assert!(
        matches!(result, Err(titania_core::LaneError::Unknown(_))),
        "empty lane name must produce LaneError::Unknown"
    );
}

// ===========================================================================
// A6. Edge case: GateScope::lanes returns 7 edit lanes in stable order
// ===========================================================================

#[test]
fn a6_gate_scope_edit_returns_7_lanes_in_order() {
    let edit = titania_core::GateScope::Edit;
    let lanes = edit.lanes();
    assert_eq!(lanes.len(), 7);
    // Verify exact ordering
    assert_eq!(lanes[0], "Fmt".parse().unwrap());
    assert_eq!(lanes[1], "Compile".parse().unwrap());
    assert_eq!(lanes[2], "Clippy".parse().unwrap());
    assert_eq!(lanes[3], "AstGrep".parse().unwrap());
    assert_eq!(lanes[4], "Dylint".parse().unwrap());
    assert_eq!(lanes[5], "PanicScan".parse().unwrap());
    assert_eq!(lanes[6], "PolicyScan".parse().unwrap());
}

#[test]
fn a6_gate_scope_lanes_stable_across_calls() {
    let edit = titania_core::GateScope::Edit;
    let mut lanes1 = edit.lanes().to_vec();
    let mut lanes2 = edit.lanes().to_vec();
    // Run many times to verify stability
    for _ in 0..100 {
        let current = edit.lanes();
        lanes1.retain(|l| !current.contains(l));
        lanes2.retain(|l| !current.contains(l));
        // All lanes should still be present
        assert_eq!(current.len(), 7);
    }
    assert!(lanes1.is_empty());
    assert!(lanes2.is_empty());
}

#[test]
fn a6_gate_scope_prepush_is_superset_of_edit() {
    let edit = titania_core::GateScope::Edit.lanes().to_vec();
    let prepush = titania_core::GateScope::Prepush.lanes();
    assert_eq!(prepush.len(), 9);
    // First 7 should match edit
    for (i, expected) in edit.iter().enumerate() {
        assert_eq!(prepush[i], *expected);
    }
    // Additional lanes: Test, Deny
    assert_eq!(prepush[7], "Test".parse().unwrap());
    assert_eq!(prepush[8], "Deny".parse().unwrap());
}

#[test]
fn a6_gate_scope_release_is_superset_of_prepush() {
    let prepush = titania_core::GateScope::Prepush.lanes().to_vec();
    let release = titania_core::GateScope::Release.lanes();
    assert_eq!(release.len(), 10);
    // First 9 should match prepush
    for (i, expected) in prepush.iter().enumerate() {
        assert_eq!(release[i], *expected);
    }
    // Additional lane: Build
    assert_eq!(release[9], "Build".parse().unwrap());
}

// ===========================================================================
// A7. Contract: Reject cannot contain two empty collections
// ===========================================================================

#[test]
fn a7_contract_reject_never_both_empty_property() {
    // Test the property across multiple scenarios
    let scenarios: Vec<(usize, usize, bool /* expect_ok */)> = vec![
        (0, 0, false),
        (1, 0, true),
        (0, 1, true),
        (1, 1, true),
        (5, 0, true),
        (0, 5, true),
        (5, 5, true),
    ];

    for (code_count, gate_count, expect_ok) in scenarios {
        let findings: Box<[titania_core::Finding]> = if code_count > 0 {
            let rule_id = RuleId::new("TEST_RULE").unwrap();
            let loc = WorkspacePath::new("src/lib.rs").unwrap();
            Box::new(
                std::iter::repeat_with(|| {
                    titania_core::Finding::new(
                        "Fmt".parse().unwrap(),
                        rule_id.clone(),
                        titania_core::Location::Span {
                            file: loc.clone(),
                            line_start: 1,
                            col_start: 0,
                            line_end: 1,
                            col_end: 10,
                        },
                        "msg".to_string(),
                        titania_core::RepairHint::use_iterator_pipeline("s".to_string()),
                        titania_core::FindingEffect::Reject,
                    )
                })
                .take(code_count)
                .collect::<Vec<_>>(),
            )
        } else {
            Box::new([])
        };

        let failures: Box<[titania_core::LaneFailure]> = if gate_count > 0 {
            Box::new(
                std::iter::repeat_with(|| {
                    titania_core::LaneFailure::infra_failure(
                        "cargo".to_string(),
                        "reason".to_string(),
                    )
                })
                .take(gate_count)
                .collect::<Vec<_>>(),
            )
        } else {
            Box::new([])
        };

        let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
        let result = titania_core::Report::reject(findings, failures, per_lane);

        if expect_ok {
            assert!(result.is_ok(), "reject({}, {}) should succeed", code_count, gate_count);
        } else {
            assert!(
                matches!(result, Err(titania_core::ReportError::BothEmpty)),
                "reject({}, {}) should return BothEmpty",
                code_count,
                gate_count
            );
        }
    }
}

// ===========================================================================
// A8. Contract: Finding owns Lane/RuleId/Location/RepairHint/FindingEffect
// ===========================================================================

#[test]
fn a8_finding_owns_all_fields() {
    let lane = "AstGrep".parse().unwrap();
    let rule_id = RuleId::new("FUNC_LOOPS_FOR").unwrap();
    let file = WorkspacePath::new("src/lib.rs").unwrap();
    let location = titania_core::Location::Span {
        file: file.clone(),
        line_start: 10,
        col_start: 5,
        line_end: 15,
        col_end: 20,
    };
    let message = "for loop should use iterators".to_string();
    let repair = titania_core::RepairHint::use_iterator_pipeline("convert to iterator".to_string());
    let effect = titania_core::FindingEffect::Reject;

    let finding = titania_core::Finding::new(
        lane,
        rule_id.clone(),
        location,
        message.clone(),
        repair,
        effect,
    );

    // Verify all fields are accessible and match
    assert_eq!(finding.lane(), &"AstGrep".parse().unwrap());
    assert_eq!(finding.rule_id(), &rule_id);
    assert_eq!(finding.message(), &message);
    assert_eq!(finding.effect(), &titania_core::FindingEffect::Reject);

    // Location is accessible
    match finding.location() {
        titania_core::Location::Span { file: f, line_start, col_start, line_end, col_end } => {
            assert_eq!(f.as_str(), "src/lib.rs");
            assert_eq!(*line_start, 10);
            assert_eq!(*col_start, 5);
            assert_eq!(*line_end, 15);
            assert_eq!(*col_end, 20);
        }
        other => panic!("expected Span, got {:?}", other),
    }

    // Repair is accessible
    match finding.repair() {
        titania_core::RepairHint::UseIteratorPipeline { .. } => {}
        other => panic!("expected UseIteratorPipeline, got {:?}", other),
    }
}

#[test]
fn a8_finding_clone_preserves_all() {
    let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
    let file = WorkspacePath::new("src/lib.rs").unwrap();
    let finding = titania_core::Finding::new(
        "Fmt".parse().unwrap(),
        rule_id,
        titania_core::Location::Span {
            file,
            line_start: 1,
            col_start: 0,
            line_end: 1,
            col_end: 10,
        },
        "msg".to_string(),
        titania_core::RepairHint::use_iterator_pipeline("s".to_string()),
        titania_core::FindingEffect::Reject,
    );
    let cloned = finding.clone();
    assert_eq!(finding, cloned);
    assert_eq!(finding.lane(), cloned.lane());
    assert_eq!(finding.rule_id(), cloned.rule_id());
    assert_eq!(finding.message(), cloned.message());
    assert_eq!(finding.effect(), cloned.effect());
}
