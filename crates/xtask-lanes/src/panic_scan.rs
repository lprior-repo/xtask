//! Layer 4: production panic/assert macro scan using `rg`.
//!
//! Uses a tightened regex that requires `(` after the macro name, plus
//! post-filtering of comment lines. This is heuristic — not a parser.
//! False positives from block comments and string literals are a known
//! residual. A parser-backed scan (syn) is a future improvement.

use std::time::Duration;

use xtask_core::{
    CommandEvidence, Digest, Finding, FindingEffect, Lane, LaneEvidence, LaneFailure, LaneOutcome,
    Location, ProcessTermination, RepairHint, RuleId, WorkspacePath,
};

use crate::LaneRunner;
use crate::process::run_command;

const SCAN_TIMEOUT: Duration = Duration::from_mins(1);

/// Scans for `assert!(`, `assert_eq!(`, `assert_ne!(`, `unreachable!(` in production source.
///
/// Excludes tests, benches, examples, and build scripts.
pub struct PanicAssertScanLane;

/// The regex pattern for panic-producing macro INVOCATIONS (requires opening paren).
const PANIC_PATTERN: &str = r"(^|[^A-Za-z0-9_])(assert!|assert_eq!|assert_ne!|unreachable!)\s*\(";

/// Glob exclusions for non-production code.
const EXCLUDE_GLOBS: &[&str] = &[
    "--glob",
    "!**/tests/**",
    "--glob",
    "!**/benches/**",
    "--glob",
    "!**/examples/**",
    "--glob",
    "!build.rs",
    "--glob",
    "*.rs",
];

impl LaneRunner for PanicAssertScanLane {
    fn lane(&self) -> Lane {
        Lane::PanicAssertScan
    }

    fn run(&self) -> LaneOutcome {
        let mut args = vec!["-n".to_owned()];
        for glob in EXCLUDE_GLOBS {
            args.push((*glob).to_owned());
        }
        args.push(PANIC_PATTERN.to_owned());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        let result = match run_command("rg", &arg_refs, None, SCAN_TIMEOUT) {
            Ok(r) => r,
            Err(e) => {
                return LaneOutcome::Failed(LaneFailure::InfraFailure {
                    tool: "rg".to_owned(),
                    reason: format!("rg not found or failed: {e:?}"),
                });
            }
        };

        // rg exit codes: 0 = matches found, 1 = no matches, 2 = error
        match result.termination {
            ProcessTermination::Exited { code: 1 } => LaneOutcome::Clean {
                evidence: make_evidence(),
            },
            ProcessTermination::Exited { code: 0 } => {
                let findings = parse_rg_matches(&result.stdout_str());
                if findings.is_empty() {
                    LaneOutcome::Clean {
                        evidence: make_evidence(),
                    }
                } else {
                    LaneOutcome::Findings(findings)
                }
            }
            ProcessTermination::Exited { code: _ } => {
                LaneOutcome::Failed(LaneFailure::ToolFailure {
                    tool: "rg".to_owned(),
                    termination: ProcessTermination::Exited { code: 2 },
                })
            }
            ref term => LaneOutcome::Failed(LaneFailure::ToolFailure {
                tool: "rg".to_owned(),
                termination: term.clone(),
            }),
        }
    }
}

fn parse_rg_matches(output: &str) -> Box<[Finding]> {
    let mut findings = Vec::new();
    for line in output.lines() {
        // rg -n output format: file:line:match
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        let (file, line_num_str, matched_text) = match (parts.first(), parts.get(1), parts.get(2)) {
            (Some(f), Some(l), Some(m)) => (*f, *l, *m),
            _ => continue,
        };

        // Skip comment lines — heuristic false-positive filter.
        let trimmed = matched_text.trim_start();
        if trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || trimmed.starts_with("//!")
        {
            continue;
        }

        let line_num = line_num_str.parse::<u32>().unwrap_or(1);

        // Extract the macro name from the match
        let macro_name = matched_text
            .split(['(', '{', ' '])
            .next()
            .unwrap_or("assert!")
            .trim();

        findings.push(Finding {
            lane: Lane::PanicAssertScan,
            rule_id: RuleId(
                format!("HOLZMAN_PANIC_{}", macro_name.replace('!', "")).to_uppercase(),
            ),
            location: Location::Span {
                file: WorkspacePath(file.to_owned()),
                line_start: line_num,
                col_start: 0,
                line_end: line_num,
                col_end: 0,
            },
            message: format!("Production panic macro `{macro_name}` found outside tests"),
            repair: RepairHint::RequiresHumanReview {
                note: "Remove the panic macro or move it to test code. Use typed errors instead."
                    .to_owned(),
            },
            effect: FindingEffect::Reject,
        });
    }
    findings.into_boxed_slice()
}

fn make_evidence() -> LaneEvidence {
    LaneEvidence {
        command: CommandEvidence {
            executable: "rg".to_owned(),
            argv: Box::from(["rg".to_owned(), "-n".to_owned()]),
        },
        tool_version: String::new(),
        exit_status: ProcessTermination::Exited { code: 1 },
        parsed_result_digest: Digest::from_bytes(b"clean"),
    }
}
