use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    allow_file::load_allow_file,
    model::{COLD_MARKERS, FindingData, HOT_CRATES, SourceRole},
    syntax::{ApiSourceLine, compact, remove_spaces},
};

pub(super) fn scan(
    root: &Path,
) -> Result<(Vec<String>, Vec<FindingData>, Vec<FindingData>), String> {
    let allowed = load_allow_file(root)?;
    let sources = hot_sources(root).map_err(|error| format!("hot source scan failed: {error}"))?;
    let mut state = ScanState::new(allowed);
    sources.iter().try_for_each(|source| state.scan_source(root, source))?;
    Ok(state.finish())
}

struct ScanState {
    allowed: BTreeSet<(String, String)>,
    classified: Vec<String>,
    violations: Vec<FindingData>,
    justified: Vec<FindingData>,
}

impl ScanState {
    fn new(allowed: BTreeSet<(String, String)>) -> Self {
        Self { allowed, classified: Vec::new(), violations: Vec::new(), justified: Vec::new() }
    }

    fn finish(self) -> (Vec<String>, Vec<FindingData>, Vec<FindingData>) {
        (self.classified, self.violations, self.justified)
    }

    fn scan_source(&mut self, root: &Path, source: &Path) -> Result<(), String> {
        let rel_path = relative_path(root, source);
        let role = source_role(&rel_path);
        self.classified.push(format!("ClassifiedPath|{:?}|{}", role, rel_path));
        if role != SourceRole::HotProduction {
            return Ok(());
        }
        let text = fs::read_to_string(source)
            .map_err(|error| format!("{}: unreadable: {error}", source.display()))?;
        self.scan_hot_text(&rel_path, &text);
        Ok(())
    }

    fn scan_hot_text(&mut self, rel_path: &str, text: &str) {
        let mut state = HotLineState::default();
        text.lines().enumerate().for_each(|(index, line)| {
            self.scan_hot_line(rel_path, index.saturating_add(1), line, &mut state);
        });
    }

    fn scan_hot_line(
        &mut self,
        rel_path: &str,
        line_no: usize,
        line: &str,
        state: &mut HotLineState,
    ) {
        if state.test_scope.skip_line(line) {
            return;
        }
        let source_line = ApiSourceLine::parse(line, &mut state.block_comment);
        classify_line(rel_path, line_no, &source_line)
            .into_iter()
            .for_each(|finding| self.push_classified_finding(finding));
    }

    fn push_classified_finding(&mut self, finding: FindingData) {
        let key = (finding.rel_path.clone(), finding.class_id.to_owned());
        if self.allowed.contains(&key) {
            self.justified.push(finding);
        } else {
            self.violations.push(finding);
        }
    }
}

#[derive(Default)]
struct HotLineState {
    block_comment: bool,
    test_scope: TestScope,
}

#[derive(Default)]
struct TestScope {
    cfg_test_pending: bool,
    depth: i32,
}

impl TestScope {
    fn skip_line(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        if self.depth > 0 {
            self.depth = next_depth(self.depth, line);
            return true;
        }
        if trimmed.starts_with("#[cfg(test)]") {
            self.cfg_test_pending = true;
            return true;
        }
        if self.cfg_test_pending && trimmed.contains("mod ") {
            self.depth = initial_test_depth(line);
            self.cfg_test_pending = false;
            return true;
        }
        self.clear_pending_for_code(trimmed);
        false
    }

    fn clear_pending_for_code(&mut self, trimmed: &str) {
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            self.cfg_test_pending = false;
        }
    }
}

fn next_depth(current: i32, line: &str) -> i32 {
    current.saturating_add(char_count_i32(line, '{')).saturating_sub(char_count_i32(line, '}'))
}

fn initial_test_depth(line: &str) -> i32 {
    let depth = char_count_i32(line, '{').saturating_sub(char_count_i32(line, '}'));
    if depth <= 0 { 1 } else { depth }
}

fn char_count_i32(line: &str, needle: char) -> i32 {
    i32::try_from(line.matches(needle).count()).map_or(i32::MAX, core::convert::identity)
}

fn relative_path(root: &Path, source: &Path) -> String {
    match source.strip_prefix(root) {
        Ok(path) => path.display().to_string(),
        Err(_error) => source.display().to_string(),
    }
}

fn source_role(path: &str) -> SourceRole {
    if is_test_path(path) {
        return SourceRole::Test;
    }
    if path.contains("/src/bin/") || path.starts_with("crates/titania-lanes/") {
        return SourceRole::LaneBinary;
    }
    if is_cold_path(path) {
        return SourceRole::ColdSupport;
    }
    if path.starts_with("crates/titania-core/src/") {
        SourceRole::HotProduction
    } else {
        SourceRole::ColdSupport
    }
}

fn is_test_path(path: &str) -> bool {
    path.contains("/tests/")
        || path.ends_with("/tests.rs")
        || path.ends_with("_tests.rs")
        || path.contains("/benches/")
        || path.contains("/kani/")
        || path.ends_with("/kani.rs")
}

fn is_cold_path(path: &str) -> bool {
    path.split(['/', '.', '_', '-']).any(|token| COLD_MARKERS.contains(&token))
}

fn line_has_string_map(line: &str) -> bool {
    let normalized = remove_spaces(line);
    [
        "HashMap<String",
        "HashMap<&str",
        "BTreeMap<String",
        "BTreeMap<&str",
        "IndexMap<String",
        "IndexMap<&str",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn classify_line(rel_path: &str, line_no: usize, source_line: &ApiSourceLine) -> Vec<FindingData> {
    let stripped = source_line.code();
    if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with("use ") {
        return Vec::new();
    }
    let text = compact(stripped);
    checks_for(stripped)
        .into_iter()
        .filter_map(|(class_id, matched)| {
            finding_if_matched(rel_path, line_no, class_id, &text, matched)
        })
        .collect()
}

fn checks_for(stripped: &str) -> [(&'static str, bool); 6] {
    [
        ("FORMAT-PRINT-001", stripped.contains("println!(") || stripped.contains("eprintln!(")),
        ("FORMAT-DBG-001", stripped.contains("dbg!(")),
        (
            "FORMAT-JSON-001",
            stripped.contains("serde_json") || stripped.contains("serde_json::Value"),
        ),
        (
            "FORMAT-YAML-001",
            stripped.contains("serde_saphyr")
                || stripped.contains("saphyr::")
                || stripped.contains(" saphyr"),
        ),
        ("MAP-STRING-001", line_has_string_map(stripped)),
        ("CHANNEL-UNBOUNDED-001", has_unbounded_channel(stripped)),
    ]
}

fn finding_if_matched(
    rel_path: &str,
    line_no: usize,
    class_id: &'static str,
    text: &str,
    matched: bool,
) -> Option<FindingData> {
    matched.then(|| FindingData {
        rel_path: rel_path.to_owned(),
        line_no,
        class_id,
        text: text.to_owned(),
    })
}

fn has_unbounded_channel(stripped: &str) -> bool {
    stripped.contains("std::sync::mpsc::channel(")
        || stripped.contains("mpsc::channel(")
        || stripped.contains("unbounded_channel(")
        || stripped.contains("crossbeam_channel::unbounded(")
}

fn rust_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    fs::read_dir(root)?.try_fold(Vec::new(), |mut out, entry| {
        append_rust_entry(&mut out, entry?, root)?;
        Ok(out)
    })
}

fn append_rust_entry(out: &mut Vec<PathBuf>, entry: fs::DirEntry, _root: &Path) -> io::Result<()> {
    let path = entry.path();
    if path.is_dir() {
        out.extend(rust_files(&path)?);
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
        out.push(path);
    }
    Ok(())
}

fn hot_sources(root: &Path) -> io::Result<Vec<PathBuf>> {
    HOT_CRATES.iter().try_fold(Vec::new(), |mut out, crate_name| {
        let src = root.join("crates").join(crate_name).join("src");
        out.extend(rust_files(&src)?);
        Ok(out)
    })
}
