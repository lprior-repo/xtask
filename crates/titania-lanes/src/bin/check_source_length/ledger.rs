use std::{collections::HashSet, path::Path};

use titania_lanes::{Finding, LaneReport, helpers::line_no_from_idx};

use crate::{
    LEDGER_PATH,
    paths::{is_excluded_source_path, tracked_set},
};

pub(crate) fn load_ledger(root: &Path, report: &mut LaneReport) -> Vec<String> {
    let path = root.join(LEDGER_PATH);
    if !path.is_file() {
        eprintln!("Info: source-length exceptions ledger absent; using empty exceptions");
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("Info: source-length exceptions ledger unreadable; using empty exceptions");
        return Vec::new();
    };
    let tracked = tracked_set(root);
    parse_ledger(&text, &tracked, report)
}

fn parse_ledger(text: &str, tracked: &HashSet<String>, report: &mut LaneReport) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    text.lines().enumerate().for_each(|(idx, raw)| {
        let line_no = line_no_from_idx(idx);
        if let Some(file) = parse_ledger_line(raw, line_no, tracked, &entries, report) {
            entries.push(file);
        }
    });
    entries
}

fn parse_ledger_line(
    raw: &str,
    line_no: u32,
    tracked: &HashSet<String>,
    entries: &[String],
    report: &mut LaneReport,
) -> Option<String> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let parts: Vec<&str> = line.split('|').collect();
    let file = ledger_file(&parts, line_no, report)?;
    validate_ledger_file(file, line_no, tracked, entries, report)?;
    Some(file.to_string())
}

fn ledger_file<'a>(parts: &'a [&str], line_no: u32, report: &mut LaneReport) -> Option<&'a str> {
    if parts.len() == 5 {
        return parts.first().copied();
    }
    report.push(Finding::new(
        "SRC-LEN-LEDGER",
        format!("{LEDGER_PATH}:{line_no}"),
        line_no,
        "malformed row; expected <file>|<owner>|<split_bead>|<removal_plan>|<reason>",
    ));
    None
}

fn validate_ledger_file(
    file: &str,
    line_no: u32,
    tracked: &HashSet<String>,
    entries: &[String],
    report: &mut LaneReport,
) -> Option<()> {
    if let Some(message) = ledger_file_error(file, tracked, entries) {
        report.push(Finding::new("SRC-LEN-LEDGER", ledger_ref(line_no), line_no, message));
        return None;
    }
    Some(())
}

fn ledger_file_error(file: &str, tracked: &HashSet<String>, entries: &[String]) -> Option<String> {
    if invalid_relative_path(file) {
        return Some("invalid path; use a normalized repository-relative path".to_string());
    }
    rust_file_error(file).or_else(|| tracked_error(file, tracked, entries))
}

fn invalid_relative_path(file: &str) -> bool {
    file.starts_with('/') || file.starts_with("../") || file.contains("/../")
}

fn rust_file_error(file: &str) -> Option<String> {
    if !file.ends_with(".rs") {
        return Some(format!("path is not a Rust source file: {file}"));
    }
    if is_excluded_source_path(file) {
        return Some(format!("path is excluded from first-party source-length checks: {file}"));
    }
    None
}

fn tracked_error(file: &str, tracked: &HashSet<String>, entries: &[String]) -> Option<String> {
    if !tracked.contains(file) {
        return Some(format!("path is not a tracked first-party Rust source file: {file}"));
    }
    if entries.iter().any(|known| known == file) {
        return Some(format!("duplicate exception for {file}"));
    }
    None
}

fn ledger_ref(line_no: u32) -> String {
    format!("{LEDGER_PATH}:{line_no}")
}
