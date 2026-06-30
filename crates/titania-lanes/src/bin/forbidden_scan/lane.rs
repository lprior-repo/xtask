use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use crate::source_line::SourceLine;
use titania_lanes::{
    Finding, LaneExit, LaneReport, current_target_project, exit,
    helpers::{line_no_from_idx, relative_path},
};

/// Default forbidden tokens (Holzman Rust slice 1).
const DEFAULT_FORBIDDEN: &[&str] =
    &["panic!", "unwrap()", "expect()", "todo!", "unimplemented!", "dbg!"];

/// Argument used to override the default forbidden set.
const FORBIDDEN_FLAG: &str = "--forbidden=";

pub(crate) fn main_exit(args: Vec<String>) -> ExitCode {
    let forbidden = match parse_forbidden(&args) {
        Ok(set) => set,
        Err(message) => {
            eprintln!("[forbidden-scan] {message}");
            return exit(LaneExit::Usage);
        }
    };
    let root = match target_root() {
        Ok(root) => root,
        Err(code) => return code,
    };
    emit_scan_header(&root, &forbidden);
    scan_and_exit(&root, &forbidden)
}

fn target_root() -> Result<PathBuf, ExitCode> {
    current_target_project().map(|target| target.as_std_path().to_path_buf()).map_err(|error| {
        eprintln!("[forbidden-scan] cannot resolve target project: {error}");
        exit(LaneExit::Usage)
    })
}

fn emit_scan_header(root: &Path, forbidden: &[ForbiddenToken]) {
    eprintln!("CWD: {}", root.display());
    eprintln!("ScanDomain: crates/*/src");
    eprintln!(
        "ForbiddenTokens: {}",
        forbidden.iter().map(ForbiddenToken::as_str).collect::<Vec<_>>().join(",")
    );
}

fn scan_and_exit(root: &Path, forbidden: &[ForbiddenToken]) -> ExitCode {
    let mut report = LaneReport::new();
    collect_source_files(root)
        .iter()
        .for_each(|file| scan_file(root, file, forbidden, &mut report));
    eprint!("{}", report.render());
    if report.is_clean() { clean_exit() } else { violations_exit() }
}

fn clean_exit() -> ExitCode {
    eprintln!("NoViolationFound");
    exit(LaneExit::Clean)
}

fn violations_exit() -> ExitCode {
    eprintln!("ViolationFound: forbidden token surface is non-empty");
    exit(LaneExit::Violations)
}

fn parse_forbidden(args: &[String]) -> Result<Vec<ForbiddenToken>, String> {
    let override_set = args
        .iter()
        .find(|arg| arg.starts_with(FORBIDDEN_FLAG))
        .map(|arg| parse_override_set(arg.as_str()));
    Ok(match override_set {
        Some(set) if !set.is_empty() => set,
        Some(_) | None => default_forbidden_set(),
    })
}

fn parse_override_set(arg: &str) -> Vec<ForbiddenToken> {
    let body = arg.strip_prefix(FORBIDDEN_FLAG).map_or("", core::convert::identity);
    body.split(',').filter_map(ForbiddenToken::parse).collect()
}

fn default_forbidden_set() -> Vec<ForbiddenToken> {
    DEFAULT_FORBIDDEN.iter().filter_map(|s| ForbiddenToken::parse(s)).collect()
}

fn collect_source_files(root: &Path) -> Vec<PathBuf> {
    let crates_dir = root.join("crates");
    let Ok(entries) = std::fs::read_dir(crates_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path().join("src"))
        .filter(|p| p.is_dir())
        .flat_map(walk_rust_files)
        .collect()
}

fn walk_rust_files(dir: PathBuf) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![dir];
    while let Some(top) = stack.pop() {
        append_rust_files(&top, &mut stack, &mut out);
    }
    out.sort();
    out
}

fn append_rust_files(top: &Path, stack: &mut Vec<PathBuf>, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(top) else {
        return;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .for_each(|path| record_path(path, stack, out));
}

fn record_path(path: PathBuf, stack: &mut Vec<PathBuf>, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        stack.push(path);
    } else if path.extension().is_some_and(|e| e == "rs") {
        out.push(path);
    }
}

fn scan_file(root: &Path, path: &Path, forbidden: &[ForbiddenToken], report: &mut LaneReport) {
    report.record_scan();
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let display = relative_path(root, path);
    scan_content(&content, &display, forbidden, report);
}

fn scan_content(
    content: &str,
    display: &str,
    forbidden: &[ForbiddenToken],
    report: &mut LaneReport,
) {
    let mut block_comment = false;
    content.lines().enumerate().for_each(|(idx, line)| {
        let source_line = SourceLine::parse(line, &mut block_comment);
        scan_source_line(&source_line, idx, display, forbidden, report);
    });
}

fn scan_source_line(
    line: &SourceLine,
    idx: usize,
    display: &str,
    forbidden: &[ForbiddenToken],
    report: &mut LaneReport,
) {
    if line.is_non_code() {
        return;
    }
    let line_no = line_no_from_idx(idx);
    forbidden.iter().filter(|token| token.is_present_in(line.code())).for_each(|token| {
        report.push(Finding::new(
            "FORBIDDEN-001",
            display,
            line_no,
            format!("forbidden token `{}`", token.as_str()),
        ));
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ForbiddenToken(String);

impl ForbiddenToken {
    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() { None } else { Some(Self(trimmed.to_owned())) }
    }

    fn as_str(&self) -> &str {
        self.0.as_str()
    }

    fn is_present_in(&self, code: &str) -> bool {
        code.contains(self.as_str())
    }
}
