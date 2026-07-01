//! Run Cargo-native lanes inside the target project discovered from CWD.

use std::{env, process::ExitCode};

use serde_json::Value;
use titania_core::{TargetProject, TargetProjectError, discover_target};
use titania_lanes::{CommandIn, Finding, LaneError, LaneExit, LaneReport, exit};

const RULE_FMT: &str = "CARGO-FMT-001";
const RULE_COMPILE: &str = "CARGO-COMPILE-001";
const RULE_CLIPPY: &str = "CARGO-CLIPPY-001";
const RULE_TEST: &str = "CARGO-TEST-001";
const RULE_BUILD: &str = "CARGO-BUILD-001";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CargoLane {
    Fmt,
    Compile,
    Clippy,
    Test,
    Build,
}

impl CargoLane {
    fn parse(raw: &str) -> Result<Self, String> {
        if raw.trim() != raw {
            return Err(usage_message());
        }
        match raw {
            "fmt" => Ok(Self::Fmt),
            "compile" => Ok(Self::Compile),
            "clippy" => Ok(Self::Clippy),
            "test" => Ok(Self::Test),
            "build" => Ok(Self::Build),
            _other => Err(usage_message()),
        }
    }

    fn rule(self) -> &'static str {
        match self {
            Self::Fmt => RULE_FMT,
            Self::Compile => RULE_COMPILE,
            Self::Clippy => RULE_CLIPPY,
            Self::Test => RULE_TEST,
            Self::Build => RULE_BUILD,
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::Fmt => "cargo fmt",
            Self::Compile => "cargo check",
            Self::Clippy => "cargo clippy",
            Self::Test => "cargo test",
            Self::Build => "cargo build",
        }
    }
}

fn main() -> ExitCode {
    exit(run(env::args().collect()))
}

fn run(args: Vec<String>) -> LaneExit {
    match run_checked(args) {
        Ok(report) => {
            eprint!("{}", report.render());
            if report.is_clean() { LaneExit::Clean } else { LaneExit::Violations }
        }
        Err(RunCargoError::Usage(message)) => {
            eprintln!("{message}");
            LaneExit::Usage
        }
        Err(RunCargoError::Target(error)) => {
            eprintln!("target discovery failed: {error}");
            LaneExit::Usage
        }
        Err(RunCargoError::Command(error)) => {
            eprintln!("cargo execution failed: {error}");
            LaneExit::Failure
        }
        Err(RunCargoError::CurrentDir(error)) => {
            eprintln!("cannot read current directory: {error}");
            LaneExit::Failure
        }
    }
}

fn run_checked(args: Vec<String>) -> Result<LaneReport, RunCargoError> {
    let mut rest = args.into_iter();
    let _program = rest.next();
    let subcommand = rest.next().ok_or_else(|| RunCargoError::Usage(usage_message()))?;
    let lane = CargoLane::parse(&subcommand).map_err(RunCargoError::Usage)?;
    let extra_args: Vec<String> = rest.collect();
    let cwd = env::current_dir().map_err(RunCargoError::CurrentDir)?;
    let target = discover_target(&cwd).map_err(RunCargoError::Target)?;
    run_lane(&target, lane, &extra_args).map_err(RunCargoError::Command)
}

fn run_lane(
    target: &TargetProject,
    lane: CargoLane,
    extra_args: &[String],
) -> Result<LaneReport, LaneError> {
    let mut report = LaneReport::new();
    report.record_scan();
    let output = cargo_output(target, lane, extra_args)?;
    let stdout = output.stdout_str()?;
    let stderr = output.stderr_str()?;
    collect_findings(lane, stdout, stderr, &mut report);
    if output.success() && report.is_clean() {
        report.record_pass();
    }
    if !output.success() && report.is_clean() {
        report.push(Finding::new(lane.rule(), lane.path(), 0, fallback_message(stdout, stderr)));
    }
    Ok(report)
}

fn cargo_output(
    target: &TargetProject,
    lane: CargoLane,
    extra_args: &[String],
) -> Result<titania_lanes::CommandOutput, LaneError> {
    let mut command = CommandIn::new(target, "cargo")?;
    command.inherit_env();
    match lane {
        CargoLane::Fmt => {
            command.arg("fmt").arg("--check");
        }
        CargoLane::Compile => {
            command.arg("check").arg("--workspace").arg("--all-targets").arg("--frozen");
        }
        CargoLane::Clippy => {
            command
                .arg("clippy")
                .arg("--workspace")
                .arg("--lib")
                .arg("--bins")
                .arg("--examples")
                .arg("--frozen")
                .arg("--message-format=json")
                .arg("--")
                .arg("-D")
                .arg("warnings")
                .arg("-W")
                .arg("clippy::all");
        }
        CargoLane::Test => {
            command
                .arg("test")
                .arg("--workspace")
                .arg("--all-features")
                .arg("--frozen")
                .arg("--")
                .arg("--test-threads=1");
        }
        CargoLane::Build => {
            command.arg("build").arg("--workspace").arg("--release").arg("--frozen");
        }
    }
    append_extra_args(&mut command, extra_args);
    command.run_capture_raw()
}

fn append_extra_args<'a>(command: &mut CommandIn<'a>, extra_args: &'a [String]) {
    for arg in extra_args {
        command.arg(arg.as_str());
    }
}

fn collect_findings(lane: CargoLane, stdout: &str, stderr: &str, report: &mut LaneReport) {
    match lane {
        CargoLane::Fmt => collect_fmt_findings(stdout, stderr, report),
        CargoLane::Compile => collect_error_lines(RULE_COMPILE, lane.path(), stderr, report),
        CargoLane::Clippy => collect_clippy_findings(stdout, stderr, report),
        CargoLane::Test => collect_test_findings(stdout, stderr, report),
        CargoLane::Build => collect_error_lines(RULE_BUILD, lane.path(), stderr, report),
    }
}

fn collect_fmt_findings(stdout: &str, stderr: &str, report: &mut LaneReport) {
    let mut path = "cargo fmt";
    let mut saw_diff_header = false;
    for line in stdout.lines().chain(stderr.lines()) {
        if let Some(rest) = line.strip_prefix("Diff in ") {
            path = rest.strip_suffix(':').unwrap_or(rest);
            saw_diff_header = true;
            report.push(Finding::new(RULE_FMT, path, 0, "rustfmt diff hunk"));
        } else if line.starts_with("@@") && !saw_diff_header {
            report.push(Finding::new(RULE_FMT, path, 0, "rustfmt diff hunk"));
        }
    }
}

fn collect_clippy_findings(stdout: &str, stderr: &str, report: &mut LaneReport) {
    for line in stdout.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("reason").and_then(Value::as_str) != Some("compiler-message") {
            continue;
        }
        let Some(message) = value.get("message") else {
            continue;
        };
        let level = message.get("level").and_then(Value::as_str);
        if level != Some("warning") && level != Some("error") {
            continue;
        }
        let text = message_text(message);
        let (path, line_no) = message_location(message);
        report.push(Finding::new(RULE_CLIPPY, path, line_no, text));
    }
    if report.is_clean() {
        collect_error_lines(RULE_CLIPPY, "cargo clippy", stderr, report);
    }
}

fn collect_test_findings(stdout: &str, stderr: &str, report: &mut LaneReport) {
    for line in stdout.lines().chain(stderr.lines()) {
        if let Some(name) = failed_test_name(line) {
            report.push(Finding::new(RULE_TEST, "cargo test", 0, format!("test failed: {name}")));
        }
    }
}

fn collect_error_lines(rule: &'static str, path: &str, text: &str, report: &mut LaneReport) {
    for line in text.lines() {
        if line.starts_with("error[") || line.starts_with("error:") {
            report.push(Finding::new(rule, path, 0, line.to_owned()));
        }
    }
}

fn failed_test_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("test ")?;
    rest.strip_suffix(" ... FAILED")
}

fn message_text(message: &Value) -> String {
    message
        .get("rendered")
        .and_then(Value::as_str)
        .or_else(|| message.get("message").and_then(Value::as_str))
        .unwrap_or("cargo clippy diagnostic")
        .trim()
        .to_owned()
}

fn message_location(message: &Value) -> (String, u32) {
    message
        .get("spans")
        .and_then(Value::as_array)
        .and_then(|spans| {
            spans.iter().find(|span| span.get("is_primary") == Some(&Value::Bool(true)))
        })
        .map_or_else(
            || (String::from("cargo clippy"), 0),
            |span| {
                let path = span
                    .get("file_name")
                    .and_then(Value::as_str)
                    .unwrap_or("cargo clippy")
                    .to_owned();
                let line_no = span
                    .get("line_start")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0);
                (path, line_no)
            },
        )
}

fn fallback_message(stdout: &str, stderr: &str) -> String {
    stderr
        .lines()
        .chain(stdout.lines())
        .find(|line| !line.trim().is_empty())
        .unwrap_or("cargo command failed without output")
        .to_owned()
}

fn usage_message() -> String {
    String::from("usage: run-cargo <fmt|compile|clippy|test|build>")
}

#[derive(Debug)]
enum RunCargoError {
    Usage(String),
    Target(TargetProjectError),
    Command(LaneError),
    CurrentDir(std::io::Error),
}
