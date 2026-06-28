//! Layer 0: `cargo fmt --check` lane runner.

use std::time::Duration;

use xtask_core::{
    CommandEvidence, Digest, Finding, FindingEffect, Lane, LaneEvidence, LaneFailure, LaneOutcome,
    Location, ProcessTermination, RepairHint, RuleId, WorkspacePath,
};

use crate::LaneRunner;
use crate::process::run_command;

const FMT_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs `cargo fmt --all -- --check`.
pub struct FmtLane;

impl LaneRunner for FmtLane {
    fn lane(&self) -> Lane {
        Lane::Fmt
    }

    fn run(&self) -> LaneOutcome {
        let result = match run_command(
            "cargo",
            &["fmt", "--all", "--", "--check"],
            None,
            FMT_TIMEOUT,
        ) {
            Ok(r) => r,
            Err(e) => {
                return LaneOutcome::Failed(LaneFailure::InfraFailure {
                    tool: "cargo fmt".to_owned(),
                    reason: format!("{e:?}"),
                });
            }
        };

        if result.is_success() {
            return LaneOutcome::Clean {
                evidence: make_evidence(),
            };
        }

        let findings = parse_fmt_diff(&result.stderr_str());
        if findings.is_empty() {
            LaneOutcome::Failed(LaneFailure::ToolFailure {
                tool: "cargo fmt".to_owned(),
                termination: result.termination,
            })
        } else {
            LaneOutcome::Findings(findings)
        }
    }
}

fn parse_fmt_diff(output: &str) -> Box<[Finding]> {
    let mut findings = Vec::new();
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Diff in ")
            && let Some((file, line_num)) = parse_diff_location(rest)
        {
            findings.push(Finding {
                lane: Lane::Fmt,
                rule_id: RuleId("HOLZMAN_FORMAT_DRIFT".to_owned()),
                location: Location::Span {
                    file: WorkspacePath(file),
                    line_start: line_num,
                    col_start: 0,
                    line_end: line_num,
                    col_end: 0,
                },
                message: "Formatting does not match rustfmt policy".to_owned(),
                repair: RepairHint::RequiresHumanReview {
                    note: "Run `cargo fmt --all` to fix".to_owned(),
                },
                effect: FindingEffect::Reject,
            });
        }
    }
    findings.into_boxed_slice()
}

/// Parse "file.rs at line 42:" into (`file_path`, `line_number`).
fn parse_diff_location(rest: &str) -> Option<(String, u32)> {
    let mut parts = rest.split(" at line ");
    let file = parts.next()?.to_owned();
    let line_part = parts.next()?;
    let line_str = line_part.trim_end_matches(':');
    let line_num = line_str.parse::<u32>().ok()?;
    Some((file, line_num))
}

fn make_evidence() -> LaneEvidence {
    LaneEvidence {
        command: CommandEvidence {
            executable: "cargo".to_owned(),
            argv: Box::from([
                "fmt".to_owned(),
                "--all".to_owned(),
                "--".to_owned(),
                "--check".to_owned(),
            ]),
        },
        tool_version: String::new(),
        exit_status: ProcessTermination::Exited { code: 0 },
        parsed_result_digest: Digest::from_bytes(b"clean"),
    }
}
