//! cargo fuzz run wrapper for libfuzzer minimization.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/fuzz-minimization.sh`. Run via
//! `cargo run --bin fuzz-minimization -- <target> [extra args...]` from the
//! repository root or via the matching Moon task in `.moon/tasks/all.yml`.
//!
//! ## Behavior parity
//! The bash wrapper exists because `cargo-fuzz`'s TOML schema cannot
//! simultaneously accept `[package.metadata] cargo-fuzz = true` and
//! `[package.metadata.cargo-fuzz] sancov_timeout = 60`. We therefore pass
//! libfuzzer minimization options on the command line:
//!
//! ```text
//! cargo fuzz run <target> \
//!     --target x86_64-unknown-linux-gnu \
//!     -- \
//!     -len_control=1 \
//!     -minimize_contribs=1 \
//!     <extra args...>
//! ```
//!
//! The Rust wrapper spawns `cargo fuzz run` and propagates the child exit
//! code. If `cargo` itself fails to launch (missing binary, etc.) we map
//! the I/O error to `LaneExit::Failure` so the lane surfaces a clear CI
//! error rather than silently exiting 0.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::{fs, io, path::Path};

use titania_lanes::{CommandIn, LaneExit, current_target_project, exit};

/// Boundary-parsed lane input.
enum LaneInput {
    DiscoverDefault,
    Explicit { fuzz_target: String, extra_args: Vec<String> },
}

enum LaneOutcome {
    NotApplicable(String),
    Child(LaneExit),
}

impl LaneOutcome {
    #[must_use]
    fn to_lane_exit(&self) -> LaneExit {
        match self {
            LaneOutcome::NotApplicable(_) => LaneExit::NotApplicable,
            LaneOutcome::Child(code) => *code,
        }
    }
}

fn parse_lane_input(args: Vec<String>) -> LaneInput {
    match args.split_first() {
        Some((target, extra)) if !target.is_empty() => {
            LaneInput::Explicit { fuzz_target: target.clone(), extra_args: extra.to_vec() }
        }
        _ => LaneInput::DiscoverDefault,
    }
}

fn status_to_lane(code: Option<i32>) -> LaneExit {
    match code {
        Some(0) => LaneExit::Clean,
        Some(2) => LaneExit::Usage,
        Some(1) => LaneExit::Violations,
        Some(_) | None => LaneExit::Failure,
    }
}

fn main() -> std::process::ExitCode {
    let input = parse_lane_input(std::env::args().skip(1).collect());
    let target = match current_target_project() {
        Ok(target) => target,
        Err(err) => {
            eprintln!("[fuzz-minimization] cannot resolve target project: {err}");
            return exit(LaneExit::Usage);
        }
    };

    match run_lane(&target, input) {
        Ok(outcome) => {
            let code = outcome.to_lane_exit();
            if let LaneOutcome::NotApplicable(reason) = outcome {
                eprintln!("[fuzz-minimization] NotApplicable: {reason}");
            }
            exit(code)
        }
        Err(err) => {
            eprintln!("[fuzz-minimization] {err}");
            exit(LaneExit::Failure)
        }
    }
}

fn run_lane(target: &titania_core::TargetProject, input: LaneInput) -> Result<LaneOutcome, String> {
    match input {
        LaneInput::DiscoverDefault => {
            if has_fuzz_targets(target)? {
                Ok(LaneOutcome::NotApplicable(
                    "fuzz targets exist, but no target name was provided".to_owned(),
                ))
            } else {
                Ok(LaneOutcome::NotApplicable("target project has no fuzz target".to_owned()))
            }
        }
        LaneInput::Explicit { fuzz_target, extra_args } => {
            if has_fuzz_targets(target)? {
                run_fuzz_target(target, &fuzz_target, &extra_args)
            } else {
                Ok(LaneOutcome::NotApplicable("target project has no fuzz target".to_owned()))
            }
        }
    }
}

fn has_fuzz_targets(target: &titania_core::TargetProject) -> Result<bool, String> {
    let fuzz_dir = target.as_std_path().join("fuzz");
    if !fuzz_dir.join("Cargo.toml").is_file() {
        return Ok(false);
    }
    let targets_dir = fuzz_dir.join("fuzz_targets");
    let entries = match fs::read_dir(&targets_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(format!("failed to read {}: {err}", targets_dir.display()));
        }
    };
    entries
        .map(|entry| entry.map(|entry| is_rust_source(&entry.path())))
        .try_fold(false, |found, entry| entry.map(|is_target| found || is_target))
        .map_err(|err| format!("failed to inspect {}: {err}", targets_dir.display()))
}

fn is_rust_source(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("rs")
}

fn run_fuzz_target(
    target: &titania_core::TargetProject,
    fuzz_target: &str,
    extra_args: &[String],
) -> Result<LaneOutcome, String> {
    let mut command = CommandIn::new(target, "cargo")
        .map_err(|err| format!("failed to prepare cargo fuzz: {err}"))?;
    command.inherit_env();
    command.arg("fuzz").arg("run").arg(fuzz_target);
    command.arg("--target").arg("x86_64-unknown-linux-gnu");
    command.arg("--");
    command.arg("-len_control=1");
    command.arg("-minimize_contribs=1");
    extra_args.iter().for_each(|arg| {
        command.arg(arg.as_str());
    });

    command
        .run_status_raw()
        .map(|status| LaneOutcome::Child(status_to_lane(status.code())))
        .map_err(|io_err| format!("failed to spawn cargo fuzz: {io_err}"))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{LaneInput, LaneOutcome, run_lane};
    use titania_core::TargetProject;
    use titania_lanes::LaneExit;

    #[test]
    fn project_without_fuzz_targets_emits_not_applicable_disposition() {
        let temp = tempfile::tempdir().expect("temporary target project");
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("write manifest");
        let target = target_project(temp.path());

        let outcome = run_lane(&target, LaneInput::DiscoverDefault)
            .expect("lane should classify missing fuzz setup");

        assert!(matches!(outcome, LaneOutcome::NotApplicable(_)));
        assert_eq!(outcome.to_lane_exit(), LaneExit::NotApplicable);
    }

    fn target_project(path: &Path) -> TargetProject {
        TargetProject::try_from_path(path).expect("valid target project")
    }
}
