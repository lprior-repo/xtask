use std::path::Path;

use titania_lanes::{
    Finding, LaneReport,
    helpers::{brace_delta, line_no_from_idx, relative_path, saturating_add_usize},
};

use crate::FN_LINE_LIMIT;

#[derive(Clone, Copy)]
enum ScanState {
    Outside,
    Inside { start_line: u32, count: usize, depth: i32 },
}

pub(crate) fn check_file(root: &Path, file: &Path, report: &mut LaneReport) {
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    let rel = relative_path(root, file);
    scan_functions(&text, &rel, report);
}

fn scan_functions(text: &str, rel: &str, report: &mut LaneReport) {
    let mut state = ScanState::Outside;
    text.lines().enumerate().for_each(|(idx, raw)| {
        state = state.advance(raw, line_no_from_idx(idx), rel, report);
    });
}

impl ScanState {
    fn advance(self, raw: &str, line_no: u32, rel: &str, report: &mut LaneReport) -> Self {
        match self {
            Self::Outside => outside_next(raw, line_no),
            Self::Inside { .. } => inside_next(self, raw, rel, report),
        }
    }
}

fn outside_next(raw: &str, line_no: u32) -> ScanState {
    if is_fn_header(raw) {
        ScanState::Inside { start_line: line_no, count: 0, depth: brace_delta(raw) }
    } else {
        ScanState::Outside
    }
}

fn inside_next(state: ScanState, raw: &str, rel: &str, report: &mut LaneReport) -> ScanState {
    let ScanState::Inside { start_line, count, depth } = state else {
        return state;
    };
    let count = next_count(count, raw);
    let depth = depth.saturating_add(brace_delta(raw));
    if depth > 0 {
        return ScanState::Inside { start_line, count, depth };
    }
    push_oversized_function(start_line, count, rel, report);
    ScanState::Outside
}

fn next_count(count: usize, raw: &str) -> usize {
    if is_logical_line(raw) { saturating_add_usize(count, 1) } else { count }
}

fn push_oversized_function(start_line: u32, count: usize, rel: &str, report: &mut LaneReport) {
    if count > FN_LINE_LIMIT {
        report.push(Finding::new(
            "FN-LINE-LIMIT",
            rel,
            start_line,
            format!("function has {count} logical lines (limit {FN_LINE_LIMIT})"),
        ));
    }
}

fn is_fn_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || !line.contains("fn ") || !line.contains('(') {
        return false;
    }
    match line.find("fn ").and_then(|idx| line.get(..idx)) {
        Some(before) => is_fn_boundary(before),
        None => false,
    }
}

fn is_fn_boundary(before: &str) -> bool {
    match before.chars().last() {
        Some(prev) => !(prev.is_alphanumeric() || prev == '_'),
        None => true,
    }
}

fn is_logical_line(line: &str) -> bool {
    let trimmed = line.trim();
    !(trimmed.is_empty() || trimmed.starts_with("//") || trimmed == "{" || trimmed == "}")
}
