use std::{env, path::PathBuf, process::ExitCode};

use titania_core::TargetProject;
use titania_lanes::{CommandIn, Finding, LaneExit, LaneReport, current_target_project, exit};

const RULE_REJECTED: &str = "FLUX-REJECTED-001";
const RULE_USAGE: &str = "FLUX-USAGE-001";
const RULE_FLUX_MISSING: &str = "FLUX-MISSING-001";

const REJECTED_SELECTORS: &[&str] = &["--lib", "--test", "--tests", "--benches", "--all-targets"];

struct Invocation {
    package: String,
    forwarded: Vec<String>,
}

pub(crate) fn main_exit(args: Vec<String>) -> ExitCode {
    if help_requested(&args) {
        return print_help();
    }
    let invocation = match parse_invocation(&args) {
        Ok(invocation) => invocation,
        Err(code) => return code,
    };
    let target = match discover_target_project() {
        Ok(target) => target,
        Err(code) => return code,
    };
    run_flux(&target, &invocation)
}

fn help_requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--help" || arg == "-h")
}

fn print_help() -> ExitCode {
    eprintln!(
        "usage: flux-check-package <package> [cargo-flux options]\n\
         Rejects --lib, --test, --tests, --benches, --all-targets and\n\
         shells out to `cargo flux -p <package> --message-format human`."
    );
    exit(LaneExit::Usage)
}

fn parse_invocation(args: &[String]) -> Result<Invocation, ExitCode> {
    let Some((package, forwarded)) = args.split_first() else {
        return Err(usage_exit("usage: flux-check-package <package> [cargo-flux options]"));
    };
    reject_unsupported_selectors(forwarded)?;
    Ok(Invocation { package: package.clone(), forwarded: forwarded.to_vec() })
}

fn reject_unsupported_selectors(args: &[String]) -> Result<(), ExitCode> {
    let mut report = LaneReport::new();
    args.iter().filter(|arg| is_rejected_selector(arg)).for_each(|arg| {
        report.push(Finding::new(
            RULE_REJECTED,
            "argv",
            0,
            format!("unsupported cargo-flux target selector for installed cargo-flux: {arg}"),
        ));
    });
    if report.is_clean() { Ok(()) } else { Err(render_exit(report, LaneExit::Usage)) }
}

fn is_rejected_selector(arg: &str) -> bool {
    REJECTED_SELECTORS.contains(&arg)
}

fn discover_target_project() -> Result<TargetProject, ExitCode> {
    current_target_project().map_err(|error| {
        let mut report = LaneReport::new();
        report.push(Finding::new(
            RULE_USAGE,
            "target",
            0,
            format!("target discovery failed: {error}"),
        ));
        render_exit(report, LaneExit::Usage)
    })
}

fn usage_exit(message: &str) -> ExitCode {
    let mut report = LaneReport::new();
    report.push(Finding::new(RULE_USAGE, "argv", 0, message));
    render_exit(report, LaneExit::Usage)
}

fn run_flux(target: &TargetProject, invocation: &Invocation) -> ExitCode {
    let cargo_args = build_cargo_args(invocation);
    let path = rustup_first_path();
    let mut command = match prepare_command(target, path.as_deref()) {
        Ok(command) => command,
        Err(error) => return cargo_missing_exit(error),
    };
    append_args(&mut command, &cargo_args);
    run_command(command)
}

fn build_cargo_args(invocation: &Invocation) -> Vec<String> {
    let mut args = vec![
        "flux".to_owned(),
        "-p".to_owned(),
        invocation.package.clone(),
        "--message-format".to_owned(),
        "human".to_owned(),
    ];
    args.extend(invocation.forwarded.iter().cloned());
    args
}

fn prepare_command<'a>(
    target: &'a TargetProject,
    path: Option<&'a str>,
) -> Result<CommandIn<'a>, String> {
    let mut command = CommandIn::new(target, "cargo").map_err(|error| error.to_string())?;
    command.inherit_env();
    if let Some(path) = path {
        command.env("PATH", path);
    }
    Ok(command)
}

fn run_command(command: CommandIn<'_>) -> ExitCode {
    match command.run_status_raw() {
        Ok(status) => exit(match status.code() {
            Some(0) => LaneExit::Clean,
            Some(1) => LaneExit::Violations,
            Some(2) => LaneExit::Usage,
            Some(_) | None => LaneExit::Failure,
        }),
        Err(error) => cargo_missing_exit(error.to_string()),
    }
}

fn cargo_missing_exit(error: String) -> ExitCode {
    let mut report = LaneReport::new();
    report.push(Finding::new(
        RULE_FLUX_MISSING,
        "cargo",
        0,
        format!("cargo flux failed to start: {error}"),
    ));
    render_exit(report, LaneExit::Failure)
}

fn render_exit(report: LaneReport, code: LaneExit) -> ExitCode {
    eprint!("{}", report.render());
    exit(code)
}

fn append_args<'a>(command: &mut CommandIn<'a>, args: &'a [String]) {
    args.iter().for_each(|arg| {
        command.arg(arg.as_str());
    });
}

fn rustup_first_path() -> Option<String> {
    let current = env::var_os("PATH")?;
    let home = env::var_os("HOME")?;
    let cargo_bin = PathBuf::from(home).join(".cargo").join("bin");
    if !cargo_bin.is_dir() {
        return current.into_string().ok();
    }
    let paths = std::iter::once(cargo_bin.clone())
        .chain(env::split_paths(&current).filter(move |path| path != &cargo_bin));
    env::join_paths(paths).ok().and_then(|path| path.into_string().ok())
}
