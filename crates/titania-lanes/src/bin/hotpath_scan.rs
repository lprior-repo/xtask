//! Rejects HashMap/IndexMap/mpsc tokens on hot paths outside allowlist.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/hotpath-scan.sh`. Run via
//! `cargo run --bin hotpath_scan --` from the repository root or via the
//! matching Moon task in `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::{collections::BTreeSet, path::Path};

use titania_core::TargetProject;
use titania_lanes::{
    Finding, LaneExit, LaneReport, current_target_project, exit, helpers::line_no_from_idx,
};

const HOT_ROOTS: &[&str] =
    &["crates/vb_core/src", "crates/vb_runtime/src", "crates/vb_storage/src", "crates/vb_ipc/src"];
const TOKENS: &[&str] = &[
    "HashMap",
    "IndexMap",
    "IndexSet",
    "BTreeMap",
    "std::sync::mpsc",
    "mpsc::channel",
    "channel(",
];
const COLD_TOKENS: &[&str] = &[
    "diagnostic",
    "diagnostics",
    "fixture",
    "fixtures",
    "harness",
    "kani",
    "loom",
    "proof",
    "property",
    "proptest",
    "proptests",
    "support",
    "test",
    "tests",
    "verification",
];
const ALLOW_FILE: &str = "scripts/hotpath-scan.allow";

fn main() -> std::process::ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[hotpath-scan] target discovery failed: {error}");
            return exit(LaneExit::Failure);
        }
    };
    let mut report = LaneReport::new();
    run(&target, &mut report);
    print_and_exit(&report)
}

fn run(target: &TargetProject, report: &mut LaneReport) {
    let root = target.as_std_path();
    let allow = load_allow(root, report);
    HOT_ROOTS.iter().map(|hot| root.join(hot)).filter(|dir| dir.is_dir()).for_each(|dir| {
        scan_dir(&dir, root, &allow, report);
    });
}

fn print_and_exit(report: &LaneReport) -> std::process::ExitCode {
    let rendered = report.render();
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    if report.is_clean() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}

type AllowEntry = (String, String);

struct AllowRow<'a> {
    path: &'a str,
    token: &'a str,
    owner: &'a str,
    reviewer: &'a str,
    test: &'a str,
    reason: &'a str,
}

fn load_allow(root: &Path, report: &mut LaneReport) -> BTreeSet<AllowEntry> {
    let path = root.join(ALLOW_FILE);
    let Some(text) = read_allow_file(&path) else {
        return BTreeSet::new();
    };
    text.lines()
        .enumerate()
        .filter_map(|(idx, raw)| allow_entry_from_line(idx, raw, report))
        .collect()
}

fn read_allow_file(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn allow_entry_from_line(idx: usize, raw: &str, report: &mut LaneReport) -> Option<AllowEntry> {
    let line_no = line_no_from_idx(idx);
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let Some(row) = parse_allow_row(line) else {
        push_allow_finding(report, line_no, "malformed allow row");
        return None;
    };
    if let Some(message) = validate_allow_row(&row) {
        push_allow_finding(report, line_no, message);
        return None;
    }
    Some((row.path.to_string(), row.token.to_string()))
}

fn parse_allow_row(line: &str) -> Option<AllowRow<'_>> {
    let mut parts = line.split('|');
    Some(AllowRow {
        path: parts.next()?,
        token: parts.next()?,
        owner: parts.next()?,
        reviewer: parts.next()?,
        test: parts.next()?,
        reason: parts.next()?,
    })
}

fn validate_allow_row(row: &AllowRow<'_>) -> Option<String> {
    if is_overbroad_allow_path(row.path) {
        return Some("overbroad path".to_string());
    }
    if !TOKENS.contains(&row.token) {
        return Some(format!("unknown token {}", row.token));
    }
    if has_required_allow_metadata(row) { None } else { Some("missing fields".to_string()) }
}

fn is_overbroad_allow_path(path: &str) -> bool {
    path.contains('*') || !path.starts_with("crates/") || !path.ends_with(".rs")
}

fn has_required_allow_metadata(row: &AllowRow<'_>) -> bool {
    row.owner.starts_with("owner=")
        && row.reviewer.starts_with("reviewed_by=")
        && row.test.starts_with("test=")
        && row.reason.starts_with("reason=")
}

fn push_allow_finding(report: &mut LaneReport, line_no: u32, message: impl Into<String>) {
    report.push(Finding::new("ALLOW", format!("{ALLOW_FILE}:{line_no}"), line_no, message));
}

fn is_cold_path(rel: &str) -> bool {
    let normalized = rel.replace(['/', '.', '_', '-'], " ");
    for tok in COLD_TOKENS {
        if normalized.split_whitespace().any(|w| w == *tok) {
            return true;
        }
    }
    false
}

fn scan_dir(dir: &Path, root: &Path, allow: &BTreeSet<(String, String)>, report: &mut LaneReport) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let p = entry.path();
        if p.is_dir() {
            scan_dir(&p, root, allow, report);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            let rel = rel_str(root, &p);
            if is_cold_path(&rel) {
                continue;
            }
            scan_file(&p, &rel, allow, report);
        }
    }
}

fn rel_str(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).map_or_else(
        |_| p.to_string_lossy().into_owned(),
        |rel| rel.to_string_lossy().replace('\\', "/"),
    )
}

fn scan_file(file: &Path, rel: &str, allow: &BTreeSet<(String, String)>, report: &mut LaneReport) {
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    for (idx, raw) in text.lines().enumerate() {
        let line_no = line_no_from_idx(idx);
        let no_comment = strip_comment(raw);
        for token in TOKENS {
            if !no_comment.contains(token) {
                continue;
            }
            if allow.contains(&(rel.to_string(), token.to_string())) {
                continue;
            }
            report.push(Finding::new(
                "HOTPATH",
                rel,
                line_no,
                format!("token {token} on hot path"),
            ));
        }
    }
}

fn strip_comment(line: &str) -> &str {
    line.find("//").and_then(|idx| line.get(..idx)).map_or(line, |prefix| prefix)
}
