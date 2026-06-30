use std::{env, path::PathBuf};

use titania_core::TargetProject;
use titania_lanes::{CommandIn, LaneExit};

use super::LocalLane;

pub(super) fn run_local_lane(target: &TargetProject, lane: LocalLane) -> LaneExit {
    let binary = match sibling_binary(lane.binary_name()) {
        Ok(binary) => binary,
        Err(error) => {
            eprintln!("[gauntlet] Failure: {error}");
            return LaneExit::Failure;
        }
    };
    binary_status(target, binary)
}

fn sibling_binary(binary_name: &str) -> Result<PathBuf, String> {
    let current = env::current_exe().map_err(|error| error.to_string())?;
    let Some(dir) = current.parent() else {
        return Err("cannot resolve current Titania lane binary directory".to_owned());
    };
    let binary = dir.join(binary_name);
    if binary.is_file() { Ok(binary) } else { missing_binary(binary_name, dir) }
}

fn missing_binary(binary_name: &str, dir: &std::path::Path) -> Result<PathBuf, String> {
    let shown = dir.display();
    Err(format!(
        "missing Titania lane binary `{binary_name}` beside `{shown}`; build/install titania-lanes lane binaries before running applicable target projects"
    ))
}

fn binary_status(target: &TargetProject, binary: PathBuf) -> LaneExit {
    let Some(program) = binary.to_str() else {
        eprintln!("[gauntlet] Failure: Titania lane binary path is not valid UTF-8");
        return LaneExit::Failure;
    };
    let mut cmd = match CommandIn::new(target, program) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("[gauntlet] Failure: cannot prepare Titania lane binary: {error}");
            return LaneExit::Failure;
        }
    };
    command_status(&mut cmd)
}

pub(super) fn run_clippy_vb_compile(target: &TargetProject) -> LaneExit {
    cargo_status(
        target,
        &["clippy", "-p", "vb_compile", "--lib", "--", "-D", "warnings", "-A", "unsafe_code"],
    )
}

pub(super) fn run_test(target: &TargetProject, group: &str) -> LaneExit {
    let args = vec!["test", "-p", "vb_compile", "--lib", group, "--", "--nocapture"];
    cargo_status(target, &args)
}

pub(super) fn run_kani(target: &TargetProject, harness: &str) -> LaneExit {
    let args = vec!["kani", "--package", "vb_compile", "--harness", harness, "--quiet"];
    cargo_status(target, &args)
}

pub(super) fn run_kani_default_unwind(target: &TargetProject, harness: &str) -> LaneExit {
    let args = vec![
        "kani",
        "--package",
        "vb_runtime",
        "--harness",
        harness,
        "--default-unwind",
        "1",
        "--quiet",
    ];
    cargo_status(target, &args)
}

pub(super) fn cargo_capture(
    target: &TargetProject,
    args: &[&str],
) -> Result<titania_lanes::CommandOutput, String> {
    let mut cmd = CommandIn::new(target, "cargo").map_err(|error| error.to_string())?;
    cmd.inherit_env();
    cmd.env_remove("RUSTC_WRAPPER");
    cmd.env("SCCACHE_DISABLE", "1");
    cmd.args(args);
    cmd.run_capture().map_err(|error| error.to_string())
}

fn cargo_status(target: &TargetProject, args: &[&str]) -> LaneExit {
    let mut cmd = match CommandIn::new(target, "cargo") {
        Ok(command) => command,
        Err(_) => return LaneExit::Violations,
    };
    cmd.args(args);
    command_status(&mut cmd)
}

fn command_status(cmd: &mut CommandIn<'_>) -> LaneExit {
    cmd.inherit_env();
    cmd.env_remove("RUSTC_WRAPPER");
    cmd.env("SCCACHE_DISABLE", "1");
    match cmd.run_status_raw() {
        Ok(status) if status.success() => LaneExit::Clean,
        Ok(_) => LaneExit::Violations,
        Err(error) => {
            eprintln!("[gauntlet] Failure: command execution failed: {error}");
            LaneExit::Failure
        }
    }
}
