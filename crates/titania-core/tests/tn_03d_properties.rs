//! Property tests for the v1 domain model.
//!
//! Each property is a hypothesis about invariants exercised at scale.

#![allow(clippy::as_conversions)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::type_complexity)]
#![allow(clippy::map_identity)]

use proptest::prelude::*;

use titania_core::{Digest, RuleId, TextRange, WorkspacePath};

// ===========================================================================
// Lane properties
// ===========================================================================

proptest! {
    // P1: from_str(to_string(l)) == Ok(l) for all lanes
    #[test]
    fn lane_from_str_to_string_round_trip(lane in any::<titania_core::Lane>()) {
        let s = lane.to_string();
        let back: titania_core::Lane = s.parse().expect("valid lane name should parse");
        assert_eq!(lane, back);
    }

    // P2: serde round-trip for all lanes
    #[test]
    fn lane_serde_round_trip_all(lane in any::<titania_core::Lane>()) {
        let json = serde_json::to_string(&lane).unwrap();
        let back: titania_core::Lane = serde_json::from_str(&json).unwrap();
        assert_eq!(lane, back);
    }

    // P3: all 10 serialized strings are unique
    #[test]
    fn lane_variants_are_unique_strings(_ in 0..256u32) {
        let names = [
            "Fmt", "Compile", "Clippy", "AstGrep", "Dylint",
            "PanicScan", "PolicyScan", "Test", "Deny", "Build",
        ];
        let mut strings: Vec<String> = Vec::with_capacity(10);
        for name in &names {
            let lane = name.parse::<titania_core::Lane>().expect(name);
            strings.push(lane.to_string());
        }
        let mut unique = strings.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 10, "all lane names must be distinct");
    }

    // ===========================================================================
    // Report properties
    // ===========================================================================

    // P4: Reject never has both collections empty
    #[test]
    fn report_reject_never_both_empty(
        code_size in 0usize..=5,
        gate_size in 0usize..=5,
    ) {
        let findings: Box<[titania_core::Finding]> = if code_size > 0 {
            let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
            let loc = WorkspacePath::new("src/lib.rs").unwrap();
            Box::new(std::iter::repeat_with(|| {
                titania_core::Finding::new(
                    "Fmt".parse().unwrap(),
                    rule_id.clone(),
                    titania_core::Location::Span {
                        file: loc.clone(),
                        line_start: 1, col_start: 0, line_end: 1, col_end: 10,
                    },
                    "msg".to_string(),
                    titania_core::RepairHint::use_iterator_pipeline("s".to_string()),
                    titania_core::FindingEffect::Reject,
                )
            }).take(code_size).collect::<Vec<_>>())
        } else {
            Box::new([])
        };

        let failures: Box<[titania_core::LaneFailure]> = if gate_size > 0 {
            Box::new(std::iter::repeat_with(|| {
                titania_core::LaneFailure::infra_failure(
                    "cargo".to_string(),
                    "reason".to_string(),
                )
            }).take(gate_size).collect::<Vec<_>>())
        } else {
            Box::new([])
        };

        let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
        let result = titania_core::Report::reject(findings, failures, per_lane);

        if code_size == 0 && gate_size == 0 {
            assert!(matches!(result, Err(titania_core::ReportError::BothEmpty)));
        } else {
            assert!(result.is_ok(), "non-empty collection should produce Ok");
        }
    }

    // P5: reject_kind classification is complete for non-empty collections
    #[test]
    fn reject_kind_classification_complete(
        code_size in 0usize..=5,
        gate_size in 0usize..=5,
    ) {
        let findings: Box<[titania_core::Finding]> = if code_size > 0 {
            let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
            let loc = WorkspacePath::new("src/lib.rs").unwrap();
            Box::new(std::iter::repeat_with(|| {
                titania_core::Finding::new(
                    "Fmt".parse().unwrap(),
                    rule_id.clone(),
                    titania_core::Location::Span {
                        file: loc.clone(),
                        line_start: 1, col_start: 0, line_end: 1, col_end: 10,
                    },
                    "msg".to_string(),
                    titania_core::RepairHint::use_iterator_pipeline("s".to_string()),
                    titania_core::FindingEffect::Reject,
                )
            }).take(code_size).collect::<Vec<_>>())
        } else {
            Box::new([])
        };

        let failures: Box<[titania_core::LaneFailure]> = if gate_size > 0 {
            Box::new(std::iter::repeat_with(|| {
                titania_core::LaneFailure::infra_failure(
                    "cargo".to_string(),
                    "reason".to_string(),
                )
            }).take(gate_size).collect::<Vec<_>>())
        } else {
            Box::new([])
        };

        let per_lane: Box<[titania_core::LaneOutcome]> = Box::new([]);
        let r = titania_core::Report::reject(findings, failures, per_lane).unwrap();

        // At least one collection is non-empty, so reject_kind must return Some
        assert!(r.reject_kind().is_some());
        assert!(matches!(
            r.reject_kind(),
            Some(titania_core::RejectKind::CodeOnly)
                | Some(titania_core::RejectKind::GateOnly)
                | Some(titania_core::RejectKind::Mixed)
        ));
    }

    // ===========================================================================
    // Finding properties
    // ===========================================================================

    // P6: Finding serde round-trip
    #[test]
    fn finding_serde_round_trip(
        lane_name in "[A-Z][a-zA-Z]*",
        line_start in 1u32..1000u32,
        col_start in 0u32..200u32,
        col_end in 0u32..200u32,
    ) {
        // Validate lane name is a real Lane
        if lane_name.parse::<titania_core::Lane>().is_err() {
            return;
        }

        let rule_id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
        let file = WorkspacePath::new("src/lib.rs").unwrap();
        let finding = titania_core::Finding::new(
            lane_name.parse().unwrap(),
            rule_id,
            titania_core::Location::Span {
                file,
                line_start,
                col_start,
                line_end: line_start,
                col_end,
            },
            "message".to_string(),
            titania_core::RepairHint::use_iterator_pipeline("suggestion".to_string()),
            titania_core::FindingEffect::Reject,
        );
        let json = serde_json::to_string(&finding).unwrap();
        let back: titania_core::Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(finding, back);
    }

    // P7: Finding lane matches constructed lane
    #[test]
    fn finding_lane_matches_constructed_lane(
        lane_name in "[A-Z][a-zA-Z]*",
    ) {
        if lane_name.parse::<titania_core::Lane>().is_err() {
            return;
        }
        let lane = lane_name.parse().unwrap();
        let rule_id = RuleId::new("TEST_RULE").unwrap();
        let file = WorkspacePath::new("src/lib.rs").unwrap();
        let finding = titania_core::Finding::new(
            lane,
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
        assert_eq!(finding.lane(), &lane);
    }

    // ===========================================================================
    // Location properties
    // ===========================================================================

    // P8: Span line_start >= 1
    #[test]
    fn location_span_line_start_ge_1(
        line_start in 1u32..1000u32,
        col_start in 0u32..200u32,
        line_end in 1u32..1000u32,
        col_end in 0u32..200u32,
    ) {
        let file = WorkspacePath::new("src/lib.rs").unwrap();
        let loc = titania_core::Location::span(file, line_start, col_start, line_end, col_end).unwrap();
        assert!(matches!(loc, titania_core::Location::Span { .. }));
        assert_eq!(loc.line_start(), line_start);
    }

    // P9: Span columns non-negative (always true for u32)
    #[test]
    fn location_span_col_non_negative(
        col_start in 0u32..200u32,
        col_end in 0u32..200u32,
    ) {
        let file = WorkspacePath::new("src/lib.rs").unwrap();
        let loc = titania_core::Location::span(file, 1, col_start, 1, col_end).unwrap();
        assert!(loc.col_start() >= 0);
        assert!(loc.col_end() >= 0);
    }

    // P10: Location serde round-trip all variants
    #[test]
    fn location_serde_round_trip_all_variants(
        line_start in 1u32..100u32,
        col_start in 0u32..50u32,
        col_end in 0u32..50u32,
        crate_name in "[a-z_][a-z0-9_]*",
        tool_name in "[a-z][a-z0-9-]*",
    ) {
        // Span
        let span = titania_core::Location::Span {
            file: WorkspacePath::new("src/lib.rs").unwrap(),
            line_start, col_start, line_end: line_start + 1, col_end,
        };
        let span_json = serde_json::to_string(&span).unwrap();
        let span_back: titania_core::Location = serde_json::from_str(&span_json).unwrap();
        assert_eq!(span, span_back);

        // Dependency
        let dep = titania_core::Location::Dependency {
            crate_name: crate_name.clone(),
            version: "1.0.0".to_string(),
        };
        let dep_json = serde_json::to_string(&dep).unwrap();
        let dep_back: titania_core::Location = serde_json::from_str(&dep_json).unwrap();
        assert_eq!(dep, dep_back);

        // Workspace
        let ws = titania_core::Location::Workspace;
        let ws_json = serde_json::to_string(&ws).unwrap();
        let ws_back: titania_core::Location = serde_json::from_str(&ws_json).unwrap();
        assert_eq!(ws, ws_back);

        // Manifest
        let manifest = titania_core::Location::Manifest {
            file: WorkspacePath::new("Cargo.toml").unwrap(),
        };
        let manifest_json = serde_json::to_string(&manifest).unwrap();
        let manifest_back: titania_core::Location = serde_json::from_str(&manifest_json).unwrap();
        assert_eq!(manifest, manifest_back);

        // Tool
        let tool = titania_core::Location::Tool {
            name: tool_name.clone(),
            version: "1.0".to_string(),
        };
        let tool_json = serde_json::to_string(&tool).unwrap();
        let tool_back: titania_core::Location = serde_json::from_str(&tool_json).unwrap();
        assert_eq!(tool, tool_back);
    }

    // P11: No two Location variants produce identical JSON
    #[test]
    fn location_variants_distinguishable(_ in 0u32..10) {
        let span = titania_core::Location::Span {
            file: WorkspacePath::new("src/lib.rs").unwrap(),
            line_start: 1, col_start: 0, line_end: 1, col_end: 10,
        };
        let dep = titania_core::Location::Dependency {
            crate_name: "serde".to_string(),
            version: "1.0".to_string(),
        };
        let manifest = titania_core::Location::Manifest {
            file: WorkspacePath::new("Cargo.toml").unwrap(),
        };
        let ws = titania_core::Location::Workspace;
        let tool = titania_core::Location::Tool {
            name: "clippy".to_string(),
            version: "0.1".to_string(),
        };

        let all: Vec<titania_core::Location> = vec![span, dep, manifest, ws, tool];
        let mut jsons: Vec<String> = Vec::with_capacity(all.len());
        for loc in &all {
            jsons.push(serde_json::to_string(loc).unwrap());
        }
        let mut unique = jsons.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 5, "all Location variants must produce distinct JSON");
    }

    // ===========================================================================
    // RepairHint properties
    // ===========================================================================

    // P12: RepairHint serde round-trip all 7 variants
    #[test]
    fn repair_hint_serde_round_trip_all(_ in 0u32..10) {
        let patch = titania_core::RepairHint::patch(
            "src/lib.rs".to_string(),
            TextRange::new(0, 10).unwrap(),
            "fixed".to_string(),
        ).unwrap();
        let patch_json = serde_json::to_string(&patch).unwrap();
        let patch_back: titania_core::RepairHint = serde_json::from_str(&patch_json).unwrap();
        assert_eq!(patch, patch_back);

        let iter_pipe = titania_core::RepairHint::use_iterator_pipeline("use .into_iter()".to_string());
        let iter_json = serde_json::to_string(&iter_pipe).unwrap();
        let iter_back: titania_core::RepairHint = serde_json::from_str(&iter_json).unwrap();
        assert_eq!(iter_pipe, iter_back);

        let flatten = titania_core::RepairHint::flatten_nesting("reduce depth".to_string());
        let flatten_json = serde_json::to_string(&flatten).unwrap();
        let flatten_back: titania_core::RepairHint = serde_json::from_str(&flatten_json).unwrap();
        assert_eq!(flatten, flatten_back);

        let checked = titania_core::RepairHint::use_checked_arithmetic("checked_add".to_string());
        let checked_json = serde_json::to_string(&checked).unwrap();
        let checked_back: titania_core::RepairHint = serde_json::from_str(&checked_json).unwrap();
        assert_eq!(checked, checked_back);

        let remove = titania_core::RepairHint::remove_allow_attribute("allow(dead_code)".to_string());
        let remove_json = serde_json::to_string(&remove).unwrap();
        let remove_back: titania_core::RepairHint = serde_json::from_str(&remove_json).unwrap();
        assert_eq!(remove, remove_back);

        let replace = titania_core::RepairHint::replace_dependency(
            "old = \"0.1\"".to_string(),
            "new = \"0.2\"".to_string(),
        );
        let replace_json = serde_json::to_string(&replace).unwrap();
        let replace_back: titania_core::RepairHint = serde_json::from_str(&replace_json).unwrap();
        assert_eq!(replace, replace_back);

        let human = titania_core::RepairHint::requires_human_review("fix manually".to_string());
        let human_json = serde_json::to_string(&human).unwrap();
        let human_back: titania_core::RepairHint = serde_json::from_str(&human_json).unwrap();
        assert_eq!(human, human_back);
    }

    // ===========================================================================
    // ProcessTermination properties
    // ===========================================================================

    // P13: ProcessTermination serde round-trip
    #[test]
    fn process_termination_serde_round_trip(
        code in any::<i32>(),
        signal in 1i32..32i32,
    ) {
        let exited = titania_core::ProcessTermination::exited(code);
        let exited_json = serde_json::to_string(&exited).unwrap();
        let exited_back: titania_core::ProcessTermination = serde_json::from_str(&exited_json).unwrap();
        assert_eq!(exited, exited_back);

        let signaled = titania_core::ProcessTermination::signaled(signal).unwrap();
        let signaled_json = serde_json::to_string(&signaled).unwrap();
        let signaled_back: titania_core::ProcessTermination = serde_json::from_str(&signaled_json).unwrap();
        assert_eq!(signaled, signaled_back);

        let timed = titania_core::ProcessTermination::timed_out();
        let timed_json = serde_json::to_string(&timed).unwrap();
        let timed_back: titania_core::ProcessTermination = serde_json::from_str(&timed_json).unwrap();
        assert_eq!(timed, timed_back);

        let mem = titania_core::ProcessTermination::memory_limit_exceeded();
        let mem_json = serde_json::to_string(&mem).unwrap();
        let mem_back: titania_core::ProcessTermination = serde_json::from_str(&mem_json).unwrap();
        assert_eq!(mem, mem_back);

        let spawn = titania_core::ProcessTermination::spawn_failed();
        let spawn_json = serde_json::to_string(&spawn).unwrap();
        let spawn_back: titania_core::ProcessTermination = serde_json::from_str(&spawn_json).unwrap();
        assert_eq!(spawn, spawn_back);
    }

    // ===========================================================================
    // QualityReceipt properties
    // ===========================================================================

    // P14: QualityReceipt schema_version always 1
    #[test]
    fn quality_receipt_schema_version_is_one(_ in 0u32..10) {
        let digest = Digest::from_bytes(b"test");
        let lanes: Box<[titania_core::LaneReceipt]> = Box::new([]);
        let receipt = titania_core::QualityReceipt::new(
            1,
            titania_core::GateScope::Edit,
            digest.clone(),
            digest.clone(),
            digest.clone(),
            digest.clone(),
            lanes,
        ).unwrap();
        assert_eq!(receipt.schema_version(), 1);
    }

    // ===========================================================================
    // RejectKind properties
    // ===========================================================================

    // P15: RejectKind serde round-trip
    #[test]
    fn reject_kind_serde_round_trip(_ in 0u32..10) {
        for kind in [
            titania_core::RejectKind::CodeOnly,
            titania_core::RejectKind::GateOnly,
            titania_core::RejectKind::Mixed,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: titania_core::RejectKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }
}
