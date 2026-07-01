//! Fail-closed wrapper: refuses cargo test runs that executed zero applicable tests.
//!
//! Rust re-implementation of the bash lane in
//! `titania/scripts/guard-zero-tests.sh`. Run via
//! `cargo run --bin guard_zero_tests -- -- <cargo-test-args...>` from the
//! repository root or via the matching Moon task in `.moon/tasks/all.yml`.
//!
//! Exit codes: 0 = >0 applicable tests executed, 1 = zero tests or parse
//! failure, 2 = usage error.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::env;

use thiserror::Error;
use titania_core::TargetProject;
use titania_lanes::{CommandIn, CommandOutput, LaneError, LaneExit, current_target_project, exit};

const USAGE: &str = "usage: guard_zero_tests [--] <cargo-test-args>\n  \
     exit 0: >0 applicable tests executed\n  \
     exit 1: zero applicable tests or parse failure\n  \
     exit 2: usage error";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("{USAGE}");
        return exit(LaneExit::Clean);
    }
    match parse_lane_input(args) {
        LaneInput::MissingCommand => {
            eprintln!("guard-zero-tests: no command supplied");
            eprintln!("{USAGE}");
            exit(LaneExit::Usage)
        }
        LaneInput::Run(cmd_args) => {
            let target = match current_target_project() {
                Ok(target) => target,
                Err(err) => {
                    eprintln!("[guard-zero-tests] cannot resolve target project: {err}");
                    return exit(LaneExit::Usage);
                }
            };
            match run_lane(&target, &cmd_args) {
                Ok(()) => exit(LaneExit::Clean),
                Err(err) => {
                    eprintln!("[guard-zero-tests] {err}");
                    exit(LaneExit::Violations)
                }
            }
        }
    }
}

enum LaneInput {
    Run(Vec<String>),
    MissingCommand,
}

enum TestEvidence {
    Applicable(u32),
    NotApplicable,
}

type ParsedCommand<'a> = (&'a str, &'a [String]);

/// Bin-local errors. `Command` wraps a `LaneError` from the subprocess
/// layer; `Parse` covers guard-zero-tests-specific parse failures (empty
/// command line, non-numeric test counts, malformed cargo output).
/// `ZeroApplicable` is the semantic failure — the command completed but
/// ran zero applicable tests. Keeping it as a typed variant (not a string
/// `Err`) lets `main` emit `LaneExit::Violations` semantically.
#[derive(Debug, Error)]
enum GuardError {
    #[error(transparent)]
    Command(#[from] LaneError),
    #[error("{0}")]
    Parse(String),
    #[error("zero applicable tests executed")]
    ZeroApplicable,
}

fn parse_lane_input(args: Vec<String>) -> LaneInput {
    if args.is_empty() { LaneInput::MissingCommand } else { LaneInput::Run(args) }
}

fn run_lane(target: &TargetProject, cmd_args: &[String]) -> Result<(), GuardError> {
    eprintln!("[guard-zero-tests] running: {}", cmd_args.join(" "));
    let (program, passthrough) = parse_command_args(cmd_args).map_err(GuardError::Parse)?;
    let output = run_test_command(target, program, passthrough)?;
    let combined = combine_output(&output.stdout, &output.stderr);
    reject_failed_command(&output, &combined)?;
    report_test_evidence(parse_test_count(&combined)?)
}

fn parse_command_args(cmd_args: &[String]) -> Result<ParsedCommand<'_>, String> {
    // Strip a leading `--` separator so callers can pass it conventionally
    // (`guard-zero-tests -- /bin/sh -c '...'`). Without this, `--` becomes
    // the program name and the lane tries to run a binary called `--`.
    let trimmed: &[String] = match cmd_args.split_first() {
        Some((first, rest)) if first == "--" => rest,
        _ => cmd_args,
    };
    trimmed
        .split_first()
        .map(|(program, passthrough)| (program.as_str(), passthrough))
        .ok_or_else(|| "guard-zero-tests: empty command".to_string())
}

fn run_test_command<'a>(
    target: &'a TargetProject,
    program: &'a str,
    passthrough: &'a [String],
) -> Result<CommandOutput, LaneError> {
    let mut command = CommandIn::new(target, program)?;
    command.inherit_env();
    passthrough.iter().for_each(|arg| {
        command.arg(arg.as_str());
    });
    command.run_capture_raw()
}

fn reject_failed_command(output: &CommandOutput, combined: &str) -> Result<(), GuardError> {
    match output.status.code() {
        Some(0) => Ok(()),
        Some(code) => reject_nonzero_command(code, combined),
        None => reject_signaled_command(combined),
    }
}

fn reject_nonzero_command(code: i32, combined: &str) -> Result<(), GuardError> {
    eprintln!("[guard-zero-tests] cargo test exited {code} — treating as tooling failure");
    if let Some(n) = extract_applicable_count(combined) {
        eprintln!("[guard-zero-tests] applicable test count: {n} (cargo failed with exit {code})");
    }
    eprintln!("{combined}");
    Err(GuardError::Parse(format!("command exited with status {code}")))
}

fn reject_signaled_command(combined: &str) -> Result<(), GuardError> {
    eprintln!("[guard-zero-tests] command terminated by signal");
    eprintln!("{combined}");
    Err(GuardError::Parse("command terminated by signal".to_string()))
}

fn parse_test_count(combined: &str) -> Result<u32, GuardError> {
    extract_applicable_count(combined)
        .ok_or_else(|| GuardError::Parse("no applicable test count in cargo output".to_string()))
}

fn report_test_evidence(count: u32) -> Result<(), GuardError> {
    match classify_evidence(count) {
        TestEvidence::Applicable(count) => {
            eprintln!("[guard-zero-tests] PASS: {count} applicable tests executed");
            Ok(())
        }
        TestEvidence::NotApplicable => {
            eprintln!(
                "[guard-zero-tests] FAIL: command completed but executed zero applicable tests"
            );
            Err(GuardError::ZeroApplicable)
        }
    }
}

fn classify_evidence(count: u32) -> TestEvidence {
    if count == 0 { TestEvidence::NotApplicable } else { TestEvidence::Applicable(count) }
}

fn combine_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut combined = String::new();
    if let Ok(s) = std::str::from_utf8(stdout) {
        combined.push_str(s);
    }
    if let Ok(s) = std::str::from_utf8(stderr) {
        combined.push_str(s);
    }
    combined
}

/// Try four patterns the bash script handled, in order, returning a summed
/// non-negative `u32` from the first pattern family present.
fn extract_applicable_count(text: &str) -> Option<u32> {
    extract_running_n(text)
        .or_else(|| extract_libtest_passed(text))
        .or_else(|| extract_cargo_test_passed(text))
        .or_else(|| extract_cargo_test_filtered(text))
}

/// Format 1: lines that look like `running 5 tests` / `running 0 tests`.
fn extract_running_n(text: &str) -> Option<u32> {
    sum_line_counts(text, running_line_count)
}

/// Format 2: `test result: ok. 5 passed; 0 failed; ...`.
fn extract_libtest_passed(text: &str) -> Option<u32> {
    sum_line_counts(text, libtest_passed_count)
}

/// Format 3: `cargo test: 5 passed (1 suite, 0.08s)`.
fn extract_cargo_test_passed(text: &str) -> Option<u32> {
    sum_line_counts(text, cargo_test_passed_count)
}

/// Format 4 is covered by [`cargo_test_passed_count`].
fn extract_cargo_test_filtered(text: &str) -> Option<u32> {
    sum_line_counts(text, cargo_test_passed_count)
}

/// Sum matches across `text`. Returns `None` (not `Some(0)`) when no
/// line matched, so an empty parse doesn't masquerade as "zero tests".
fn sum_line_counts(text: &str, parse: fn(&str) -> Option<u32>) -> Option<u32> {
    let counts: Vec<u32> = text.lines().filter_map(parse).collect();
    if counts.is_empty() { None } else { Some(counts.into_iter().sum()) }
}

fn running_line_count(line: &str) -> Option<u32> {
    let rest = line.trim_start().strip_prefix("running ")?;
    let n = rest.split_whitespace().next()?;
    let stripped = n.strip_suffix(',').unwrap_or(n);
    stripped.parse::<u32>().ok()
}

fn libtest_passed_count(line: &str) -> Option<u32> {
    let rest = line.trim_start().strip_prefix("test result:")?.trim_start();
    let after = rest.strip_prefix("ok.")?;
    let after = after.trim_start();
    let n = after.split_whitespace().next()?;
    let stripped = n.strip_suffix(',').unwrap_or(n);
    stripped.parse::<u32>().ok()
}

fn cargo_test_passed_count(line: &str) -> Option<u32> {
    let rest = line.trim_start().strip_prefix("cargo test:")?.trim_start();
    let n = rest.split_whitespace().next()?;
    let stripped = n.strip_suffix(',').unwrap_or(n);
    stripped.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::{libtest_passed_count, parse_command_args, running_line_count, sum_line_counts};

    #[test]
    fn parse_command_args_rejects_empty() {
        let result = parse_command_args(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_command_args_strips_leading_dashdash_separator() {
        let args: Vec<String> = vec!["--".into(), "/bin/sh".into(), "-c".into(), "true".into()];
        let (program, passthrough) = parse_command_args(&args).expect("non-empty");
        assert_eq!(program, "/bin/sh");
        assert_eq!(passthrough, &["-c", "true"]);
    }

    #[test]
    fn parse_command_args_rejects_only_dashdash() {
        let only_dashdash: Vec<String> = vec!["--".into()];
        let result = parse_command_args(&only_dashdash);
        assert!(result.is_err());
    }

    #[test]
    fn extract_applicable_count_uses_first_matching_pattern() {
        let text = "running 5 tests\n\
                     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n\
                     cargo test: 5 passed (1 suite, 0.08s)";
        assert_eq!(super::extract_applicable_count(text), Some(5));
    }

    #[test]
    fn extract_applicable_count_uses_libtest_when_no_running() {
        let text = "test result: ok. 3 passed; 0 failed\n";
        assert_eq!(super::extract_applicable_count(text), Some(3));
    }

    #[test]
    fn extract_applicable_count_returns_none_when_nothing_matches() {
        assert_eq!(super::extract_applicable_count("no test output"), None);
    }

    /// Regression: `sum_line_counts` used to return `Some(0)` when no
    /// line matched the parser, which caused a "zero applicable tests"
    /// false positive on output like just "0 failed".
    #[test]
    fn empty_or_no_match_input_returns_none() {
        assert_eq!(sum_line_counts("", libtest_passed_count), None);
        assert_eq!(sum_line_counts("0 failed\n", libtest_passed_count), None);
        assert_eq!(sum_line_counts("0 failed", running_line_count), None);
    }
}
