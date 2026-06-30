//! Enumerates cargo kani harnesses for one or more packages, writes per-pkg JSON.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/kani-list.sh`. Run via
//! `cargo run --bin kani_list -- <package>...` from the repository root or via
//! the matching Moon task in `.moon/tasks/all.yml`.
//!
//! Exit codes: 0 = clean, 1 = violations, 2 = usage, 3 = upstream failure.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use serde_json::Value;
use titania_core::TargetProject;

use titania_lanes::{CommandIn, Finding, LaneExit, LaneReport, current_target_project, exit};

/// Usage blurb emitted on `--help`.
const USAGE: &str = "usage: kani_list [<package> ...]\n\
     no package args: write target-workspace kani-list JSON to KANI_LIST_DIR/workspace.json\n\
     package args: validate package names before writing per-package scoped kani-list JSON\n\
     set KANI_FEATURES=feature1,feature2 to activate package features";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("{USAGE}");
        return exit(LaneExit::Clean);
    }

    let mut report = LaneReport::new();
    let input = parse_lane_input(args);
    match run_lane(&input) {
        Ok(()) => exit(LaneExit::Clean),
        Err(LaneError::Usage(msg)) => {
            eprintln!("[kani_list] {msg}");
            exit(LaneExit::Usage)
        }
        Err(LaneError::Failure(msg)) => {
            let finding = Finding::new("KANI-LIST-001", "<lane>", 0, msg.clone());
            eprintln!("[kani_list] FAIL: {msg}");
            report.push(finding);
            exit(LaneExit::Failure)
        }
        Err(LaneError::Violation(rule, path, line, msg)) => {
            let finding = Finding::new(rule, path, line, msg.clone());
            eprintln!("[kani_list] {msg}");
            report.push(finding);
            exit(LaneExit::Violations)
        }
    }
}

/// Lane-local error taxonomy.
enum LaneError {
    Usage(String),
    Failure(String),
    Violation(&'static str, String, u32, String),
}

/// Boundary-parsed lane input.
enum LaneInput {
    Workspace,
    Packages(Vec<String>),
}

impl From<io::Error> for LaneError {
    fn from(err: io::Error) -> Self {
        LaneError::Failure(format!("io error: {err}"))
    }
}

fn parse_lane_input(args: Vec<String>) -> LaneInput {
    let packages: Vec<String> = args.into_iter().filter(|a| !a.is_empty()).collect();
    if packages.is_empty() { LaneInput::Workspace } else { LaneInput::Packages(packages) }
}

fn run_lane(input: &LaneInput) -> Result<(), LaneError> {
    let target = current_target_project()
        .map_err(|e| LaneError::Usage(format!("target discovery failed: {e}")))?;
    let output_dir = output_dir(&target)?;
    fs::create_dir_all(&output_dir)?;

    match input {
        LaneInput::Workspace => run_workspace_list(&target, &output_dir),
        LaneInput::Packages(packages) => run_package_lists(&target, &output_dir, packages),
    }
}

fn run_workspace_list(target: &TargetProject, output_dir: &Path) -> Result<(), LaneError> {
    let target_file = output_dir.join("workspace.json");
    let produced = target.as_std_path().join("kani-list.json");
    remove_if_present(&produced)?;

    eprintln!("[kani-list] scope=workspace output={}", target_file.display());
    let kani_status = run_kani_list(target, None)?;
    if !kani_status.success() {
        return Err(LaneError::Violation(
            "KANI-LIST-EXEC",
            target.as_std_path().display().to_string(),
            0,
            format!("cargo kani list failed (exit {:?})", kani_status.code()),
        ));
    }
    validate_produced_json(&produced)?;
    fs::rename(&produced, &target_file)?;
    eprintln!("KANI_LIST_OK output_dir={} scope=workspace", output_dir.display());
    Ok(())
}

fn run_package_lists(
    target: &TargetProject,
    output_dir: &Path,
    packages: &[String],
) -> Result<(), LaneError> {
    let metadata_text = run_cargo_metadata(target)?;
    let metadata: Value = serde_json::from_str(&metadata_text)
        .map_err(|e| LaneError::Failure(format!("cargo metadata parse: {e}")))?;

    packages.iter().try_for_each(|package| {
        let manifest = find_manifest(&metadata, package)?;
        let package_dir = manifest_dir(&manifest);
        let target_file = output_dir.join(format!("{package}.json"));
        let produced = target.as_std_path().join("kani-list.json");
        remove_if_present(&produced)?;

        eprintln!(
            "[kani-list] package={package} dir={} output={}",
            package_dir.display(),
            target_file.display()
        );
        let kani_status = run_kani_list(target, Some(&manifest))?;
        if !kani_status.success() {
            return Err(LaneError::Violation(
                "KANI-LIST-EXEC",
                package_dir.display().to_string(),
                0,
                format!("cargo kani list failed (exit {:?})", kani_status.code()),
            ));
        }
        validate_produced_json(&produced)?;
        fs::rename(&produced, &target_file)?;
        eprintln!("[kani-list] wrote {}", target_file.display());
        Ok(())
    })?;

    eprintln!("KANI_LIST_OK output_dir={} packages={}", output_dir.display(), packages.join(","));
    Ok(())
}

fn output_dir(target: &TargetProject) -> Result<PathBuf, LaneError> {
    let raw = match env::var_os("KANI_LIST_DIR") {
        Some(s) if !s.is_empty() => PathBuf::from(s),
        _ => PathBuf::from(".evidence/kani-list"),
    };
    Ok(target_root_path(target, raw))
}

fn run_cargo_metadata(target: &TargetProject) -> Result<String, LaneError> {
    let manifest = target.manifest_path();
    let mut command = command_in(target, "cargo")?;
    command
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(manifest.as_str());
    let output = command
        .run_capture_raw()
        .map_err(|e| LaneError::Failure(format!("failed to spawn cargo metadata: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LaneError::Failure(format!("cargo metadata failed: {stderr}")));
    }
    String::from_utf8(output.stdout)
        .map_err(|e| LaneError::Failure(format!("cargo metadata non-UTF8: {e}")))
}

fn find_manifest(metadata: &Value, package: &str) -> Result<PathBuf, LaneError> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| LaneError::Failure("cargo metadata: missing 'packages'".to_string()))?;
    let matches: Vec<&Value> = packages
        .iter()
        .filter(|p| p.get("name").and_then(Value::as_str) == Some(package))
        .collect();
    match matches.len() {
        0 => Err(LaneError::Failure(format!("package '{package}' not found in workspace"))),
        1 => {
            let manifest = matches
                .first()
                .and_then(|v| v.get("manifest_path"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    LaneError::Failure(format!("package '{package}' has no manifest_path"))
                })?;
            Ok(PathBuf::from(manifest))
        }
        n => Err(LaneError::Failure(format!(
            "expected exactly one package named '{package}', found {n}"
        ))),
    }
}

fn manifest_dir(manifest: &Path) -> PathBuf {
    match manifest.parent() {
        Some(parent) => parent.to_path_buf(),
        None => PathBuf::from("."),
    }
}

fn run_kani_list(
    target: &TargetProject,
    manifest: Option<&Path>,
) -> Result<std::process::ExitStatus, LaneError> {
    let features = env::var_os("KANI_FEATURES").map(|value| value.to_string_lossy().into_owned());
    let mut command = command_in(target, "cargo")?;
    command.arg("kani").arg("list").arg("--format").arg("json");
    if let Some(manifest) = manifest {
        let manifest = manifest.to_str().ok_or_else(|| {
            LaneError::Failure(format!("package manifest is not UTF-8: {}", manifest.display()))
        })?;
        command.arg("--manifest-path").arg(manifest);
    }
    if let Some(features) = features.as_deref().filter(|value| !value.is_empty()) {
        command.arg("--features").arg(features);
    }
    command
        .run_status_raw()
        .map_err(|e| LaneError::Failure(format!("failed to spawn cargo kani: {e}")))
}

fn target_root_path(target: &TargetProject, path: PathBuf) -> PathBuf {
    if path.is_absolute() { path } else { target.as_std_path().join(path) }
}

fn command_in<'a>(target: &'a TargetProject, program: &'a str) -> Result<CommandIn<'a>, LaneError> {
    let mut command = CommandIn::new(target, program)
        .map_err(|e| LaneError::Failure(format!("failed to prepare {program}: {e}")))?;
    command.inherit_env();
    Ok(command)
}

fn validate_produced_json(produced: &Path) -> Result<(), LaneError> {
    if !is_non_empty(produced) {
        return Err(LaneError::Violation(
            "KANI-LIST-MISSING",
            produced.display().to_string(),
            0,
            format!("cargo kani list did not produce {}", produced.display()),
        ));
    }

    let raw = fs::read_to_string(produced)?;
    validate_json(&raw).map_err(|e| {
        LaneError::Violation(
            "KANI-LIST-INVALID-JSON",
            produced.display().to_string(),
            0,
            format!("invalid JSON in {}: {e}", produced.display()),
        )
    })
}

fn remove_if_present(path: &Path) -> Result<(), LaneError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(LaneError::Failure(format!("failed to remove {}: {err}", path.display()))),
    }
}

fn is_non_empty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.len() > 0)
}

fn validate_json(raw: &str) -> Result<(), String> {
    serde_json::from_str::<Value>(raw).map(|_| ()).map_err(|e| e.to_string())
}
