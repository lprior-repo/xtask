//! Serde JSON round-trip tests for the v1 domain model (19 types).
//!
//! Each test constructs a value through the smart constructor, serializes to
//! JSON, deserializes back, and asserts structural equality.

#![allow(clippy::needless_borrow)]
#![allow(clippy::useless_vec)]
#![allow(clippy::as_conversions)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::type_complexity)]
#![allow(clippy::map_identity)]

use titania_core::{Digest, RuleId, TextRange, WorkspacePath};

// ===========================================================================
// Lane — 10 variants
// ===========================================================================

#[test]
fn lane_serde_all_10_variants_round_trip() {
    let lanes: Vec<&str> = vec![
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
    for label in lanes {
        let lane = label.parse::<titania_core::Lane>().expect(label);
        let json = serde_json::to_string(&lane).unwrap();
        let back: titania_core::Lane = serde_json::from_str(&json).unwrap();
        assert_eq!(lane, back, "round-trip failed for {label}");
    }
}

#[test]
fn lane_json_is_string_form() {
    let v: serde_json::Value = serde_json::to_value(titania_core::Lane::Dylint).unwrap();
    assert!(v.is_string(), "Lane must serialize to JSON string, got {v}");
    assert_eq!(v.as_str().unwrap(), "Dylint");
}

#[test]
fn lane_serde_deterministic() {
    let a = "Fmt".parse::<titania_core::Lane>().unwrap();
    let b = "Fmt".parse::<titania_core::Lane>().unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap(),
        "identical lanes must serialize identically"
    );
}

// ===========================================================================
// GateScope — 3 variants
// ===========================================================================

#[test]
fn gate_scope_serde_all_3_variants_round_trip() {
    let scopes: Vec<(&str, titania_core::GateScope)> = vec![
        ("edit", titania_core::GateScope::Edit),
        ("prepush", titania_core::GateScope::Prepush),
        ("release", titania_core::GateScope::Release),
    ];
    for (label, scope) in scopes {
        let json = serde_json::to_string(&scope).unwrap();
        let back: titania_core::GateScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, back, "round-trip failed for {label}");
        assert_eq!(json, format!("\"{label}\""), "{label} must serialize to snake_case");
    }
}

#[test]
fn gate_scope_json_is_string_form() {
    let v: serde_json::Value = serde_json::to_value(titania_core::GateScope::Release).unwrap();
    assert!(v.is_string());
    assert_eq!(v.as_str().unwrap(), "release");
}

// ===========================================================================
// Report — 4 variants
// ===========================================================================

fn make_finding() -> titania_core::Finding {
    let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
    let location = WorkspacePath::new("src/lib.rs").unwrap();
    titania_core::Finding::new(
        "Fmt".parse().unwrap(),
        rule_id,
        titania_core::Location::Span {
            file: location,
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
        titania_core::ProcessTermination::timed_out(),
    )
}

fn make_diagnostic() -> titania_core::PolicyDiagnostic {
    titania_core::PolicyDiagnostic {
        message: "test diagnostic".to_string(),
        file: None,
        severity: titania_core::DiagnosticSeverity::Error,
    }
}

fn make_input_diagnostic() -> titania_core::InputDiagnostic {
    titania_core::InputDiagnostic {
        message: "test input diagnostic".to_string(),
        tool: Some("cargo".to_string()),
        severity: titania_core::DiagnosticSeverity::Warning,
    }
}

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
fn report_pass_serde_round_trip() {
    let receipt = make_quality_receipt();
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([
        titania_core::LaneOutcome::clean(make_lane_evidence()).unwrap(),
        titania_core::LaneOutcome::clean(make_lane_evidence()).unwrap(),
    ]);
    let report = titania_core::Report::pass(receipt, per_lane).unwrap();
    let json = serde_json::to_string(&report).unwrap();
    let back: titania_core::Report = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", report),
        format!("{:?}", back),
        "Pass round-trip must preserve structure"
    );
}

#[test]
fn report_reject_serde_round_trip() {
    let findings: Box<[titania_core::Finding]> = Box::new([make_finding()]);
    let failures: Box<[titania_core::LaneFailure]> = Box::new([make_lane_failure()]);
    let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
    let report = titania_core::Report::reject(findings, failures, per_lane).unwrap();
    let json = serde_json::to_string(&report).unwrap();
    let back: titania_core::Report = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", report),
        format!("{:?}", back),
        "Reject round-trip must preserve structure"
    );
}

#[test]
fn report_policy_error_serde_round_trip() {
    let diagnostics: Box<[titania_core::PolicyDiagnostic]> = Box::new([make_diagnostic()]);
    let report = titania_core::Report::policy_error(diagnostics);
    let json = serde_json::to_string(&report).unwrap();
    let back: titania_core::Report = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", report),
        format!("{:?}", back),
        "PolicyError round-trip must preserve structure"
    );
}

#[test]
fn report_input_error_serde_round_trip() {
    let diagnostics: Box<[titania_core::InputDiagnostic]> = Box::new([make_input_diagnostic()]);
    let report = titania_core::Report::input_error(diagnostics);
    let json = serde_json::to_string(&report).unwrap();
    let back: titania_core::Report = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", report),
        format!("{:?}", back),
        "InputError round-trip must preserve structure"
    );
}

// ===========================================================================
// Finding — 6 fields
// ===========================================================================

#[test]
fn finding_serde_round_trip_all_fields() {
    let finding = make_finding();
    let json = serde_json::to_string(&finding).unwrap();
    let back: titania_core::Finding = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", finding),
        format!("{:?}", back),
        "Finding must round-trip all 6 fields"
    );
}

#[test]
fn finding_json_structure() {
    let finding = make_finding();
    let v: serde_json::Value = serde_json::to_value(&finding).unwrap();
    let obj = v.as_object().expect("Finding must serialize to object");
    let keys: Vec<&str> = obj.keys().collect();
    assert!(keys.contains(&"lane"), "must have 'lane' key");
    assert!(keys.contains(&"rule_id"), "must have 'rule_id' key");
    assert!(keys.contains(&"location"), "must have 'location' key");
    assert!(keys.contains(&"message"), "must have 'message' key");
    assert!(keys.contains(&"repair"), "must have 'repair' key");
    assert!(keys.contains(&"effect"), "must have 'effect' key");
}

#[test]
fn finding_informational_serde_round_trip() {
    let mut finding = make_finding();
    // Rebuild with Informational effect
    let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
    let location = WorkspacePath::new("src/lib.rs").unwrap();
    finding = titania_core::Finding::new(
        "Fmt".parse().unwrap(),
        rule_id,
        titania_core::Location::Span {
            file: location,
            line_start: 1,
            col_start: 0,
            line_end: 1,
            col_end: 10,
        },
        "info".to_string(),
        titania_core::RepairHint::use_iterator_pipeline("suggestion".to_string()),
        titania_core::FindingEffect::Informational,
    );
    let json = serde_json::to_string(&finding).unwrap();
    let back: titania_core::Finding = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", finding),
        format!("{:?}", back),
        "Informational Finding must round-trip"
    );
}

// ===========================================================================
// FindingEffect — 2 variants
// ===========================================================================

#[test]
fn finding_effect_serde_round_trip() {
    let effects: Vec<(&str, titania_core::FindingEffect)> = vec![
        ("reject", titania_core::FindingEffect::Reject),
        ("informational", titania_core::FindingEffect::Informational),
    ];
    for (label, effect) in effects {
        let json = serde_json::to_string(&effect).unwrap();
        let back: titania_core::FindingEffect = serde_json::from_str(&json).unwrap();
        assert_eq!(effect, back, "{label} round-trip failed");
        assert_eq!(json, format!("\"{label}\""));
    }
}

// ===========================================================================
// Location — 5 variants
// ===========================================================================

#[test]
fn location_span_serde_round_trip() {
    let loc = titania_core::Location::Span {
        file: WorkspacePath::new("src/main.rs").unwrap(),
        line_start: 10,
        col_start: 5,
        line_end: 20,
        col_end: 15,
    };
    let json = serde_json::to_string(&loc).unwrap();
    let back: titania_core::Location = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", loc), format!("{:?}", back), "Span must round-trip all 5 fields");
}

#[test]
fn location_dependency_serde_round_trip() {
    let loc = titania_core::Location::Dependency {
        crate_name: "serde".to_string(),
        version: "1.0.0".to_string(),
    };
    let json = serde_json::to_string(&loc).unwrap();
    let back: titania_core::Location = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", loc), format!("{:?}", back), "Dependency must round-trip");
}

#[test]
fn location_manifest_serde_round_trip() {
    let loc = titania_core::Location::Manifest { file: WorkspacePath::new("Cargo.toml").unwrap() };
    let json = serde_json::to_string(&loc).unwrap();
    let back: titania_core::Location = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", loc), format!("{:?}", back), "Manifest must round-trip");
}

#[test]
fn location_workspace_serde_round_trip() {
    let loc = titania_core::Location::Workspace;
    let json = serde_json::to_string(&loc).unwrap();
    let back: titania_core::Location = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", loc), format!("{:?}", back), "Workspace must round-trip");
}

#[test]
fn location_tool_serde_round_trip() {
    let loc =
        titania_core::Location::Tool { name: "clippy".to_string(), version: "0.1.0".to_string() };
    let json = serde_json::to_string(&loc).unwrap();
    let back: titania_core::Location = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", loc), format!("{:?}", back), "Tool must round-trip");
}

#[test]
fn location_all_variants_json_structure() {
    // Span: object with "kind" + data fields
    let span = titania_core::Location::Span {
        file: WorkspacePath::new("src/lib.rs").unwrap(),
        line_start: 1,
        col_start: 0,
        line_end: 1,
        col_end: 10,
    };
    let span_val: serde_json::Value = serde_json::to_value(&span).unwrap();
    assert!(span_val.is_object());

    // Dependency: object with "kind" + crate_name + version
    let dep = titania_core::Location::Dependency {
        crate_name: "serde".to_string(),
        version: "1.0".to_string(),
    };
    let dep_val: serde_json::Value = serde_json::to_value(&dep).unwrap();
    assert!(dep_val.is_object());
    let dep_obj = dep_val.as_object().unwrap();
    assert!(dep_obj.contains_key("crate_name"));
    assert!(dep_obj.contains_key("version"));

    // Workspace: unit variant
    let ws = titania_core::Location::Workspace;
    let ws_val: serde_json::Value = serde_json::to_value(&ws).unwrap();
    assert!(ws_val.is_object());

    // Manifest: object with "kind" + file
    let manifest =
        titania_core::Location::Manifest { file: WorkspacePath::new("Cargo.toml").unwrap() };
    let manifest_val: serde_json::Value = serde_json::to_value(&manifest).unwrap();
    assert!(manifest_val.is_object());

    // Tool: object with "kind" + name + version
    let tool =
        titania_core::Location::Tool { name: "clippy".to_string(), version: "0.1".to_string() };
    let tool_val: serde_json::Value = serde_json::to_value(&tool).unwrap();
    assert!(tool_val.is_object());
    let tool_obj = tool_val.as_object().unwrap();
    assert!(tool_obj.contains_key("name"));
    assert!(tool_obj.contains_key("version"));
}

// ===========================================================================
// RepairHint — 7 variants
// ===========================================================================

fn make_text_range() -> TextRange {
    TextRange::new(0, 10).unwrap()
}

#[test]
fn repair_hint_patch_serde_round_trip() {
    let hint = titania_core::RepairHint::patch(
        "src/lib.rs".to_string(),
        make_text_range(),
        "fixed".to_string(),
    )
    .unwrap();
    let json = serde_json::to_string(&hint).unwrap();
    let back: titania_core::RepairHint = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", hint), format!("{:?}", back), "Patch must round-trip");
}

#[test]
fn repair_hint_use_iterator_pipeline_serde_round_trip() {
    let hint = titania_core::RepairHint::use_iterator_pipeline("use .into_iter()".to_string());
    let json = serde_json::to_string(&hint).unwrap();
    let back: titania_core::RepairHint = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", hint), format!("{:?}", back), "UseIteratorPipeline must round-trip");
}

// ===========================================================================
// LaneOutcome — 4 variants (Clean, Findings, Failed, Skipped)
// ===========================================================================

#[test]
fn lane_outcome_clean_serde_round_trip() {
    let evidence = make_lane_evidence();
    let outcome = titania_core::LaneOutcome::clean(evidence).unwrap();
    let json = serde_json::to_string(&outcome).unwrap();
    let back: titania_core::LaneOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", outcome), format!("{:?}", back), "Clean must round-trip");
}

#[test]
fn lane_outcome_findings_serde_round_trip() {
    let findings: Box<[titania_core::Finding]> = Box::new([make_finding()]);
    let outcome = titania_core::LaneOutcome::findings(findings);
    let json = serde_json::to_string(&outcome).unwrap();
    let back: titania_core::LaneOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", outcome), format!("{:?}", back), "Findings must round-trip");
}

#[test]
fn lane_outcome_failed_infra_serde_round_trip() {
    let failure = titania_core::LaneFailure::infra_failure(
        "cargo-fmt".to_string(),
        "missing binary".to_string(),
    );
    let outcome = titania_core::LaneOutcome::failed(failure);
    let json = serde_json::to_string(&outcome).unwrap();
    let back: titania_core::LaneOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", outcome),
        format!("{:?}", back),
        "Failed(InfraFailure) must round-trip"
    );
}

#[test]
fn lane_outcome_failed_tool_serde_round_trip() {
    let failure = titania_core::LaneFailure::tool_failure(
        "clippy".to_string(),
        titania_core::ProcessTermination::exited(1),
    );
    let outcome = titania_core::LaneOutcome::failed(failure);
    let json = serde_json::to_string(&outcome).unwrap();
    let back: titania_core::LaneOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", outcome),
        format!("{:?}", back),
        "Failed(ToolFailure) must round-trip"
    );
}

#[test]
fn lane_outcome_skipped_serde_round_trip_all() {
    let reasons: Vec<&str> =
        vec!["PriorCompilationFailure", "NotSelectedByScope", "NotApplicable", "PolicyDisabled"];
    for reason_str in reasons {
        let reason = reason_str.parse::<titania_core::SkipReason>().expect(reason_str);
        let outcome = titania_core::LaneOutcome::skipped(reason);
        let json = serde_json::to_string(&outcome).unwrap();
        let back: titania_core::LaneOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(
            format!("{:?}", outcome),
            format!("{:?}", back),
            "Skipped({reason_str}) must round-trip"
        );
    }
}

// ===========================================================================
// SkipReason — 4 variants
// ===========================================================================

#[test]
fn skip_reason_all_variants_round_trip() {
    let reasons: Vec<(&str, titania_core::SkipReason)> = vec![
        ("PriorCompilationFailure", titania_core::SkipReason::PriorCompilationFailure),
        ("NotSelectedByScope", titania_core::SkipReason::NotSelectedByScope),
        ("NotApplicable", titania_core::SkipReason::NotApplicable),
        ("PolicyDisabled", titania_core::SkipReason::PolicyDisabled),
    ];
    for (label, reason) in reasons {
        let json = serde_json::to_string(&reason).unwrap();
        let back: titania_core::SkipReason = serde_json::from_str(&json).unwrap();
        assert_eq!(reason, back, "{label} round-trip failed");
    }
}

// ===========================================================================
// LaneEvidence — struct
// ===========================================================================

fn make_lane_evidence() -> titania_core::LaneEvidence {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "fmt", "--check".to_string()].into_boxed_slice(),
    )
    .unwrap();
    let digest = Digest::from_bytes(b"digest");
    titania_core::LaneEvidence::new(
        cmd,
        "rustfmt 1.84.0".to_string(),
        titania_core::ProcessTermination::exited(0),
        digest,
    )
}

#[test]
fn lane_evidence_serde_round_trip() {
    let evidence = make_lane_evidence();
    let json = serde_json::to_string(&evidence).unwrap();
    let back: titania_core::LaneEvidence = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", evidence), format!("{:?}", back), "LaneEvidence must round-trip");
}

// ===========================================================================
// CommandEvidence — struct
// ===========================================================================

#[test]
fn command_evidence_serde_round_trip() {
    let cmd = titania_core::CommandEvidence::new(
        "cargo".to_string(),
        vec!["cargo".to_string(), "clippy".to_string()].into_boxed_slice(),
    )
    .unwrap();
    let json = serde_json::to_string(&cmd).unwrap();
    let back: titania_core::CommandEvidence = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", cmd), format!("{:?}", back), "CommandEvidence must round-trip");
}

// ===========================================================================
// LaneFailure — 4 variants
// ===========================================================================

#[test]
fn lane_failure_all_4_variants_round_trip() {
    let infra =
        titania_core::LaneFailure::infra_failure("rustc".to_string(), "not found".to_string());
    let tool = titania_core::LaneFailure::tool_failure(
        "clippy".to_string(),
        titania_core::ProcessTermination::exited(1),
    );
    let resource =
        titania_core::LaneFailure::resource_failure("cargo".to_string(), "timeout".to_string());
    let suspicious =
        titania_core::LaneFailure::suspicious_failure("dylint".to_string(), "tampered".to_string());
    for (name, failure) in [
        ("InfraFailure", infra),
        ("ToolFailure", tool),
        ("ResourceFailure", resource),
        ("SuspiciousFailure", suspicious),
    ] {
        let json = serde_json::to_string(&failure).unwrap();
        let back: titania_core::LaneFailure = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{:?}", failure), format!("{:?}", back), "{name} must round-trip");
    }
}

// ===========================================================================
// ProcessTermination — 5 variants
// ===========================================================================

#[test]
fn process_termination_all_5_variants_round_trip() {
    let variants: Vec<titania_core::ProcessTermination> = vec![
        titania_core::ProcessTermination::exited(0),
        titania_core::ProcessTermination::exited(42),
        titania_core::ProcessTermination::exited(-1),
        titania_core::ProcessTermination::signaled(9).unwrap(),
        titania_core::ProcessTermination::timed_out(),
        titania_core::ProcessTermination::memory_limit_exceeded(),
        titania_core::ProcessTermination::spawn_failed(),
    ];
    for term in variants {
        let json = serde_json::to_string(&term).unwrap();
        let back: titania_core::ProcessTermination = serde_json::from_str(&json).unwrap();
        assert_eq!(
            format!("{:?}", term),
            format!("{:?}", back),
            "ProcessTermination must round-trip"
        );
    }
}

#[test]
fn process_termination_exited_serde_preserves_code() {
    let code = 42;
    let term = titania_core::ProcessTermination::exited(code);
    let json = serde_json::to_string(&term).unwrap();
    let back: titania_core::ProcessTermination = serde_json::from_str(&json).unwrap();
    assert_eq!(back.exit_code(), Some(code), "exited code must be preserved");
}

// ===========================================================================
// RejectKind — 3 variants
// ===========================================================================

#[test]
fn reject_kind_all_3_variants_round_trip() {
    let kinds: Vec<(&str, titania_core::RejectKind)> = vec![
        ("CodeOnly", titania_core::RejectKind::CodeOnly),
        ("GateOnly", titania_core::RejectKind::GateOnly),
        ("Mixed", titania_core::RejectKind::Mixed),
    ];
    for (label, kind) in kinds {
        let json = serde_json::to_string(&kind).unwrap();
        let back: titania_core::RejectKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, back, "{label} round-trip failed");
    }
}

// ===========================================================================
// QualityReceipt — struct
// ===========================================================================

#[test]
fn quality_receipt_serde_round_trip() {
    let receipt = make_quality_receipt();
    let json = serde_json::to_string(&receipt).unwrap();
    let back: titania_core::QualityReceipt = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", receipt), format!("{:?}", back), "QualityReceipt must round-trip");
}

#[test]
fn quality_receipt_schema_version_in_json() {
    let receipt = make_quality_receipt();
    let v: serde_json::Value = serde_json::to_value(&receipt).unwrap();
    let obj = v.as_object().expect("QualityReceipt must be object");
    assert!(obj.contains_key("schema_version"), "must have 'schema_version' key");
    assert_eq!(obj.get("schema_version").unwrap().as_u64().unwrap(), 1);
}

// ===========================================================================
// LaneReceipt — struct
// ===========================================================================

#[test]
fn lane_receipt_serde_round_trip() {
    let digest = Digest::from_bytes(b"lane_receipt");
    let lr = titania_core::LaneReceipt {
        lane: "Fmt".parse().unwrap(),
        evidence_digest: digest,
        clean: true,
    };
    let json = serde_json::to_string(&lr).unwrap();
    let back: titania_core::LaneReceipt = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", lr), format!("{:?}", back), "LaneReceipt must round-trip");
}

// ===========================================================================
// PolicyDiagnostic — struct
// ===========================================================================

#[test]
fn policy_diagnostic_serde_round_trip() {
    let d = make_diagnostic();
    let json = serde_json::to_string(&d).unwrap();
    let back: titania_core::PolicyDiagnostic = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", d), format!("{:?}", back), "PolicyDiagnostic must round-trip");
}

#[test]
fn policy_diagnostic_with_file_serde_round_trip() {
    let mut d = make_diagnostic();
    d.file = Some(WorkspacePath::new("policy.toml").unwrap());
    let json = serde_json::to_string(&d).unwrap();
    let back: titania_core::PolicyDiagnostic = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", d),
        format!("{:?}", back),
        "PolicyDiagnostic with file must round-trip"
    );
}

// ===========================================================================
// InputDiagnostic — struct
// ===========================================================================

#[test]
fn input_diagnostic_serde_round_trip() {
    let d = make_input_diagnostic();
    let json = serde_json::to_string(&d).unwrap();
    let back: titania_core::InputDiagnostic = serde_json::from_str(&json).unwrap();
    assert_eq!(format!("{:?}", d), format!("{:?}", back), "InputDiagnostic must round-trip");
}

#[test]
fn input_diagnostic_without_tool_serde_round_trip() {
    let d = titania_core::InputDiagnostic {
        message: "no tool".to_string(),
        tool: None,
        severity: titania_core::DiagnosticSeverity::Warning,
    };
    let json = serde_json::to_string(&d).unwrap();
    let back: titania_core::InputDiagnostic = serde_json::from_str(&json).unwrap();
    assert_eq!(
        format!("{:?}", d),
        format!("{:?}", back),
        "InputDiagnostic without tool must round-trip"
    );
}

// ===========================================================================
// DiagnosticSeverity — 2 variants
// ===========================================================================

#[test]
fn diagnostic_severity_serde_round_trip() {
    let severities: Vec<(&str, titania_core::DiagnosticSeverity)> = vec![
        ("error", titania_core::DiagnosticSeverity::Error),
        ("warning", titania_core::DiagnosticSeverity::Warning),
    ];
    for (label, sev) in severities {
        let json = serde_json::to_string(&sev).unwrap();
        let back: titania_core::DiagnosticSeverity = serde_json::from_str(&json).unwrap();
        assert_eq!(sev, back, "{label} round-trip failed");
        assert_eq!(json, format!("\"{label}\""));
    }
}
