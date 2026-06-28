//! Layer 2: `cargo clippy --lib --bins` lane runner (source-only, strict).

use std::time::Duration;

use serde::Deserialize;
use xtask_core::{
    Finding, FindingEffect, Lane, LaneEvidence, LaneFailure, LaneOutcome, Location,
    ProcessTermination, RepairHint, RuleId, WorkspacePath,
};

use crate::LaneRunner;
use crate::process::run_command;

const CLIPPY_TIMEOUT: Duration = Duration::from_mins(2);

/// The critical clippy lints passed as `-F` (forbid — `#[allow]` cannot lower).
const CRITICAL_LINTS: &[&str] = &[
    "-F",
    "clippy::unwrap_used",
    "-F",
    "clippy::expect_used",
    "-F",
    "clippy::panic",
    "-F",
    "clippy::panic_in_result_fn",
    "-F",
    "clippy::todo",
    "-F",
    "clippy::unimplemented",
    "-F",
    "clippy::indexing_slicing",
    "-F",
    "clippy::string_slice",
    "-F",
    "clippy::get_unwrap",
    "-F",
    "clippy::arithmetic_side_effects",
    "-F",
    "clippy::dbg_macro",
    "-D",
    "warnings",
];

/// Runs `cargo clippy --workspace --lib --bins --message-format=json` with strict lints.
pub struct ClippyLane;

/// A JSON message from clippy.
#[derive(Debug, Deserialize)]
struct ClippyMessage {
    reason: Option<String>,
    message: Option<ClippyDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct ClippyDiagnostic {
    level: Option<String>,
    code: Option<ClippyErrorCode>,
    message: Option<String>,
    spans: Option<Vec<ClippySpan>>,
}

#[derive(Debug, Deserialize)]
struct ClippyErrorCode {
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClippySpan {
    file_name: Option<String>,
    line_start: Option<u32>,
    column_start: Option<u32>,
    line_end: Option<u32>,
    column_end: Option<u32>,
}

impl LaneRunner for ClippyLane {
    fn lane(&self) -> Lane {
        Lane::Clippy
    }

    fn run(&self) -> LaneOutcome {
        let mut args = vec![
            "clippy".to_owned(),
            "--workspace".to_owned(),
            "--lib".to_owned(),
            "--bins".to_owned(),
            "--frozen".to_owned(),
            "--message-format=json".to_owned(),
            "--".to_owned(),
        ];
        for lint in CRITICAL_LINTS {
            args.push((*lint).to_owned());
        }
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        let result = match run_command("cargo", &arg_refs, None, CLIPPY_TIMEOUT) {
            Ok(r) => r,
            Err(e) => {
                return LaneOutcome::Failed(LaneFailure::InfraFailure {
                    tool: "cargo clippy".to_owned(),
                    reason: format!("{e:?}"),
                });
            }
        };

        let findings = parse_clippy_messages(&result.stdout_str());

        match result.termination {
            ProcessTermination::Exited { code: 0 } if findings.is_empty() => LaneOutcome::Clean {
                evidence: make_evidence(),
            },
            ProcessTermination::Exited { .. } => {
                if findings.is_empty() {
                    LaneOutcome::Failed(LaneFailure::ToolFailure {
                        tool: "cargo clippy".to_owned(),
                        termination: result.termination,
                    })
                } else {
                    LaneOutcome::Findings(findings)
                }
            }
            ref term => LaneOutcome::Failed(LaneFailure::ToolFailure {
                tool: "cargo clippy".to_owned(),
                termination: term.clone(),
            }),
        }
    }
}

fn parse_clippy_messages(output: &str) -> Box<[Finding]> {
    let mut findings = Vec::new();
    for line in output.lines() {
        let msg: ClippyMessage = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if msg.reason.as_deref() != Some("compiler-message") {
            continue;
        }
        let Some(diag) = msg.message else {
            continue;
        };
        let level = diag.level.as_deref().unwrap_or("warning");
        if level != "error" && level != "warning" {
            continue;
        }

        let lint_code = diag
            .code
            .as_ref()
            .and_then(|c| c.code.as_deref())
            .unwrap_or("unknown");

        let rule = clippy_to_rule(lint_code);
        let effect = if level == "error" {
            FindingEffect::Reject
        } else {
            FindingEffect::Informational
        };

        let span = diag.spans.as_ref().and_then(|s| s.first()).and_then(|s| {
            Some(Location::Span {
                file: WorkspacePath(s.file_name.as_deref()?.to_owned()),
                line_start: s.line_start.unwrap_or(1),
                col_start: s.column_start.unwrap_or(0),
                line_end: s.line_end.unwrap_or(1),
                col_end: s.column_end.unwrap_or(0),
            })
        });

        findings.push(Finding {
            lane: Lane::Clippy,
            rule_id: RuleId(rule),
            location: span.unwrap_or(Location::Workspace),
            message: diag.message.unwrap_or_else(|| "Clippy lint".to_owned()),
            repair: RepairHint::RequiresHumanReview {
                note: format!("Fix clippy lint: {lint_code}"),
            },
            effect,
        });
    }
    findings.into_boxed_slice()
}

/// Map a clippy lint code to an Xtask rule ID.
fn clippy_to_rule(lint_code: &str) -> String {
    let bare = lint_code.strip_prefix("clippy::").unwrap_or(lint_code);

    match bare {
        "unwrap_used" => "HOLZMAN_PANIC_UNWRAP".to_owned(),
        "expect_used" => "HOLZMAN_PANIC_EXPECT".to_owned(),
        "panic" => "HOLZMAN_PANIC".to_owned(),
        "indexing_slicing" => "HOLZMAN_PANIC_INDEXING".to_owned(),
        "string_slice" => "HOLZMAN_PANIC_STRING_SLICE".to_owned(),
        "get_unwrap" => "HOLZMAN_PANIC_GET_UNWRAP".to_owned(),
        "arithmetic_side_effects" => "HOLZMAN_CHECKED_ARITHMETIC".to_owned(),
        "as_conversions" => "HOLZMAN_UNSAFE_AS_CAST".to_owned(),
        "todo" | "unimplemented" => "HOLZMAN_PANIC_TODO".to_owned(),
        "dbg_macro" => "HOLZMAN_PANIC_DBG".to_owned(),
        "panic_in_result_fn" => "HOLZMAN_PANIC_IN_RESULT".to_owned(),
        other => format!("CLIPPY_{other}").to_uppercase(),
    }
}

fn make_evidence() -> LaneEvidence {
    LaneEvidence {
        command: xtask_core::CommandEvidence {
            executable: "cargo".to_owned(),
            argv: Box::from([
                "clippy".to_owned(),
                "--workspace".to_owned(),
                "--lib".to_owned(),
                "--bins".to_owned(),
            ]),
        },
        tool_version: String::new(),
        exit_status: ProcessTermination::Exited { code: 0 },
        parsed_result_digest: xtask_core::Digest::from_bytes(b"clean"),
    }
}
