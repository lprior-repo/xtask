use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use aho_corasick::AhoCorasick;
use titania_lanes::{Finding, LaneReport, helpers::line_no_from_idx};

use crate::source::SourceLine;

/// Build the Aho-Corasick automaton over the file-level fallible-signal needles.
///
/// Returns `None` if the automaton cannot be constructed; callers fall back to
/// the per-line slow path in that case (this crate never expects AC build to
/// fail at runtime given the static needle set).
pub(super) fn build_fallible_signal_ac() -> Option<AhoCorasick> {
    AhoCorasick::builder()
        .match_kind(aho_corasick::MatchKind::Standard)
        .build(FALLIBLE_SIGNAL_NEEDLES)
        .ok()
}

/// Fallible-signal needles used by the file-level AC prefilter. The per-line
/// `contains_fallible_signal` check enforces the word-boundary + `(`-after
/// rule; the AC prefilter is a fast first-pass that lets files lacking any of
/// these substrings skip the per-line scan entirely. Keep this list aligned
/// with the needles in `contains_fallible_signal` minus the trailing `(`.
const FALLIBLE_SIGNAL_NEEDLES: &[&str] = &[
    "fallible",
    "try_",
    "write_",
    "send",
    "recv",
    "cancel",
    "persist",
    "commit",
    "remove_",
    "create_",
    "open_",
    "save_",
    "read_to_",
    "from_bytes",
    "to_allocvec",
    "try_from_parts",
];

pub(super) fn scan(
    root: &Path,
    ac: Option<&AhoCorasick>,
    allow: &BTreeMap<String, String>,
    report: &mut LaneReport,
) {
    scan_roots(root).iter().for_each(|file_root| {
        scan_dir(file_root, root, ac, allow, report);
    });
}

fn scan_roots(root: &Path) -> Vec<PathBuf> {
    let crates = crate_src_roots(root);
    let xtask = xtask_root(root);
    crates.into_iter().chain(xtask).collect()
}

fn crate_src_roots(root: &Path) -> Vec<PathBuf> {
    let crates_dir = root.join("crates");
    let Ok(read) = std::fs::read_dir(&crates_dir) else {
        return Vec::new();
    };
    read.flatten().filter_map(crate_src_root).collect()
}

fn crate_src_root(entry: std::fs::DirEntry) -> Option<PathBuf> {
    let path = entry.path();
    let src = path.join("src");
    (path.is_dir() && src.is_dir()).then_some(src)
}

fn xtask_root(root: &Path) -> Option<PathBuf> {
    let xtask = root.join("xtask/src");
    xtask.is_dir().then_some(xtask)
}

fn should_skip(rel: &str) -> bool {
    rel.contains("kani_")
        || rel.contains("workspace_tests")
        || rel.ends_with("/tests.rs")
        || rel.ends_with("_tests.rs")
        || rel.contains("/test_harness.rs")
        || rel.contains("/tests/")
        || rel.contains("/impl_tests/")
        || rel.contains("/lifecycle_tests/")
}

fn scan_dir(
    dir: &Path,
    root: &Path,
    ac: Option<&AhoCorasick>,
    allow: &BTreeMap<String, String>,
    report: &mut LaneReport,
) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    read.flatten().for_each(|entry| scan_entry(entry.path(), root, ac, allow, report));
}

fn scan_entry(
    path: PathBuf,
    root: &Path,
    ac: Option<&AhoCorasick>,
    allow: &BTreeMap<String, String>,
    report: &mut LaneReport,
) {
    if path.is_dir() {
        scan_dir(&path, root, ac, allow, report);
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
        scan_rust_file(&path, root, ac, allow, report);
    }
}

fn scan_rust_file(
    file: &Path,
    root: &Path,
    ac: Option<&AhoCorasick>,
    allow: &BTreeMap<String, String>,
    report: &mut LaneReport,
) {
    let rel = rel_str(root, file);
    if !should_skip(&rel) {
        scan_file(file, &rel, ac, allow, report);
    }
}

fn rel_str(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_error) => path.to_string_lossy().into_owned(),
    }
}

fn scan_file(
    file: &Path,
    rel: &str,
    ac: Option<&AhoCorasick>,
    allow: &BTreeMap<String, String>,
    report: &mut LaneReport,
) {
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    if let Some(automaton) = ac {
        let has_signal = automaton.find_overlapping_iter(&text).next().is_some();
        if !has_signal {
            return;
        }
    }
    let mut block_comment = false;
    let mut context = ScanContext { rel, allow, report };
    text.lines().enumerate().for_each(|(idx, line)| {
        scan_line(&mut context, line_no_from_idx(idx), line, &mut block_comment);
    });
}

struct ScanContext<'a> {
    rel: &'a str,
    allow: &'a BTreeMap<String, String>,
    report: &'a mut LaneReport,
}

fn scan_line(context: &mut ScanContext<'_>, line_no: u32, raw: &str, block_comment: &mut bool) {
    let source_line = SourceLine::parse(raw, block_comment);
    if let Some(class_id) = classify_line(&source_line) {
        push_unless_allowed(context, line_no, raw, class_id);
    }
}

fn push_unless_allowed(
    context: &mut ScanContext<'_>,
    line_no: u32,
    raw: &str,
    class_id: &'static str,
) {
    let key = format!("{}|{class_id}", context.rel);
    if !context.allow.contains_key(&key) {
        context.report.push(Finding::new(
            class_id,
            context.rel,
            line_no,
            format!("discarded fallible: {}", raw.trim()),
        ));
    }
}

fn classify_line(line: &SourceLine) -> Option<&'static str> {
    let trimmed = line.code();
    if is_ignored_line(line, trimmed) {
        return None;
    }
    discard_patterns(trimmed)
        .into_iter()
        .find_map(|(class_id, matched)| matched.then_some(class_id))
}

fn is_ignored_line(line: &SourceLine, trimmed: &str) -> bool {
    trimmed.is_empty()
        || line.is_signature()
        || trimmed.starts_with("use ")
        || trimmed.starts_with("return ")
        || !line.is_code_expression()
}

fn discard_patterns(trimmed: &str) -> [(&'static str, bool); 5] {
    [
        ("DISCARD-002", discarded_assignment(trimmed)),
        ("DISCARD-003", discarded_ok_err(trimmed)),
        ("DISCARD-004", discarded_match_arm(trimmed)),
        ("DISCARD-005", discarded_drop(trimmed)),
        ("DISCARD-001", discarded_bare_call(trimmed)),
    ]
}

fn discarded_assignment(trimmed: &str) -> bool {
    (trimmed.starts_with("let _ =") || trimmed.starts_with("let _="))
        && contains_fallible_signal(trimmed)
}

fn discarded_ok_err(trimmed: &str) -> bool {
    (trimmed.ends_with(".ok();") || trimmed.ends_with(".err();"))
        && contains_fallible_signal(trimmed)
}

fn discarded_match_arm(trimmed: &str) -> bool {
    trimmed.contains("Ok(()) | Err(_) => {}")
        || trimmed.contains("Ok(())|Err(_)=>{}")
        || trimmed.contains("Err(_) => {}")
}

fn discarded_drop(trimmed: &str) -> bool {
    trimmed.contains("drop(") && contains_fallible_signal(trimmed)
}

fn discarded_bare_call(trimmed: &str) -> bool {
    trimmed.ends_with(';')
        && !trimmed.contains('=')
        && !trimmed.contains('?')
        && !trimmed.contains('|')
        && !trimmed.contains("assert")
        && !trimmed.contains("expect(")
        && !trimmed.contains("unwrap")
        && contains_fallible_signal(trimmed)
}

fn contains_fallible_signal(trimmed: &str) -> bool {
    [
        "fallible",
        "try_",
        "write_",
        "send(",
        "recv(",
        "cancel",
        "persist",
        "commit",
        "remove_",
        "create_",
        "open_",
        "save_",
        "read_to_",
        "from_bytes",
        "to_allocvec",
        "try_from_parts",
    ]
    .iter()
    .any(|needle| trimmed.contains(needle))
}
