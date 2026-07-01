use std::process::ExitCode;

use serde_json::Value;
use titania_core::TargetProject;
use titania_lanes::{CommandIn, LaneExit, current_target_project, exit};

/// Sentinel model name that should never be valid. xtask prints the full
/// "Available models:" listing as a side effect of any unknown --model.
const SENTINEL: &str = "__loom_list_enumerate__";

enum LaneOutcome {
    Models(Vec<String>),
    NotApplicable(String),
}

pub(crate) fn main_exit(args: Vec<String>) -> ExitCode {
    if let Some(code) = usage_exit(&args) {
        return code;
    }
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[loom-list] target discovery failed: {error}");
            return exit(LaneExit::Usage);
        }
    };
    render_lane_result(run_lane(&target))
}

fn usage_exit(args: &[String]) -> Option<ExitCode> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("usage: loom_list\n  enumerates xtask loom models (PO-011)");
        return Some(exit(LaneExit::Clean));
    }
    if !args.is_empty() {
        eprintln!("usage: loom_list\n  no arguments allowed");
        return Some(exit(LaneExit::Usage));
    }
    None
}

fn render_lane_result(result: Result<LaneOutcome, String>) -> ExitCode {
    match result {
        Ok(LaneOutcome::Models(models)) => models_exit(&models),
        Ok(LaneOutcome::NotApplicable(reason)) => not_applicable_exit(&reason),
        Err(err) => violations_exit(&err),
    }
}

fn models_exit(models: &[String]) -> ExitCode {
    eprintln!("[loom-list] Found {} loom models:", models.len());
    models.iter().for_each(|name| println!("{name}"));
    exit(LaneExit::Clean)
}

fn not_applicable_exit(reason: &str) -> ExitCode {
    eprintln!("[loom-list] NotApplicable: {reason}");
    exit(LaneExit::NotApplicable)
}

fn violations_exit(err: &str) -> ExitCode {
    eprintln!("[loom-list] {err}");
    exit(LaneExit::Violations)
}

fn run_lane(target: &TargetProject) -> Result<LaneOutcome, String> {
    if !has_xtask_inventory(target) {
        return Ok(LaneOutcome::NotApplicable(
            "target project has no xtask loom inventory".to_owned(),
        ));
    }
    let output = run_xtask_loom(target)?;
    let combined = combined_output(&output.stdout, &output.stderr);
    classify_loom_output(output.status.success(), &combined)
}

fn has_xtask_inventory(target: &TargetProject) -> bool {
    target.as_std_path().join("xtask/Cargo.toml").is_file()
}

fn run_xtask_loom(target: &TargetProject) -> Result<titania_lanes::CommandOutput, String> {
    let mut command =
        CommandIn::new(target, "cargo").map_err(|e| format!("failed to prepare cargo: {e}"))?;
    command.inherit_env();
    command.arg("xtask").arg("loom").arg("--model").arg(SENTINEL);
    command.run_capture_raw().map_err(|e| format!("failed to spawn cargo xtask loom: {e}"))
}

fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    format!("{stdout}{stderr}")
}

fn classify_loom_output(sentinel_success: bool, combined: &str) -> Result<LaneOutcome, String> {
    if sentinel_success {
        eprintln!("[loom-list] WARNING: xtask exited 0 for sentinel model (unexpected)");
    }
    if combined.contains("no such command: `xtask`") {
        return Ok(LaneOutcome::NotApplicable(
            "cargo xtask command is absent for target project".to_owned(),
        ));
    }
    parse_models(combined)
        .map_or_else(|| unparsed_inventory(combined), |models| Ok(LaneOutcome::Models(models)))
}

fn unparsed_inventory(combined: &str) -> Result<LaneOutcome, String> {
    eprintln!("[loom-list] Raw output:\n{combined}");
    Ok(LaneOutcome::NotApplicable("could not parse model inventory from xtask output".to_owned()))
}

/// Parse the model list. Prefers the JSON array form
/// (`Available models: ["name1", "name2"]`); falls back to indented list.
fn parse_models(text: &str) -> Option<Vec<String>> {
    if let Some(models) = parse_json_array(text) {
        return Some(models);
    }
    parse_indented_list(text)
}

/// Try the JSON-array form: find the first line containing
/// `Available models:` then parse the bracketed substring with `serde_json`.
fn parse_json_array(text: &str) -> Option<Vec<String>> {
    let body = available_models_json_body(text)?;
    let value: Value = serde_json::from_str(body).ok()?;
    let arr = value.as_array()?;
    non_empty_names(arr.iter().filter_map(Value::as_str).map(str::to_owned).collect())
}

fn available_models_json_body(text: &str) -> Option<&str> {
    let line = text.lines().find(|l| l.contains("Available models:"))?;
    let start = line.find('[')?;
    let end = line.rfind(']')?;
    if end <= start { None } else { line.get(start..=end) }
}

/// Fallback parser: walk each indented line, take the first whitespace-delimited
/// token. Rejects obvious prose tokens (`Available`, `Error:`).
fn parse_indented_list(text: &str) -> Option<Vec<String>> {
    non_empty_names(text.lines().filter_map(indented_model_token).collect())
}

fn indented_model_token(line: &str) -> Option<String> {
    if !line.starts_with(char::is_whitespace) {
        return None;
    }
    let token = first_model_token(line.trim_start())?;
    if is_valid_model_token(token) { Some(token.to_owned()) } else { None }
}

fn first_model_token(trimmed: &str) -> Option<&str> {
    let token = trimmed.split(|c: char| c.is_whitespace() || c == '\u{2014}').next()?.trim();
    if token.is_empty() { None } else { Some(token) }
}

fn is_valid_model_token(token: &str) -> bool {
    token != "Available"
        && !token.starts_with("Error:")
        && token.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
}

fn non_empty_names(names: Vec<String>) -> Option<Vec<String>> {
    if names.is_empty() { None } else { Some(names) }
}
