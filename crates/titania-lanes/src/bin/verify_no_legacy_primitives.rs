//! Rejects 'parallel' / 'aggregate' inside STEP_PRIMITIVES + ALLOWED_STEP_FIELDS constants.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/verify-no-legacy-primitives.sh`. Run via
//! `cargo run --bin verify-no-legacy-primitives --` from the repository root or via the
//! matching Moon task in `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]
#![allow(clippy::let_underscore_must_use)]

use std::{fs, io::ErrorKind, path::PathBuf};

use titania_core::TargetProject;
use titania_lanes::{Finding, LaneExit, LaneReport, current_target_project, exit};

const FORBIDDEN: &[&str] = &["\"parallel\"", "\"aggregate\""];
const SOURCES: &[TargetRelativePath] = &[
    TargetRelativePath::new("crates/vb_validate/src/schema.rs"),
    TargetRelativePath::new("crates/vb_validate/src/schema_fields.rs"),
];

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

fn main() -> std::process::ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[verify-no-legacy-primitives] target discovery failed: {error}");
            return exit(LaneExit::Failure);
        }
    };
    let mut report = LaneReport::new();
    run(&target, &mut report);
    print_and_exit(&report)
}

fn run(target: &TargetProject, report: &mut LaneReport) {
    SOURCES.iter().for_each(|rel| check_file(target, *rel, report));
}

fn print_and_exit(report: &LaneReport) -> std::process::ExitCode {
    let rendered = report.render();
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    if report.is_clean() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}

fn check_file(target: &TargetProject, rel: TargetRelativePath, report: &mut LaneReport) {
    let path = rel.in_target(target);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            eprintln!(
                "[verify-no-legacy-primitives] not applicable: {} absent; skipping optional vb_validate primitive source",
                rel.as_str()
            );
            return;
        }
        Err(error) => {
            report.push(Finding::new(
                "LEGACY-PRIM",
                rel.as_str(),
                0,
                format!("file not readable: {:?}", error.kind()),
            ));
            return;
        }
    };
    check_const_body(&text, rel, "const STEP_PRIMITIVES", "STEP_PRIMITIVES", report);
    check_const_body(&text, rel, "const ALLOWED_STEP_FIELDS", "ALLOWED_STEP_FIELDS", report);
}

fn check_const_body(
    text: &str,
    rel: TargetRelativePath,
    marker: &str,
    label: &str,
    report: &mut LaneReport,
) {
    let Some(body) = extract_const_body(text, marker) else {
        eprintln!(
            "[verify-no-legacy-primitives] clean: {} has no {}; skipping optional constant",
            rel.as_str(),
            label
        );
        return;
    };
    FORBIDDEN.iter().for_each(|bad| {
        if body.contains(bad) {
            report.push(Finding::new(
                "LEGACY-PRIM",
                rel.as_str(),
                0,
                format!("{label} contains {bad}"),
            ));
        }
    });
}

fn extract_const_body<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)?;
    let after_marker = text.get(start..)?;
    let equals_offset = after_marker.find('=')?;
    let scan_start = start.saturating_add(equals_offset);
    let scan = text.get(scan_start..)?;
    let (open_pos, open, close) = scan.char_indices().find_map(|(offset, ch)| match ch {
        '[' => Some((scan_start.saturating_add(offset), '[', ']')),
        '{' => Some((scan_start.saturating_add(offset), '{', '}')),
        _ => None,
    })?;
    extract_balanced(text, start, open_pos, open, close)
}

fn extract_balanced(
    text: &str,
    start: usize,
    open_pos: usize,
    open: char,
    close: char,
) -> Option<&str> {
    let tail = text.get(open_pos..)?;
    let mut depth: i32 = 0;
    for (offset, ch) in tail.char_indices() {
        if ch == open {
            depth = depth.saturating_add(1);
        } else if ch == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                let end = open_pos.saturating_add(offset).saturating_add(ch.len_utf8());
                return text.get(start..end);
            }
        }
    }
    None
}
