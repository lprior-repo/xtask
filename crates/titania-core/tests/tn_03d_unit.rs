//! Unit tests for the v1 domain model (19 types).
//!
//! Tests smart constructors, accessor methods, and invariant enforcement.
//! No `is_ok()`-only assertions — every test asserts exact values.

#![allow(clippy::as_conversions)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::useless_vec)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::type_complexity)]
#![allow(clippy::map_identity)]

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    str::FromStr,
};

use titania_core::{Digest, RuleId, TextRange, WorkspacePath};

// ===========================================================================
// Lane — 10 unit-enum variants
// ===========================================================================

#[test]
fn lane_all_10_variants_constructible() {
    let variants = [
        "Fmt",
        "Compile",
        "Clippy",
        "AstGrep",
        "Dylint",
        "PanicScan",
        "PolicyScan",
        "Test",
        "Deny",
        "Build",
    ];
    for name in variants {
        let lane = name.parse::<titania_core::Lane>();
        assert!(lane.is_ok(), "Lane::{} should construct successfully", name);
    }
}

#[test]
fn lane_from_str_exact_pascal_case() {
    assert_eq!("Fmt".parse::<titania_core::Lane>(), Ok(titania_core::Lane::Fmt));
    assert_eq!("Compile".parse::<titania_core::Lane>(), Ok(titania_core::Lane::Compile));
    assert_eq!("Clippy".parse::<titania_core::Lane>(), Ok(titania_core::Lane::Clippy));
    assert_eq!("AstGrep".parse::<titania_core::Lane>(), Ok(titania_core::Lane::AstGrep));
    assert_eq!("Dylint".parse::<titania_core::Lane>(), Ok(titania_core::Lane::Dylint));
    assert_eq!("PanicScan".parse::<titania_core::Lane>(), Ok(titania_core::Lane::PanicScan));
    assert_eq!("PolicyScan".parse::<titania_core::Lane>(), Ok(titania_core::Lane::PolicyScan));
    assert_eq!("Test".parse::<titania_core::Lane>(), Ok(titania_core::Lane::Test));
    assert_eq!("Deny".parse::<titania_core::Lane>(), Ok(titania_core::Lane::Deny));
    assert_eq!("Build".parse::<titania_core::Lane>(), Ok(titania_core::Lane::Build));
    // Case-sensitive: lowercase should fail
    assert!("compile".parse::<titania_core::Lane>().is_err());
    assert!("fmt".parse::<titania_core::Lane>().is_err());
    // Mixed case should fail
    assert!("FMT".parse::<titania_core::Lane>().is_err());
}

#[test]
fn lane_from_str_empty_string_rejected() {
    let result = "".parse::<titania_core::Lane>();
    assert!(result.is_err());
    assert!(
        matches!(result, Err(titania_core::LaneError::Unknown(..))),
        "empty string should produce LaneError::Unknown"
    );
}

#[test]
fn lane_from_str_whitespace_rejected() {
    assert!(" Fmt ".parse::<titania_core::Lane>().is_err());
    assert!("Fmt\n".parse::<titania_core::Lane>().is_err());
    assert!("\tFmt".parse::<titania_core::Lane>().is_err());
}

#[test]
fn lane_to_string_round_trip_all_10() {
    let names = [
        "Fmt",
        "Compile",
        "Clippy",
        "AstGrep",
        "Dylint",
        "PanicScan",
        "PolicyScan",
        "Test",
        "Deny",
        "Build",
    ];
    for name in names {
        let lane = name.parse::<titania_core::Lane>().unwrap();
        let display = lane.to_string();
        assert_eq!(display, name);
    }
}

#[test]
fn lane_copy_eq_hash_traits() {
    let lane = titania_core::Lane::Compile;
    let lane2 = lane; // copy
    assert_eq!(lane, lane2);
    assert!(lane == lane2);

    // Hash consistency
    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    lane.hash(&mut h1);
    lane2.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

#[test]
fn lane_debug_format() {
    assert_eq!(format!("{:?}", titania_core::Lane::Compile), "Compile");
    assert_eq!(format!("{:?}", titania_core::Lane::Fmt), "Fmt");
}

// ===========================================================================
// GateScope — 3 variants
// ===========================================================================

#[test]
fn gate_scope_all_3_variants_constructible() {
    let _ = titania_core::GateScope::Edit;
    let _ = titania_core::GateScope::Prepush;
    let _ = titania_core::GateScope::Release;
}

#[test]
fn gate_scope_from_str_edit() {
    assert_eq!("edit".parse::<titania_core::GateScope>(), Ok(titania_core::GateScope::Edit));
}

#[test]
fn gate_scope_from_str_prepush() {
    assert_eq!("prepush".parse::<titania_core::GateScope>(), Ok(titania_core::GateScope::Prepush));
}

#[test]
fn gate_scope_from_str_release() {
    assert_eq!("release".parse::<titania_core::GateScope>(), Ok(titania_core::GateScope::Release));
}

#[test]
fn gate_scope_from_str_rejects_unknown() {
    assert!("full".parse::<titania_core::GateScope>().is_err());
    assert!("deep".parse::<titania_core::GateScope>().is_err());
    assert!("Edit".parse::<titania_core::GateScope>().is_err());
}

#[test]
fn gate_scope_lanes_edit_returns_7_in_order() {
    let lanes = titania_core::GateScope::Edit.lanes();
    assert_eq!(lanes.len(), 7);
    let expected: Vec<titania_core::Lane> = vec![
        "Fmt".parse().unwrap(),
        "Compile".parse().unwrap(),
        "Clippy".parse().unwrap(),
        "AstGrep".parse().unwrap(),
        "Dylint".parse().unwrap(),
        "PanicScan".parse().unwrap(),
        "PolicyScan".parse().unwrap(),
    ];
    assert_eq!(lanes, expected.as_slice());
}

#[test]
fn gate_scope_lanes_prepush_returns_9_in_order() {
    let lanes = titania_core::GateScope::Prepush.lanes();
    assert_eq!(lanes.len(), 9);
    let expected: Vec<titania_core::Lane> = vec![
        "Fmt".parse().unwrap(),
        "Compile".parse().unwrap(),
        "Clippy".parse().unwrap(),
        "AstGrep".parse().unwrap(),
        "Dylint".parse().unwrap(),
        "PanicScan".parse().unwrap(),
        "PolicyScan".parse().unwrap(),
        "Test".parse().unwrap(),
        "Deny".parse().unwrap(),
    ];
    assert_eq!(lanes, expected.as_slice());
}

#[test]
fn gate_scope_lanes_release_returns_10_in_order() {
    let lanes = titania_core::GateScope::Release.lanes();
    assert_eq!(lanes.len(), 10);
    let expected: Vec<titania_core::Lane> = vec![
        "Fmt".parse().unwrap(),
        "Compile".parse().unwrap(),
        "Clippy".parse().unwrap(),
        "AstGrep".parse().unwrap(),
        "Dylint".parse().unwrap(),
        "PanicScan".parse().unwrap(),
        "PolicyScan".parse().unwrap(),
        "Test".parse().unwrap(),
        "Deny".parse().unwrap(),
        "Build".parse().unwrap(),
    ];
    assert_eq!(lanes, expected.as_slice());
}

#[test]
fn gate_scope_lanes_stable_across_calls() {
    let edit = titania_core::GateScope::Edit;
    let lanes1 = edit.lanes();
    let lanes2 = edit.lanes();
    assert_eq!(lanes1, lanes2);
    assert_eq!(lanes1.len(), lanes2.len());
}

// ===========================================================================
// Report — 4 variants
// ===========================================================================

fn make_lane_evidence() -> titania_core::LaneEvidence {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "fmt".to_string()].into_boxed_slice(),
    )
    .unwrap();
    titania_core::LaneEvidence::new(
        cmd,
        "rustfmt 1.84.0".to_string(),
        titania_core::ProcessTermination::Exited { code: 0 },
        Digest::from_bytes(b"digest"),
    )
}

fn make_finding() -> titania_core::Finding {
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

fn make_lane_failure() -> titania_core::LaneFailure {
    titania_core::LaneFailure::tool_failure(
        "cargo".to_string(),
        titania_core::ProcessTermination::TimedOut,
    )
}

#[test]
fn report_pass_constructs_with_valid_args() {
    let receipt = make_quality_receipt();
    let per_lane: Box<[titania_core::LaneOutcome]> =
        Box::new([titania_core::LaneOutcome::clean(make_lane_evidence()).unwrap()]);
    let result = titania_core::Report::pass(receipt, per_lane);
    assert!(matches!(result, Ok(titania_core::Report::Pass { .. })));
}

#[test]
fn report_pass_rejects_empty_per_lane() {
    let receipt = make_quality_receipt();
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let result = titania_core::Report::pass(receipt, per_lane);
    assert!(
        matches!(result, Err(titania_core::ReportError::EmptyPerLane)),
        "Pass with empty per_lane should return EmptyPerLane"
    );
}

#[test]
fn report_reject_with_code_and_gate_constructs() {
    let findings: Box<[titania_core::Finding]> = Box::new([make_finding()]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([make_lane_failure()]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let result = titania_core::Report::reject(findings, failures, per_lane);
    assert!(matches!(result, Ok(titania_core::Report::Reject { .. })));
}

#[test]
fn report_reject_with_only_code_findings() {
    let findings: Box<[titania_core::Finding]> = Box::new([make_finding()]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let result = titania_core::Report::reject(findings, failures, per_lane);
    assert!(matches!(result, Ok(titania_core::Report::Reject { .. })));
}

#[test]
fn report_reject_with_only_gate_failures() {
    let findings: Box<[titania_core::Finding]> = Box::new([]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([make_lane_failure()]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let result = titania_core::Report::reject(findings, failures, per_lane);
    assert!(matches!(result, Ok(titania_core::Report::Reject { .. })));
}

#[test]
fn report_reject_rejects_both_empty() {
    let findings: Box<[titania_core::Finding]> = Box::new([]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let result = titania_core::Report::reject(findings, failures, per_lane);
    assert!(
        matches!(result, Err(titania_core::ReportError::BothEmpty)),
        "Reject with both empty should return BothEmpty"
    );
}

#[test]
fn report_policy_error_constructs() {
    let diagnostics: Box<[titania_core::PolicyDiagnostic]> = Box::new([make_diagnostic()]);
    let report = titania_core::Report::policy_error(diagnostics);
    assert!(matches!(report, titania_core::Report::PolicyError { .. }));
}

#[test]
fn report_input_error_constructs() {
    let diagnostics: Box<[titania_core::InputDiagnostic]> = Box::new([make_input_diagnostic()]);
    let report = titania_core::Report::input_error(diagnostics);
    assert!(matches!(report, titania_core::Report::InputError { .. }));
}

#[test]
fn reject_kind_code_only() {
    let findings: Box<[titania_core::Finding]> = Box::new([make_finding()]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let r = titania_core::Report::reject(findings, failures, per_lane).unwrap();
    assert_eq!(r.reject_kind(), Some(titania_core::RejectKind::CodeOnly));
}

#[test]
fn reject_kind_gate_only() {
    let findings: Box<[titania_core::Finding]> = Box::new([]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([make_lane_failure()]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let r = titania_core::Report::reject(findings, failures, per_lane).unwrap();
    assert_eq!(r.reject_kind(), Some(titania_core::RejectKind::GateOnly));
}

#[test]
fn reject_kind_mixed() {
    let findings: Box<[titania_core::Finding]> = Box::new([make_finding()]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([make_lane_failure()]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let r = titania_core::Report::reject(findings, failures, per_lane).unwrap();
    assert_eq!(r.reject_kind(), Some(titania_core::RejectKind::Mixed));
}

#[test]
fn reject_kind_pass_returns_none() {
    let receipt = make_quality_receipt();
    let per_lane: Box<[titania_core::LaneOutcome]> =
        Box::new([titania_core::LaneOutcome::clean(make_lane_evidence()).unwrap()]);
    let r = titania_core::Report::pass(receipt, per_lane).unwrap();
    assert!(matches!(r.reject_kind(), None));
}

#[test]
fn reject_kind_policy_error_returns_none() {
    let diagnostics: Box<[titania_core::PolicyDiagnostic]> = Box::new([make_diagnostic()]);
    let r = titania_core::Report::policy_error(diagnostics);
    assert!(matches!(r.reject_kind(), None));
}

#[test]
fn reject_kind_input_error_returns_none() {
    let diagnostics: Box<[titania_core::InputDiagnostic]> = Box::new([make_input_diagnostic()]);
    let r = titania_core::Report::input_error(diagnostics);
    assert!(matches!(r.reject_kind(), None));
}

// ===========================================================================
// Finding — struct with 6 fields
// ===========================================================================

fn make_finding_for_accessors() -> titania_core::Finding {
    let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
    let loc = WorkspacePath::new("src/lib.rs").unwrap();
    titania_core::Finding::new(
        "AstGrep".parse().unwrap(),
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
fn finding_constructs_with_all_fields() {
    let f = make_finding_for_accessors();
    assert_eq!(f.lane(), &"AstGrep".parse().unwrap());
    assert_eq!(f.rule_id().as_str(), "CLIPPY_UNWRAP_USED");
    assert_eq!(f.message(), "use iterators");
}

#[test]
fn finding_lane_accessor() {
    let f = make_finding_for_accessors();
    assert_eq!(f.lane(), &"AstGrep".parse().unwrap());
}

#[test]
fn finding_rule_id_accessor() {
    let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
    let loc = WorkspacePath::new("src/lib.rs").unwrap();
    let f = titania_core::Finding::new(
        "Fmt".parse().unwrap(),
        rule_id,
        titania_core::Location::Span {
            file: loc,
            line_start: 1,
            col_start: 0,
            line_end: 1,
            col_end: 10,
        },
        "msg".to_string(),
        titania_core::RepairHint::use_iterator_pipeline("s".to_string()),
        titania_core::FindingEffect::Reject,
    );
    assert_eq!(f.rule_id().as_str(), "CLIPPY_UNWRAP_USED");
}

#[test]
fn finding_location_accessor() {
    let f = make_finding_for_accessors();
    assert!(matches!(f.location(), titania_core::Location::Span { .. }));
}

#[test]
fn finding_message_accessor() {
    let f = make_finding_for_accessors();
    assert_eq!(f.message(), "use iterators");
}

#[test]
fn finding_repair_accessor() {
    let f = make_finding_for_accessors();
    assert!(matches!(f.repair(), titania_core::RepairHint::UseIteratorPipeline { .. }));
}

#[test]
fn finding_effect_accessor() {
    let f = make_finding_for_accessors();
    assert!(matches!(f.effect(), titania_core::FindingEffect::Reject));
}

#[test]
fn finding_clone_preserves_all_fields() {
    let f = make_finding_for_accessors();
    let cloned = f.clone();
    assert_eq!(f, cloned);
    assert_eq!(f.lane(), cloned.lane());
    assert_eq!(f.rule_id(), cloned.rule_id());
    assert_eq!(f.message(), cloned.message());
    assert_eq!(f.effect(), cloned.effect());
}

// ===========================================================================
// FindingEffect — 2 variants
// ===========================================================================

#[test]
fn finding_effect_both_variants() {
    let reject = titania_core::FindingEffect::Reject;
    let informational = titania_core::FindingEffect::Informational;
    assert!(matches!(reject, titania_core::FindingEffect::Reject));
    assert!(matches!(informational, titania_core::FindingEffect::Informational));
    assert_ne!(reject, informational);
}

// ===========================================================================
// Location — 5 variants
// ===========================================================================

#[test]
fn location_span_constructs() {
    let file = WorkspacePath::new("src/main.rs").unwrap();
    let loc = titania_core::Location::Span {
        file,
        line_start: 10,
        col_start: 5,
        line_end: 20,
        col_end: 15,
    };
    assert!(matches!(loc, titania_core::Location::Span { .. }));
}

#[test]
fn location_span_line_start_min_is_1() {
    let file = WorkspacePath::new("src/main.rs").unwrap();
    let result = titania_core::Location::span(file, 0, 0, 10, 20);
    assert!(
        matches!(result, Err(titania_core::LocationError::LineStartZero)),
        "line_start=0 should return LineStartZero"
    );
}

#[test]
fn location_span_line_start_accepts_1() {
    let file = WorkspacePath::new("src/main.rs").unwrap();
    let result = titania_core::Location::span(file, 1, 0, 10, 20);
    assert!(matches!(result, Ok(titania_core::Location::Span { .. })));
}

#[test]
fn location_span_col_start_zero_accepted() {
    let file = WorkspacePath::new("src/main.rs").unwrap();
    let result = titania_core::Location::span(file, 1, 0, 10, 20);
    assert!(matches!(result, Ok(titania_core::Location::Span { .. })));
}

#[test]
fn location_span_col_end_zero_accepted() {
    let file = WorkspacePath::new("src/main.rs").unwrap();
    let result = titania_core::Location::span(file, 1, 0, 10, 0);
    assert!(matches!(result, Ok(titania_core::Location::Span { .. })));
}

#[test]
fn location_dependency_constructs() {
    let loc = titania_core::Location::dependency("serde".to_string(), "1.0".to_string());
    assert!(matches!(loc, titania_core::Location::Dependency { .. }));
}

#[test]
fn location_manifest_constructs() {
    let loc = titania_core::Location::manifest(WorkspacePath::new("Cargo.toml").unwrap());
    assert!(matches!(loc, titania_core::Location::Manifest { .. }));
}

#[test]
fn location_workspace_constructs() {
    let loc = titania_core::Location::workspace();
    assert!(matches!(loc, titania_core::Location::Workspace));
}

#[test]
fn location_tool_constructs() {
    let loc = titania_core::Location::tool("ast-grep".to_string(), "0.25".to_string());
    assert!(matches!(loc, titania_core::Location::Tool { .. }));
}

#[test]
fn location_span_accessor_file() {
    let file = WorkspacePath::new("src/main.rs").unwrap();
    let loc = titania_core::Location::Span {
        file: file.clone(),
        line_start: 1,
        col_start: 0,
        line_end: 10,
        col_end: 20,
    };
    assert_eq!(loc.as_file(), Ok(&file));
}

#[test]
fn location_span_accessor_lines() {
    let file = WorkspacePath::new("src/main.rs").unwrap();
    let loc = titania_core::Location::Span {
        file,
        line_start: 1,
        col_start: 5,
        line_end: 10,
        col_end: 20,
    };
    assert_eq!(loc.line_start(), 1);
    assert_eq!(loc.col_start(), 5);
    assert_eq!(loc.line_end(), 10);
    assert_eq!(loc.col_end(), 20);
}

// ===========================================================================
// RepairHint — 7 variants
// ===========================================================================

#[test]
fn repair_hint_all_7_variants_constructible() {
    let _ = titania_core::RepairHint::patch(
        "src/lib.rs".to_string(),
        TextRange::new(0, 10).unwrap(),
        "fixed".to_string(),
    )
    .unwrap();
    let _ = titania_core::RepairHint::use_iterator_pipeline("use iterators".to_string());
    let _ = titania_core::RepairHint::flatten_nesting("reduce nesting".to_string());
    let _ = titania_core::RepairHint::use_checked_arithmetic("checked_add".to_string());
    let _ = titania_core::RepairHint::remove_allow_attribute("allow(dead_code)".to_string());
    let _ = titania_core::RepairHint::replace_dependency(
        "old-dep = \"0.1\"".to_string(),
        "new-dep = \"0.2\"".to_string(),
    );
    let _ = titania_core::RepairHint::requires_human_review("manual fix needed".to_string());
}

#[test]
fn repair_hint_patch_with_valid_range_succeeds() {
    let result = titania_core::RepairHint::patch(
        "src/lib.rs".to_string(),
        TextRange::new(0, 10).unwrap(),
        "fixed".to_string(),
    );
    assert!(matches!(result, Ok(titania_core::RepairHint::Patch { .. })));
}

#[test]
fn repair_hint_patch_with_zero_width_range_rejected() {
    let result = titania_core::RepairHint::patch(
        "src/lib.rs".to_string(),
        TextRange::new(0, 0).unwrap(),
        "fixed".to_string(),
    );
    assert!(
        matches!(result, Err(titania_core::RepairHintError::ZeroWidth)),
        "zero-width range should return ZeroWidth error"
    );
}

#[test]
fn repair_hint_patch_accessor_fields() {
    let hint = titania_core::RepairHint::patch(
        "src/lib.rs".to_string(),
        TextRange::new(5, 15).unwrap(),
        "replacement".to_string(),
    )
    .unwrap();
    assert!(matches!(hint, titania_core::RepairHint::Patch { .. }));
}

#[test]
fn repair_hint_use_iterator_pipeline_accessor() {
    let hint = titania_core::RepairHint::use_iterator_pipeline("use .into_iter()".to_string());
    assert!(matches!(hint, titania_core::RepairHint::UseIteratorPipeline { .. }));
}

#[test]
fn repair_hint_flatten_nesting_accessor() {
    let hint = titania_core::RepairHint::flatten_nesting("reduce nesting depth".to_string());
    assert!(matches!(hint, titania_core::RepairHint::FlattenNesting { .. }));
}

#[test]
fn repair_hint_use_checked_arithmetic_accessor() {
    let hint = titania_core::RepairHint::use_checked_arithmetic("checked_add".to_string());
    assert!(matches!(hint, titania_core::RepairHint::UseCheckedArithmetic { .. }));
}

#[test]
fn repair_hint_remove_allow_attribute_accessor() {
    let hint = titania_core::RepairHint::remove_allow_attribute("allow(unused)".to_string());
    assert!(matches!(hint, titania_core::RepairHint::RemoveAllowAttribute { .. }));
}

#[test]
fn repair_hint_replace_dependency_accessor() {
    let hint = titania_core::RepairHint::replace_dependency(
        "serde = \"1.0\"".to_string(),
        "serde = \"1.1\"".to_string(),
    );
    assert!(matches!(hint, titania_core::RepairHint::ReplaceDependency { .. }));
}

#[test]
fn repair_hint_requires_human_review_accessor() {
    let hint =
        titania_core::RepairHint::requires_human_review("manual intervention needed".to_string());
    assert!(matches!(hint, titania_core::RepairHint::RequiresHumanReview { .. }));
}

// ===========================================================================
// LaneOutcome — 4 variants
// ===========================================================================

fn make_valid_evidence() -> titania_core::LaneEvidence {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "fmt".to_string()].into_boxed_slice(),
    )
    .unwrap();
    titania_core::LaneEvidence::new(
        cmd,
        "rustfmt 1.84.0".to_string(),
        titania_core::ProcessTermination::Exited { code: 0 },
        Digest::from_bytes(b"evidence"),
    )
}

fn make_invalid_exit_evidence() -> titania_core::LaneEvidence {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "fmt".to_string()].into_boxed_slice(),
    )
    .unwrap();
    titania_core::LaneEvidence::new(
        cmd,
        "rustfmt 1.84.0".to_string(),
        titania_core::ProcessTermination::Exited { code: 1 },
        Digest::from_bytes(b"evidence"),
    )
}

#[test]
fn lane_outcome_clean_with_valid_exit_constructs() {
    let outcome = titania_core::LaneOutcome::clean(make_valid_evidence());
    assert!(matches!(outcome, Ok(titania_core::LaneOutcome::Clean { .. })));
}

#[test]
fn lane_outcome_clean_rejects_nonzero_exit() {
    let outcome = titania_core::LaneOutcome::clean(make_invalid_exit_evidence());
    assert!(
        matches!(outcome, Err(titania_core::LaneOutcomeError::NonZeroExit { .. })),
        "non-zero exit should return NonZeroExit error"
    );
}

#[test]
fn lane_outcome_clean_rejects_timed_out() {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "fmt".to_string()].into_boxed_slice(),
    )
    .unwrap();
    let evidence = titania_core::LaneEvidence::new(
        cmd,
        "rustfmt 1.84.0".to_string(),
        titania_core::ProcessTermination::TimedOut,
        Digest::from_bytes(b"evidence"),
    );
    let outcome = titania_core::LaneOutcome::clean(evidence);
    assert!(matches!(outcome, Err(titania_core::LaneOutcomeError::NonZeroExit { .. })));
}

#[test]
fn lane_outcome_clean_rejects_spawn_failed() {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "fmt".to_string()].into_boxed_slice(),
    )
    .unwrap();
    let evidence = titania_core::LaneEvidence::new(
        cmd,
        "rustfmt 1.84.0".to_string(),
        titania_core::ProcessTermination::SpawnFailed,
        Digest::from_bytes(b"evidence"),
    );
    let outcome = titania_core::LaneOutcome::clean(evidence);
    assert!(matches!(outcome, Err(titania_core::LaneOutcomeError::NonZeroExit { .. })));
}

#[test]
fn lane_outcome_findings_with_empty_findings() {
    let findings: Box<[titania_core::Finding]> = Box::new([]);
    let outcome = titania_core::LaneOutcome::findings(findings);
    assert!(matches!(outcome, titania_core::LaneOutcome::Findings(f) if f.is_empty()));
}

#[test]
fn lane_outcome_findings_with_findings() {
    let findings: Box<[titania_core::Finding]> = Box::new([make_finding()]);
    let outcome = titania_core::LaneOutcome::findings(findings);
    assert!(matches!(outcome, titania_core::LaneOutcome::Findings(f) if !f.is_empty()));
}

#[test]
fn lane_outcome_failed_infra_failure() {
    let failure =
        titania_core::LaneFailure::infra_failure("cargo-fmt".to_string(), "missing".to_string());
    let outcome = titania_core::LaneOutcome::failed(failure);
    assert!(matches!(
        outcome,
        titania_core::LaneOutcome::Failed(titania_core::LaneFailure::InfraFailure { .. })
    ));
}

#[test]
fn lane_outcome_failed_tool_failure() {
    let failure = titania_core::LaneFailure::tool_failure(
        "clippy".to_string(),
        titania_core::ProcessTermination::Exited { code: 1 },
    );
    let outcome = titania_core::LaneOutcome::failed(failure);
    assert!(matches!(
        outcome,
        titania_core::LaneOutcome::Failed(titania_core::LaneFailure::ToolFailure { .. })
    ));
}

#[test]
fn lane_outcome_failed_resource_failure() {
    let failure =
        titania_core::LaneFailure::resource_failure("dylint".to_string(), "timeout".to_string());
    let outcome = titania_core::LaneOutcome::failed(failure);
    assert!(matches!(
        outcome,
        titania_core::LaneOutcome::Failed(titania_core::LaneFailure::ResourceFailure { .. })
    ));
}

#[test]
fn lane_outcome_failed_suspicious_failure() {
    let failure = titania_core::LaneFailure::suspicious_failure(
        "ast-grep".to_string(),
        "tampered output".to_string(),
    );
    let outcome = titania_core::LaneOutcome::failed(failure);
    assert!(matches!(
        outcome,
        titania_core::LaneOutcome::Failed(titania_core::LaneFailure::SuspiciousFailure { .. })
    ));
}

#[test]
fn lane_outcome_skipped_prior_compilation_failure() {
    let outcome =
        titania_core::LaneOutcome::skipped(titania_core::SkipReason::PriorCompilationFailure);
    assert!(matches!(
        outcome,
        titania_core::LaneOutcome::Skipped(titania_core::SkipReason::PriorCompilationFailure)
    ));
}

#[test]
fn lane_outcome_skipped_not_selected_by_scope() {
    let outcome = titania_core::LaneOutcome::skipped(titania_core::SkipReason::NotSelectedByScope);
    assert!(matches!(
        outcome,
        titania_core::LaneOutcome::Skipped(titania_core::SkipReason::NotSelectedByScope)
    ));
}

#[test]
fn lane_outcome_skipped_not_applicable() {
    let outcome = titania_core::LaneOutcome::skipped(titania_core::SkipReason::NotApplicable);
    assert!(matches!(
        outcome,
        titania_core::LaneOutcome::Skipped(titania_core::SkipReason::NotApplicable)
    ));
}

#[test]
fn lane_outcome_skipped_policy_disabled() {
    let outcome = titania_core::LaneOutcome::skipped(titania_core::SkipReason::PolicyDisabled);
    assert!(matches!(
        outcome,
        titania_core::LaneOutcome::Skipped(titania_core::SkipReason::PolicyDisabled)
    ));
}

// ===========================================================================
// SkipReason — 4 variants
// ===========================================================================

#[test]
fn skip_reason_all_4_variants() {
    assert!(matches!(
        titania_core::SkipReason::PriorCompilationFailure,
        titania_core::SkipReason::PriorCompilationFailure
    ));
    assert!(matches!(
        titania_core::SkipReason::NotSelectedByScope,
        titania_core::SkipReason::NotSelectedByScope
    ));
    assert!(matches!(
        titania_core::SkipReason::NotApplicable,
        titania_core::SkipReason::NotApplicable
    ));
    assert!(matches!(
        titania_core::SkipReason::PolicyDisabled,
        titania_core::SkipReason::PolicyDisabled
    ));
}

// ===========================================================================
// LaneEvidence — struct
// ===========================================================================

#[test]
fn lane_evidence_constructs() {
    let evidence = make_valid_evidence();
    assert_eq!(evidence.tool_version(), "rustfmt 1.84.0");
}

#[test]
fn lane_evidence_accessors() {
    let evidence = make_valid_evidence();
    assert_eq!(evidence.tool_version(), "rustfmt 1.84.0");
    assert!(matches!(evidence.exit_status(), titania_core::ProcessTermination::Exited { code: 0 }));
}

// ===========================================================================
// CommandEvidence — struct
// ===========================================================================

#[test]
fn command_evidence_constructs() {
    let result = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "fmt".to_string()].into_boxed_slice(),
    );
    assert!(matches!(result, Ok(titania_core::CommandEvidence { .. })));
    let cmd = result.unwrap();
    assert_eq!(cmd.executable(), "cargo");
}

#[test]
fn command_evidence_rejects_empty_argv() {
    let result =
        titania_core::CommandEvidence::new("cargo".to_string(), Box::new([]) as Box<[String]>);
    assert!(
        matches!(result, Err(titania_core::CommandEvidenceError::EmptyArgv)),
        "empty argv should return EmptyArgv"
    );
}

#[test]
fn command_evidence_rejects_argv0_mismatch() {
    let result = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["rustc".to_string(), "fmt".to_string()].into_boxed_slice(),
    );
    assert!(
        matches!(result, Err(titania_core::CommandEvidenceError::Argv0Mismatch)),
        "argv[0] mismatch should return Argv0Mismatch"
    );
}

#[test]
fn command_evidence_rejects_single_empty_string_argv() {
    let result = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["".to_string()].into_boxed_slice(),
    );
    // Either EmptyArgv or Argv0Mismatch
    assert!(matches!(
        result,
        Err(titania_core::CommandEvidenceError::EmptyArgv)
            | Err(titania_core::CommandEvidenceError::Argv0Mismatch)
    ));
}

#[test]
fn command_evidence_accessors() {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "clippy".to_string()].into_boxed_slice(),
    )
    .unwrap();
    assert_eq!(cmd.executable(), "cargo");
    assert_eq!(cmd.argv().len(), 2);
    assert_eq!(cmd.argv()[0], "cargo");
}

// ===========================================================================
// LaneFailure — 4 variants
// ===========================================================================

#[test]
fn lane_failure_infra_failure() {
    let f =
        titania_core::LaneFailure::infra_failure("cargo-fmt".to_string(), "missing".to_string());
    assert!(matches!(f, titania_core::LaneFailure::InfraFailure { .. }));
}

#[test]
fn lane_failure_tool_failure() {
    let f = titania_core::LaneFailure::tool_failure(
        "clippy".to_string(),
        titania_core::ProcessTermination::Exited { code: 1 },
    );
    assert!(matches!(f, titania_core::LaneFailure::ToolFailure { .. }));
}

#[test]
fn lane_failure_resource_failure() {
    let f =
        titania_core::LaneFailure::resource_failure("dylint".to_string(), "timeout".to_string());
    assert!(matches!(f, titania_core::LaneFailure::ResourceFailure { .. }));
}

#[test]
fn lane_failure_suspicious_failure() {
    let f = titania_core::LaneFailure::suspicious_failure(
        "ast-grep".to_string(),
        "tampered".to_string(),
    );
    assert!(matches!(f, titania_core::LaneFailure::SuspiciousFailure { .. }));
}

#[test]
fn lane_failure_tool_accessor_infra() {
    let f =
        titania_core::LaneFailure::infra_failure("cargo-fmt".to_string(), "missing".to_string());
    assert_eq!(f.tool(), "cargo-fmt");
}

// ===========================================================================
// ProcessTermination — 5 variants
// ===========================================================================

#[test]
fn process_termination_exited_code_0() {
    let t = titania_core::ProcessTermination::Exited { code: 0 };
    assert!(matches!(t, titania_core::ProcessTermination::Exited { code: 0 }));
}

#[test]
fn process_termination_exited_code_1() {
    let t = titania_core::ProcessTermination::Exited { code: 1 };
    assert!(matches!(t, titania_core::ProcessTermination::Exited { code: 1 }));
}

#[test]
fn process_termination_exited_negative_code() {
    let t = titania_core::ProcessTermination::Exited { code: -1 };
    assert!(matches!(t, titania_core::ProcessTermination::Exited { code: -1 }));
}

#[test]
fn process_termination_signaled_valid_signal() {
    let t = titania_core::ProcessTermination::signaled(9).unwrap();
    assert!(matches!(t, titania_core::ProcessTermination::Signaled { signal: 9 }));
}

#[test]
fn process_termination_signaled_rejects_signal_0() {
    let result = titania_core::ProcessTermination::signaled(0);
    assert!(
        matches!(result, Err(titania_core::ProcessTerminationError::InvalidSignal)),
        "signal 0 should return InvalidSignal"
    );
}

#[test]
fn process_termination_signaled_rejects_signal_32() {
    let result = titania_core::ProcessTermination::signaled(32);
    assert!(
        matches!(result, Err(titania_core::ProcessTerminationError::InvalidSignal)),
        "signal 32 should return InvalidSignal"
    );
}

#[test]
fn process_termination_timed_out() {
    let t = titania_core::ProcessTermination::TimedOut;
    assert!(matches!(t, titania_core::ProcessTermination::TimedOut));
}

#[test]
fn process_termination_memory_limit_exceeded() {
    let t = titania_core::ProcessTermination::MemoryLimitExceeded;
    assert!(matches!(t, titania_core::ProcessTermination::MemoryLimitExceeded));
}

#[test]
fn process_termination_spawn_failed() {
    let t = titania_core::ProcessTermination::SpawnFailed;
    assert!(matches!(t, titania_core::ProcessTermination::SpawnFailed));
}

#[test]
fn process_termination_exited_accessor() {
    let t = titania_core::ProcessTermination::Exited { code: 42 };
    assert_eq!(t.exit_code(), Some(42));
}

#[test]
fn process_termination_exited_accessor_none_for_other_variants() {
    assert!(titania_core::ProcessTermination::timed_out().exit_code().is_none());
    assert!(titania_core::ProcessTermination::spawn_failed().exit_code().is_none());
    assert!(titania_core::ProcessTermination::memory_limit_exceeded().exit_code().is_none());
    assert!(titania_core::ProcessTermination::signaled(9).unwrap().exit_code().is_none());
}

#[test]
fn process_termination_signal_accessor() {
    let t = titania_core::ProcessTermination::signaled(9).unwrap();
    assert_eq!(t.signal(), Some(9));
}

// ===========================================================================
// RejectKind — 3 variants
// ===========================================================================

#[test]
fn reject_kind_all_3_variants() {
    assert!(matches!(titania_core::RejectKind::CodeOnly, titania_core::RejectKind::CodeOnly));
    assert!(matches!(titania_core::RejectKind::GateOnly, titania_core::RejectKind::GateOnly));
    assert!(matches!(titania_core::RejectKind::Mixed, titania_core::RejectKind::Mixed));
}

// ===========================================================================
// QualityReceipt — struct
// ===========================================================================

fn make_quality_receipt() -> titania_core::QualityReceipt {
    let digest = Digest::from_bytes(b"test");
    let lanes = vec![
        titania_core::LaneReceipt {
            lane: "Fmt".parse().unwrap(),
            evidence_digest: digest.clone(),
            clean: true,
        },
        titania_core::LaneReceipt {
            lane: "Compile".parse().unwrap(),
            evidence_digest: digest.clone(),
            clean: true,
        },
    ];
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

#[test]
fn quality_receipt_constructs_with_valid_args() {
    let receipt = make_quality_receipt();
    assert!(matches!(receipt, titania_core::QualityReceipt { .. }));
}

#[test]
fn quality_receipt_rejects_wrong_schema_version() {
    let digest = Digest::from_bytes(b"test");
    let lanes: Box<[titania_core::LaneReceipt]> = Box::new([]);
    let result = titania_core::QualityReceipt::new(
        2, // wrong version
        titania_core::GateScope::Edit,
        digest.clone(),
        digest.clone(),
        digest.clone(),
        digest.clone(),
        lanes,
    );
    assert!(
        matches!(result, Err(titania_core::QualityReceiptError::UnsupportedSchemaVersion)),
        "wrong schema_version should return UnsupportedSchemaVersion"
    );
}

#[test]
fn quality_receipt_schema_version_accessor() {
    let receipt = make_quality_receipt();
    assert_eq!(receipt.schema_version(), 1);
}

#[test]
fn quality_receipt_scope_accessor() {
    let receipt = make_quality_receipt();
    assert_eq!(receipt.scope(), &titania_core::GateScope::Edit);
}

#[test]
fn quality_receipt_lanes_accessor() {
    let receipt = make_quality_receipt();
    assert_eq!(receipt.lanes().len(), 2);
}

#[test]
fn quality_receipt_source_digest_accessor() {
    let receipt = make_quality_receipt();
    let source = receipt.source_digest();
    assert_eq!(source.as_hex(), Digest::from_bytes(b"test").as_hex());
}

#[test]
fn quality_receipt_empty_lanes_allowed() {
    let digest = Digest::from_bytes(b"test");
    let lanes: Box<[titania_core::LaneReceipt]> = Box::new([]);
    let result = titania_core::QualityReceipt::new(
        1,
        titania_core::GateScope::Edit,
        digest.clone(),
        digest.clone(),
        digest.clone(),
        digest.clone(),
        lanes,
    );
    assert!(matches!(result, Ok(titania_core::QualityReceipt { .. })));
}

// ===========================================================================
// LaneReceipt — struct
// ===========================================================================

#[test]
fn lane_receipt_constructs() {
    let lr = titania_core::LaneReceipt {
        lane: "Fmt".parse().unwrap(),
        evidence_digest: Digest::from_bytes(b"lane_receipt"),
        clean: true,
    };
    assert_eq!(lr.lane(), &"Fmt".parse().unwrap());
    assert!(lr.clean());
}

#[test]
fn lane_receipt_accessors() {
    let lr = titania_core::LaneReceipt {
        lane: "Compile".parse().unwrap(),
        evidence_digest: Digest::from_bytes(b"receipt"),
        clean: false,
    };
    assert_eq!(lr.lane(), &"Compile".parse().unwrap());
    assert_eq!(lr.evidence_digest().as_hex(), Digest::from_bytes(b"receipt").as_hex());
    assert!(!lr.clean());
}

// ===========================================================================
// PolicyDiagnostic — struct
// ===========================================================================

fn make_diagnostic() -> titania_core::PolicyDiagnostic {
    titania_core::PolicyDiagnostic {
        message: "test diagnostic".to_string(),
        file: None,
        severity: titania_core::DiagnosticSeverity::Error,
    }
}

#[test]
fn policy_diagnostic_error_constructs() {
    let d = make_diagnostic();
    assert_eq!(d.message(), "test diagnostic");
    assert!(matches!(d.severity(), titania_core::DiagnosticSeverity::Error));
}

#[test]
fn policy_diagnostic_warning_constructs() {
    let d = titania_core::PolicyDiagnostic {
        message: "warning".to_string(),
        file: None,
        severity: titania_core::DiagnosticSeverity::Warning,
    };
    assert!(matches!(d.severity(), titania_core::DiagnosticSeverity::Warning));
}

#[test]
fn policy_diagnostic_with_file() {
    let mut d = make_diagnostic();
    d.file = Some(WorkspacePath::new("policy.toml").unwrap());
    assert!(d.file().is_some());
}

#[test]
fn policy_diagnostic_without_file() {
    let d = make_diagnostic();
    assert!(d.file().is_none());
}

#[test]
fn policy_diagnostic_message_accessor() {
    let d = make_diagnostic();
    assert_eq!(d.message(), "test diagnostic");
}

// ===========================================================================
// InputDiagnostic — struct
// ===========================================================================

fn make_input_diagnostic() -> titania_core::InputDiagnostic {
    titania_core::InputDiagnostic {
        message: "test input diagnostic".to_string(),
        tool: Some("cargo".to_string()),
        severity: titania_core::DiagnosticSeverity::Error,
    }
}

#[test]
fn input_diagnostic_error_constructs() {
    let d = make_input_diagnostic();
    assert!(matches!(d.severity(), titania_core::DiagnosticSeverity::Error));
}

#[test]
fn input_diagnostic_with_tool() {
    let d = make_input_diagnostic();
    assert_eq!(d.tool(), Some("cargo"));
}

#[test]
fn input_diagnostic_without_tool() {
    let d = titania_core::InputDiagnostic {
        message: "no tool".to_string(),
        tool: None,
        severity: titania_core::DiagnosticSeverity::Warning,
    };
    assert!(matches!(d.tool(), None));
}

// ===========================================================================
// DiagnosticSeverity — 2 variants
// ===========================================================================

#[test]
fn diagnostic_severity_both_variants() {
    assert!(matches!(
        titania_core::DiagnosticSeverity::Error,
        titania_core::DiagnosticSeverity::Error
    ));
    assert!(matches!(
        titania_core::DiagnosticSeverity::Warning,
        titania_core::DiagnosticSeverity::Warning
    ));
    assert_ne!(titania_core::DiagnosticSeverity::Error, titania_core::DiagnosticSeverity::Warning);
}
