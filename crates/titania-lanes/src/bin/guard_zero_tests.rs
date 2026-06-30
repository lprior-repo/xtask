//! Fail-closed wrapper: refuses cargo test runs that executed zero applicable tests.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/guard-zero-tests.sh`. Run via
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

use titania_lanes::{CommandIn, CommandOutput, LaneExit, current_target_project, exit};

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

fn parse_lane_input(args: Vec<String>) -> LaneInput {
    let cmd: Vec<String> = match args.iter().position(|arg| arg == "--") {
        Some(separator) => args.into_iter().skip(separator.saturating_add(1)).collect(),
        None => args,
    };
    if cmd.is_empty() { LaneInput::MissingCommand } else { LaneInput::Run(cmd) }
}

fn run_lane(target: &titania_core::TargetProject, cmd_args: &[String]) -> Result<(), String> {
    eprintln!("[guard-zero-tests] running: {}", cmd_args.join(" "));
    let (program, passthrough) = parse_command_args(cmd_args)?;
    let output = run_test_command(target, program, passthrough)?;
    let combined = combine_output(&output.stdout, &output.stderr);
    reject_failed_command(&output, &combined)?;
    report_test_evidence(parse_test_count(&combined)?)
}

fn parse_command_args(cmd_args: &[String]) -> Result<ParsedCommand<'_>, String> {
    cmd_args
        .split_first()
        .map(|(program, passthrough)| (program.as_str(), passthrough))
        .ok_or_else(|| "guard-zero-tests: empty command".to_string())
}

fn run_test_command<'a>(
    target: &'a titania_core::TargetProject,
    program: &'a str,
    passthrough: &'a [String],
) -> Result<CommandOutput, String> {
    let mut command =
        CommandIn::new(target, program).map_err(|e| format!("failed to spawn {program}: {e}"))?;
    command.inherit_env();
    passthrough.iter().for_each(|arg| {
        command.arg(arg.as_str());
    });
    command.run_capture_raw().map_err(|e| format!("failed to spawn {program}: {e}"))
}

fn reject_failed_command(output: &CommandOutput, combined: &str) -> Result<(), String> {
    match output.status.code() {
        Some(0) => Ok(()),
        Some(code) => reject_nonzero_command(code, combined),
        None => reject_signaled_command(combined),
    }
}

fn reject_nonzero_command(code: i32, combined: &str) -> Result<(), String> {
    eprintln!("[guard-zero-tests] cargo test exited {code} — treating as tooling failure");
    if let Some(n) = extract_applicable_count(combined) {
        eprintln!("[guard-zero-tests] applicable test count: {n} (cargo failed with exit {code})");
    }
    eprintln!("{combined}");
    Err(format!("command exited with status {code}"))
}

fn reject_signaled_command(combined: &str) -> Result<(), String> {
    eprintln!("[guard-zero-tests] command terminated by signal");
    eprintln!("{combined}");
    Err("command terminated by signal".to_string())
}

fn parse_test_count(combined: &str) -> Result<u32, String> {
    extract_applicable_count(combined).ok_or_else(|| {
        eprintln!("[guard-zero-tests] FAIL: could not parse test count from cargo output.");
        eprintln!("[guard-zero-tests] Raw output:\n{combined}");
        "could not parse test count from cargo output".to_string()
    })
}

fn report_test_evidence(count: u32) -> Result<(), String> {
    match classify_evidence(count) {
        TestEvidence::Applicable(count) => {
            eprintln!("[guard-zero-tests] PASS: {count} applicable tests executed");
            Ok(())
        }
        TestEvidence::NotApplicable => {
            eprintln!(
                "[guard-zero-tests] FAIL: command completed but executed zero applicable tests"
            );
            Err("zero applicable tests executed".to_string())
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

fn running_line_count(line: &str) -> Option<u32> {
    let trimmed = line.trim_start();
    if !(trimmed.starts_with("running ")
        && (trimmed.contains(" test") || trimmed.starts_with("running 0")))
    {
        return None;
    }
    let after = trimmed.strip_prefix("running ")?;
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Format 2: `test result: ok. 5 passed; 0 failed; ...`.
fn extract_libtest_passed(text: &str) -> Option<u32> {
    sum_line_counts(text, libtest_passed_count)
}

fn libtest_passed_count(line: &str) -> Option<u32> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("test result:") {
        return None;
    }
    let after_passed = trimmed.split("passed").next()?;
    let token = after_passed.split(|c: char| !c.is_ascii_digit()).rev().find(|t| !t.is_empty())?;
    token.parse().ok()
}

/// Format 3: `cargo test: 5 passed (1 suite, 0.08s)`.
fn extract_cargo_test_passed(text: &str) -> Option<u32> {
    sum_line_counts(text, cargo_test_passed_count)
}

fn cargo_test_passed_count(line: &str) -> Option<u32> {
    let trimmed = line.trim_start();
    if !(trimmed.starts_with("cargo test:") && trimmed.contains("passed")) {
        return None;
    }
    let after = trimmed.strip_prefix("cargo test:")?;
    let token = after.split(|c: char| !c.is_ascii_digit()).find(|t| !t.is_empty())?;
    token.parse().ok()
}

/// Format 4 is covered by [`cargo_test_passed_count`].
fn extract_cargo_test_filtered(text: &str) -> Option<u32> {
    sum_line_counts(text, |line| {
        let trimmed = line.trim_start();
        if trimmed.starts_with("cargo test:")
            && trimmed.contains("passed")
            && trimmed.contains("filtered out")
        {
            cargo_test_passed_count(trimmed)
        } else {
            None
        }
    })
}

fn sum_line_counts(text: &str, parse: fn(&str) -> Option<u32>) -> Option<u32> {
    let (seen, total) = text
        .lines()
        .filter_map(parse)
        .fold((false, 0_u32), |(_, total), count| (true, total.saturating_add(count)));
    if seen { Some(total) } else { None }
}
