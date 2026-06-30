use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use titania_core::TargetProject;
use titania_lanes::{Finding, LaneExit, LaneReport, current_target_project, exit};

const CLI_SRC: TargetRelativePath = TargetRelativePath::new("crates/vb_cli/src");
const MASTER_DOC: TargetRelativePath = TargetRelativePath::new("velvet-ballistics-MASTER.md");
const RULE_REQUIRED: &str = "AGENT-CLI-REQUIRED-001";
const RULE_REJECTED: &str = "AGENT-CLI-REJECTED-001";

#[derive(Clone, Copy)]
struct TargetRelativePath {
    value: &'static str,
}

impl TargetRelativePath {
    const fn new(value: &'static str) -> Self {
        Self { value }
    }

    const fn as_str(self) -> &'static str {
        self.value
    }

    fn in_target(self, target: &TargetProject) -> PathBuf {
        target.as_std_path().join(self.value)
    }
}

const REQUIRED_LITERALS: &[&str] =
    &["\"agent-context\"", "\"schema_version\"", "\"--json\"", "\"stdout\"", "\"stderr\""];
const REQUIRED_IN_MASTER: &[&str] = &["Agent-First CLI Principles"];
const REJECTED_LITERALS: &[&str] = &[
    "\"info\" =>",
    "\"ls\" =>",
    "named_flag(args, \"--format\")",
    "named_flag(args, \"--output\")",
    "named_flag(args, \"--skip-confirmations\")",
    "named_flag(args, \"--skip-confirmation\")",
];

pub(crate) fn main_exit() -> ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-agent-cli-contract] target discovery failed: {error}");
            return exit(LaneExit::Usage);
        }
    };
    let report = run(&target);
    eprint!("{}", report.render());
    if report.is_clean() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}

pub(crate) fn run(target: &TargetProject) -> LaneReport {
    let mut report = LaneReport::new();
    let cli_root = CLI_SRC.in_target(target);
    if !cli_root.exists() {
        print_not_applicable(target);
        return report;
    }
    let cli_files = collect_files(&cli_root);
    check_required_literals(target, &cli_files, &mut report);
    check_rejected_literals(target, &cli_files, &mut report);
    report
}

fn print_not_applicable(target: &TargetProject) {
    eprintln!(
        "[check-agent-cli-contract] not applicable: {} is absent under {}; skipping vb_cli contract lane",
        CLI_SRC.as_str(),
        target
    );
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_files_into(root, &mut out);
    out
}

fn collect_files_into(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .for_each(|path| record_file_path(path, out));
}

fn record_file_path(path: PathBuf, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        collect_files_into(&path, out);
    } else if path.is_file() {
        out.push(path);
    }
}

fn check_required_literals(target: &TargetProject, files: &[PathBuf], report: &mut LaneReport) {
    REQUIRED_LITERALS.iter().chain(REQUIRED_IN_MASTER.iter()).for_each(|literal| {
        report.record_scan();
        check_required_literal(target, files, literal, report);
    });
}

fn check_required_literal(
    target: &TargetProject,
    files: &[PathBuf],
    literal: &str,
    report: &mut LaneReport,
) {
    if literal == "Agent-First CLI Principles" {
        check_master_literal(target, literal, report);
    } else if !any_file_contains(files, literal) {
        push_required(report, CLI_SRC.as_str(), literal);
    }
}

fn check_master_literal(target: &TargetProject, literal: &str, report: &mut LaneReport) {
    let master_doc = MASTER_DOC.in_target(target);
    if !master_doc.exists() || !file_contains(&master_doc, literal) {
        push_required(report, MASTER_DOC.as_str(), literal);
    }
}

fn push_required(report: &mut LaneReport, path: &str, literal: &str) {
    report.push(Finding::new(
        RULE_REQUIRED,
        path,
        0,
        format!("agent CLI contract missing required literal: {literal}"),
    ));
}

fn check_rejected_literals(target: &TargetProject, files: &[PathBuf], report: &mut LaneReport) {
    REJECTED_LITERALS.iter().for_each(|literal| {
        report.record_scan();
        if let Some((path, line)) = first_match(files, literal) {
            push_rejected(target, report, &path, line, literal);
        }
    });
}

fn push_rejected(
    target: &TargetProject,
    report: &mut LaneReport,
    path: &Path,
    line: u32,
    literal: &str,
) {
    report.push(Finding::new(
        RULE_REJECTED,
        finding_path(target, path),
        line,
        format!("agent CLI contract rejected literal: {literal}"),
    ));
}

fn file_contains(path: &Path, needle: &str) -> bool {
    fs::read_to_string(path).map_or_else(|_| false, |text| text.contains(needle))
}

fn any_file_contains(files: &[PathBuf], needle: &str) -> bool {
    files.iter().any(|path| file_contains(path, needle))
}

fn first_matching_line(path: &Path, needle: &str) -> u32 {
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };
    text.lines()
        .position(|line| line.contains(needle))
        .map_or(0, |idx| u32::try_from(idx).map_or(0, |line| line.saturating_add(1)))
}

fn first_match(files: &[PathBuf], needle: &str) -> Option<(PathBuf, u32)> {
    files
        .iter()
        .find(|path| file_contains(path, needle))
        .map(|path| (path.clone(), first_matching_line(path, needle)))
}

fn finding_path(target: &TargetProject, path: &Path) -> String {
    path.strip_prefix(target.as_std_path())
        .map_or_else(|_| path.display().to_string(), |rel| rel.display().to_string())
}
