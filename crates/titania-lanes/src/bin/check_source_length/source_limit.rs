use std::path::{Path, PathBuf};

use titania_lanes::{Finding, LaneReport, helpers::relative_path};

use crate::{LEDGER_PATH, SOURCE_LINE_LIMIT, paths::is_test_like_source_path};

pub(crate) fn check_source_line_limit(
    root: &Path,
    files: &[PathBuf],
    ledger: &[String],
    report: &mut LaneReport,
) {
    sorted_files(files)
        .into_iter()
        .filter_map(|file| source_line_violation(root, file, ledger))
        .for_each(|finding| report.push(finding));
}

fn sorted_files(files: &[PathBuf]) -> Vec<&PathBuf> {
    let mut sorted: Vec<&PathBuf> = files.iter().collect();
    sorted.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    sorted
}

fn source_line_violation(root: &Path, file: &Path, ledger: &[String]) -> Option<Finding> {
    let rel = relative_path(root, file);
    if is_test_like_source_path(&rel) || has_exception(&rel, ledger) {
        return None;
    }
    let lines = physical_lines(file)?;
    if lines <= SOURCE_LINE_LIMIT {
        return None;
    }
    Some(Finding::new(
        "SRC-LINE-LIMIT",
        rel,
        0,
        format!(
            "has {lines} physical lines (limit <={SOURCE_LINE_LIMIT}) and no valid {LEDGER_PATH} row"
        ),
    ))
}

fn has_exception(rel: &str, ledger: &[String]) -> bool {
    ledger.iter().any(|entry| entry == rel)
}

fn physical_lines(file: &Path) -> Option<usize> {
    std::fs::read_to_string(file).ok().map(|text| text.lines().count())
}
