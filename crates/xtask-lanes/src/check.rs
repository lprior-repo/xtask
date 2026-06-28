//! Layer 1: `cargo check --workspace --message-format=json` lane runner.

use std::time::Duration;

use serde::Deserialize;
use xtask_core::{
    Finding, FindingEffect, Lane, LaneEvidence, LaneFailure, LaneOutcome, Location,
    ProcessTermination, RepairHint, RuleId, WorkspacePath,
};

use crate::LaneRunner;
use crate::process::run_command;

const CHECK_TIMEOUT: Duration = Duration::from_mins(2);

/// Runs `cargo check --workspace --message-format=json`.
pub struct CheckLane;

/// A single JSON message from `cargo check --message-format=json`.
#[derive(Debug, Deserialize)]
struct CargoMessage {
    reason: Option<String>,
    message: Option<CompilerMessage>,
}

#[derive(Debug, Deserialize)]
struct CompilerMessage {
    level: Option<String>,
    code: Option<ErrorCode>,
    message: Option<String>,
    spans: Option<Vec<DiagnosticSpan>>,
}

#[derive(Debug, Deserialize)]
struct ErrorCode {
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiagnosticSpan {
    file_name: Option<String>,
    line_start: Option<u32>,
    column_start: Option<u32>,
    line_end: Option<u32>,
    column_end: Option<u32>,
}

impl LaneRunner for CheckLane {
    fn lane(&self) -> Lane {
        Lane::Check
    }

    fn run(&self) -> LaneOutcome {
        let result = match run_command(
            "cargo",
            &["check", "--workspace", "--frozen", "--message-format=json"],
            None,
            CHECK_TIMEOUT,
        ) {
            Ok(r) => r,
            Err(e) => {
                return LaneOutcome::Failed(LaneFailure::InfraFailure {
                    tool: "cargo check".to_owned(),
                    reason: format!("{e:?}"),
                });
            }
        };

        let findings = parse_compiler_messages(&result.stdout_str());

        match result.termination {
            ProcessTermination::Exited { code: 0 } if findings.is_empty() => LaneOutcome::Clean {
                evidence: make_evidence("cargo check"),
            },
            ProcessTermination::Exited { .. } => {
                if findings.is_empty() {
                    LaneOutcome::Failed(LaneFailure::ToolFailure {
                        tool: "cargo check".to_owned(),
                        termination: result.termination,
                    })
                } else {
                    LaneOutcome::Findings(findings)
                }
            }
            ref term => LaneOutcome::Failed(LaneFailure::ToolFailure {
                tool: "cargo check".to_owned(),
                termination: term.clone(),
            }),
        }
    }
}

fn parse_compiler_messages(output: &str) -> Box<[Finding]> {
    let mut findings = Vec::new();
    for line in output.lines() {
        let msg: CargoMessage = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if msg.reason.as_deref() != Some("compiler-message") {
            continue;
        }
        let Some(cm) = msg.message else { continue };
        if cm.level.as_deref() != Some("error") {
            continue;
        }
        let rule = cm
            .code
            .as_ref()
            .and_then(|c| c.code.as_deref())
            .map_or_else(
                || "HOLZMAN_COMPILE_ERROR".to_owned(),
                |c| format!("HOLZMAN_COMPILE_{c}"),
            );

        let span = cm.spans.as_ref().and_then(|s| s.first()).and_then(|s| {
            Some(Location::Span {
                file: WorkspacePath(s.file_name.as_deref()?.to_owned()),
                line_start: s.line_start.unwrap_or(1),
                col_start: s.column_start.unwrap_or(0),
                line_end: s.line_end.unwrap_or(1),
                col_end: s.column_end.unwrap_or(0),
            })
        });

        findings.push(Finding {
            lane: Lane::Check,
            rule_id: RuleId(rule),
            location: span.unwrap_or(Location::Workspace),
            message: cm.message.unwrap_or_else(|| "Compilation error".to_owned()),
            repair: RepairHint::RequiresHumanReview {
                note: "Fix the compilation error".to_owned(),
            },
            effect: FindingEffect::Reject,
        });
    }
    findings.into_boxed_slice()
}

fn make_evidence(tool: &str) -> LaneEvidence {
    LaneEvidence {
        command: xtask_core::CommandEvidence {
            executable: "cargo".to_owned(),
            argv: Box::from([tool.to_owned()]),
        },
        tool_version: String::new(),
        exit_status: ProcessTermination::Exited { code: 0 },
        parsed_result_digest: xtask_core::Digest::from_bytes(b"clean"),
    }
}
