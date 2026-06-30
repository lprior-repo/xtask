//! Scans `*.rs` for `#![feature(...)]` attributes and rejects disallowed
//! unstable feature names.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-nightly-features.sh`. Run via
//! `cargo run --bin check-nightly-features --` from the repository root
//! or via the matching Moon task in `.moon/tasks/all.yml`.
//!
//! ## Behavior parity
//! Two allowed feature sets:
//! - `normal_allowed = ^(try_blocks|portable_simd)$`
//! - `perf_only_allowed = ^(allocator_api|generic_const_exprs)$` — but
//!   only when the file is in a perf scope
//!   (`crates/*/src/perf/*`, `crates/*/src/generated/*`, or `benches/*`)
//!   OR the file contains the `velvet-allow-perf-nightly-feature` marker.
//!
//! Anything else triggers a finding and the lane exits 1 (mapped to
//! `LaneExit::Violations` here).
//!
//! File enumeration mirrors the bash's `rg --files` call: `*.rs` only,
//! excluding `target/`, `.git/`, `.beads/`, `vb-*/`, `arch-drift-*/`,
//! `**/target/**`, `**/generated-build/**`, `**/build-output/**`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use titania_lanes::{Finding, LaneExit, LaneReport, exit};

/// Stable features allowed in any scope.
const NORMAL_ALLOWED: &[&str] = &["try_blocks", "portable_simd"];

/// Features allowed only in perf-scoped paths or files with the marker
/// comment.
const PERF_ONLY_ALLOWED: &[&str] = &["allocator_api", "generic_const_exprs"];
/// Marker string for opt-in perf feature use.
const PERF_MARKER: &str = "velvet-allow-perf-nightly-feature";

/// Path glob patterns excluded from the file walk.
const EXCLUDED_GLOBS: &[&str] = &[
    "/target/",
    "/.git/",
    "/.beads/",
    "/vb-",
    "/arch-drift-",
    "/generated-build/",
    "/build-output/",
];

fn main() -> std::process::ExitCode {
    let mut report = LaneReport::new();
    for file in collect_source_files() {
        scan_file(&file, &mut report);
    }

    eprint!("{}", report.render());
    if report.is_clean() {
        eprintln!("[check-nightly-features] no disallowed feature attributes");
        exit(LaneExit::Clean)
    } else {
        exit(LaneExit::Violations)
    }
}

fn collect_source_files() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    walk(Path::new("."), &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let normalized = path.to_string_lossy().replace('\\', "/");
        if EXCLUDED_GLOBS.iter().any(|g| normalized.contains(g)) {
            continue;
        }
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn scan_file(path: &Path, report: &mut LaneReport) {
    report.record_scan();
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };

    let display = path.display().to_string();
    let is_perf_scoped = is_perf_scoped_path(&display);
    let has_marker = content.contains(PERF_MARKER);

    // Phase 1: collect the body of each `#![feature(...)]` attribute,
    // including multi-line forms that span until `)]`. Each yield
    // becomes one `feature_line` + comma-separated `names`.
    for (line_no, names, line_no_for_message) in collect_features(&content) {
        for name in names {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                continue;
            }
            check_feature(FeatureCheck {
                file: &display,
                feature_line: line_no,
                name: trimmed,
                scope: FeatureScope { is_perf_scoped, has_marker },
                report,
                report_line: line_no_for_message,
            });
        }
    }
}

/// Yield `(feature_line, names, first_occurrence_line)` tuples. The
/// first occurrence line is what the bash reports; subsequent lines in
/// a multi-line attribute are tracked but produce no extra message
/// (the bash's behavior: status flips once and the original line is
/// referenced). For this Rust port we surface one finding per
/// individual feature, so we attach the first line of the attribute.
type FeatureUse = (u32, Vec<String>, u32);

struct FeatureCollector {
    first_line_of_attr: u32,
    accumulating: Option<String>,
}

fn collect_features(content: &str) -> Vec<FeatureUse> {
    let mut out: Vec<FeatureUse> = Vec::new();
    let mut collector = FeatureCollector { first_line_of_attr: 0, accumulating: None };
    content.lines().enumerate().for_each(|(idx, line)| {
        collect_feature_line(&mut collector, feature_line_no(idx), line.trim(), &mut out);
    });
    if let Some(buf) = collector.accumulating {
        eprintln!("unterminated unstable feature attribute starting with `{buf}`");
    }
    out
}

fn feature_line_no(idx: usize) -> u32 {
    u32::try_from(idx.saturating_add(1)).map_or(u32::MAX, |line_no| line_no)
}

fn collect_feature_line(
    collector: &mut FeatureCollector,
    line_no: u32,
    trimmed: &str,
    out: &mut Vec<FeatureUse>,
) {
    if collect_accumulated_line(collector, trimmed, out) {
        return;
    }
    if let Some(after_open) = trimmed.strip_prefix("#![feature(") {
        collect_feature_start(collector, line_no, after_open, out);
    }
}

fn collect_accumulated_line(
    collector: &mut FeatureCollector,
    trimmed: &str,
    out: &mut Vec<FeatureUse>,
) -> bool {
    let Some(buf) = collector.accumulating.as_mut() else {
        return false;
    };
    buf.push(' ');
    buf.push_str(trimmed);
    push_closed_feature(collector.first_line_of_attr, buf, out);
    if buf.find(")]").is_some() {
        collector.accumulating = None;
    }
    true
}

fn collect_feature_start(
    collector: &mut FeatureCollector,
    line_no: u32,
    after_open: &str,
    out: &mut Vec<FeatureUse>,
) {
    if push_closed_feature(line_no, after_open, out) {
        return;
    }
    collector.first_line_of_attr = line_no;
    collector.accumulating = Some(format!("#![feature({after_open}"));
}

fn push_closed_feature(line_no: u32, text: &str, out: &mut Vec<FeatureUse>) -> bool {
    let Some(close_idx) = text.find(")]") else {
        return false;
    };
    if let Some(slice) = text.get(..=close_idx) {
        out.push((line_no, extract_names(slice), line_no));
    }
    true
}

fn extract_names(inside: &str) -> Vec<String> {
    // `inside` starts with `#![feature(` and ends with `)]`. Strip
    // those and split on commas.
    let body = inside.trim_start_matches("#![feature(").trim_end_matches(")]");
    body.split(',').map(|s| s.trim().to_owned()).collect()
}

fn is_perf_scoped_path(file: &str) -> bool {
    let normalized = file.replace('\\', "/");
    let perf_prefixes = ["crates/", "benches/"];
    // Each prefix has its own scope: `crates/<name>/src/perf/` or
    if perf_prefixes.iter().any(|p| normalized.contains(p))
        && (normalized.contains("/src/perf/") || normalized.contains("/src/generated/"))
    {
        return true;
    }
    normalized.starts_with("benches/")
}

struct FeatureScope {
    is_perf_scoped: bool,
    has_marker: bool,
}

struct FeatureCheck<'a> {
    file: &'a str,
    feature_line: u32,
    name: &'a str,
    scope: FeatureScope,
    report: &'a mut LaneReport,
    report_line: u32,
}

fn check_feature(check: FeatureCheck<'_>) {
    let FeatureCheck { file, feature_line, name, scope, report, report_line } = check;
    if NORMAL_ALLOWED.contains(&name) {
        return;
    }
    if PERF_ONLY_ALLOWED.contains(&name) {
        if scope.is_perf_scoped || scope.has_marker {
            return;
        }
        report.push(Finding::new(
            "NIGHTLY-FEATURE-002",
            file.to_owned(),
            report_line,
            format!(
                "perf-only unstable feature `{name}` outside approved scope (line {feature_line})"
            ),
        ));
        return;
    }
    report.push(Finding::new(
        "NIGHTLY-FEATURE-001",
        file.to_owned(),
        report_line,
        format!("disallowed unstable feature `{name}` (line {feature_line})"),
    ));
}
