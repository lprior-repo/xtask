use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use titania_core::TargetProject;
use titania_lanes::{CommandIn, LaneExit, current_target_project, exit};

/// TLC jar path pinned by `mise` in the bash original.
const TLC_JAR: &str = "/home/lewis/.local/share/mise/http-tarballs/36e4d95a99aa33dde9ff7b288bf3092f3dfbb26e450fc9758ee765cdb250ce38/tla2tools.jar";
const TLA_DIR: &str = "verification/tla";
const SEED: &str = "0";

#[derive(Default)]
struct RunSummary {
    had_runs: bool,
    any_failed: bool,
}

impl RunSummary {
    fn record(mut self, passed: bool) -> Self {
        self.had_runs = true;
        self.any_failed = self.any_failed || !passed;
        self
    }
}

pub(crate) fn main_exit() -> ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(err) => {
            eprintln!("[run-tlc-checks] cannot resolve target project: {err}");
            return exit(LaneExit::Usage);
        }
    };
    run_for_target(&target)
}

fn run_for_target(target: &TargetProject) -> ExitCode {
    let tla_dir = target.as_std_path().join(TLA_DIR);
    if !tla_dir.is_dir() {
        return no_tla_dir_exit(&tla_dir);
    }
    let cfg_files = collect_cfg_files(&tla_dir);
    if cfg_files.is_empty() {
        return no_cfg_exit(&tla_dir);
    }
    summarize_exit(run_cfg_pairs(target, &cfg_files))
}

fn no_tla_dir_exit(tla_dir: &Path) -> ExitCode {
    eprintln!(
        "[run-tlc-checks] no verification/tla directory found at {}; skipped",
        tla_dir.display()
    );
    exit(LaneExit::Clean)
}

fn no_cfg_exit(tla_dir: &Path) -> ExitCode {
    eprintln!("[run-tlc-checks] no .cfg files found in {}; skipped", tla_dir.display());
    exit(LaneExit::Clean)
}

fn summarize_exit(summary: RunSummary) -> ExitCode {
    if !summary.had_runs {
        eprintln!("[run-tlc-checks] no .tla/.cfg pairs found; nothing to check");
        return exit(LaneExit::Clean);
    }
    if summary.any_failed { exit(LaneExit::Violations) } else { exit(LaneExit::Clean) }
}

fn collect_cfg_files(tla_dir: &Path) -> Vec<PathBuf> {
    let entries = match fs::read_dir(tla_dir) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("[run-tlc-checks] cannot read {}: {err}", tla_dir.display());
            return Vec::new();
        }
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|path| is_cfg_file(path.as_path()))
        .collect()
}

fn is_cfg_file(path: &Path) -> bool {
    path.is_file() && path.extension() == Some(OsStr::new("cfg"))
}

fn run_cfg_pairs(target: &TargetProject, cfg_files: &[PathBuf]) -> RunSummary {
    cfg_files
        .iter()
        .filter_map(|cfg| checked_tla_pair(cfg))
        .fold(RunSummary::default(), |summary, (cfg, tla)| run_pair(target, summary, &cfg, &tla))
}

fn checked_tla_pair(cfg: &Path) -> Option<(PathBuf, PathBuf)> {
    let tla = cfg.with_extension("tla");
    if tla.is_file() { Some((cfg.to_path_buf(), tla)) } else { None }
}

fn run_pair(target: &TargetProject, summary: RunSummary, cfg: &Path, tla: &Path) -> RunSummary {
    eprintln!("Checking {}...", tla.display());
    summary.record(run_tlc(target, cfg, tla))
}

fn run_tlc(target: &TargetProject, cfg: &Path, tla: &Path) -> bool {
    let cfg_arg = cfg.display().to_string();
    let tla_arg = tla.display().to_string();
    let mut command = match CommandIn::new(target, "java") {
        Ok(command) => command,
        Err(err) => {
            eprintln!("[run-tlc-checks] failed to prepare java: {err}");
            return false;
        }
    };
    append_tlc_args(&mut command, &cfg_arg, &tla_arg);
    execute_tlc(&command)
}

fn append_tlc_args<'a>(command: &mut CommandIn<'a>, cfg_arg: &'a str, tla_arg: &'a str) {
    command.inherit_env();
    command.arg("-cp").arg(TLC_JAR).arg("tlc2.TLC").arg("-seed");
    command.arg(SEED).arg("-config").arg(cfg_arg).arg(tla_arg);
}

fn execute_tlc(command: &CommandIn<'_>) -> bool {
    match command.run_capture_raw() {
        Ok(out) => {
            print_tlc_tail(&out.stdout, &out.stderr);
            out.status.success()
        }
        Err(err) => {
            eprintln!("[run-tlc-checks] failed to spawn java: {err}");
            false
        }
    }
}

fn print_tlc_tail(stdout: &[u8], stderr: &[u8]) {
    let mut combined = String::from_utf8_lossy(stdout).into_owned();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(stderr));
    tail_lines(&combined, 3).for_each(|line| println!("{line}"));
}

fn tail_lines(text: &str, n: usize) -> impl Iterator<Item = &str> {
    let mut lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines.drain(..start);
    lines.into_iter()
}
