use std::process::ExitCode;

use titania_core::TargetProject;
use titania_lanes::{CommandIn, LaneError, LaneExit, current_target_project, exit};

/// Sentinel model name that should never be valid. xtask prints the full
/// "Available models:" listing as a side effect of any unknown --model.
const SENTINEL: &str = "__loom_list_enumerate__";

#[derive(Debug, Clone, PartialEq, Eq)]
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
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        eprintln!("usage: loom_list");
        return Some(exit(LaneExit::Usage));
    }
    None
}

fn render_lane_result(result: Result<LaneOutcome, LaneError>) -> ExitCode {
    match result {
        Ok(LaneOutcome::Models(models)) => models_exit(&models),
        Ok(LaneOutcome::NotApplicable(reason)) => not_applicable_exit(&reason),
        Err(err) => {
            eprintln!("[loom-list] {err}");
            exit(LaneExit::Violations)
        }
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

/// Run the full lane: skip when there's no xtask, otherwise drive
/// `cargo xtask loom --model <SENTINEL>` and classify the output.
fn run_lane(target: &TargetProject) -> Result<LaneOutcome, LaneError> {
    if !has_xtask_inventory(target) {
        return Ok(LaneOutcome::NotApplicable(
            "target project has no xtask loom inventory".to_owned(),
        ));
    }
    let output = run_xtask_loom(target)?;
    let combined = combined_output(&output.stdout, &output.stderr);
    Ok(classify_loom_output(output.status.success(), &combined))
}

fn has_xtask_inventory(target: &TargetProject) -> bool {
    target.as_std_path().join("xtask/Cargo.toml").is_file()
}

fn run_xtask_loom(target: &TargetProject) -> Result<titania_lanes::CommandOutput, LaneError> {
    let mut command = CommandIn::new(target, "cargo")?;
    command.inherit_env();
    command.arg("xtask").arg("loom").arg("--model").arg(SENTINEL);
    command.run_capture_raw()
}

fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    format!("{stdout}{stderr}")
}

/// Classify the xtask output. Returns `LaneOutcome` (no `Result`) — every
/// code path here either emits `NotApplicable` with a reason or `Models`
/// with the discovered list; we never produce a hard `Err` from this
/// function (subprocess failure is caught one level up in `run_xtask_loom`).
fn classify_loom_output(sentinel_success: bool, combined: &str) -> LaneOutcome {
    if sentinel_success {
        eprintln!("[loom-list] WARNING: xtask exited 0 for sentinel model (unexpected)");
    }
    if combined.contains("no such command: `xtask`") {
        return LaneOutcome::NotApplicable(
            "cargo xtask command is absent for target project".to_owned(),
        );
    }
    parse_models(combined).map_or_else(|| unparsed_inventory(combined), LaneOutcome::Models)
}

fn unparsed_inventory(combined: &str) -> LaneOutcome {
    eprintln!("[loom-list] Raw output:\n{combined}");
    LaneOutcome::NotApplicable("could not parse model inventory from xtask output".to_owned())
}

/// Parse the model list. Prefers the JSON array form
/// (`Available models: ["name1", "name2"]`); falls back to indented list.
fn parse_models(text: &str) -> Option<Vec<String>> {
    parse_json_array(text).or_else(|| {
        let names: Vec<String> = text.lines().filter_map(indented_model_token).collect();
        non_empty_names(names)
    })
}

/// Try the JSON-array form: find the first line containing
/// `Available models:` then parse the bracketed substring with `serde_json`.
fn parse_json_array(text: &str) -> Option<Vec<String>> {
    let body = available_models_json_body(text)?;
    let names: Vec<String> = serde_json::from_str::<Vec<String>>(body)
        .ok()?
        .into_iter()
        .filter(|name| !name.is_empty())
        .collect();
    non_empty_names(names)
}

fn available_models_json_body(text: &str) -> Option<&str> {
    let marker = "Available models:";
    let line = text.lines().find(|line| line.contains(marker))?;
    let start = line.find('[')?;
    let end = line.rfind(']')?;
    if end <= start { None } else { line.get(start..=end.saturating_add(1)) }
}

fn indented_model_token(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("    ") && !trimmed.starts_with('\t') {
        return None;
    }
    first_model_token(trimmed).map(str::to_owned)
}

fn first_model_token(trimmed: &str) -> Option<&str> {
    let token = trimmed.split_whitespace().next()?;
    is_valid_model_token(token).then_some(token)
}

fn is_valid_model_token(token: &str) -> bool {
    !token.is_empty()
        && !token.chars().any(|c| c == ':' || c == ',' || c == '`')
        && !matches!(token, "Available" | "Error")
}

fn non_empty_names(names: Vec<String>) -> Option<Vec<String>> {
    if names.is_empty() { None } else { Some(names) }
}
