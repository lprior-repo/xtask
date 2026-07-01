use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use titania_lanes::{Finding, LaneExit, LaneReport, current_target_project, exit};

/// Candidate wrong-spelling token. A [`SpellingRule`] makes the lane
/// applicable only when this differs from the canonical spelling.
const WRONG_SPELLING: &str = "velvet-ballistics";
const CANONICAL_SPELLING: &str = "velvet-ballistics";

/// File extensions we scan (matches the bash `--include` list).
const SCAN_EXTENSIONS: &[&str] = &["rs", "toml", "yaml", "yml", "md", "sh", "py"];

const EXCLUDED_SUBSTRINGS: &[&str] = &[
    "/.beads/",
    "/.jj/",
    "/.evidence/",
    "/evidence/",
    "/target/",
    "/target_nosccache/",
    "/target_debug_clean/",
    "/target_clean/",
    "/tests/",
    "/benches/",
    "/naming_scan/",
    "/vb-",
    "/femdation-vb-",
    "/go-skill-",
    "/holzman-workspace-",
    "/pick5-",
];

pub(crate) fn main_exit() -> ExitCode {
    let rule = match spelling_rule() {
        Ok(rule) => rule,
        Err(code) => return code,
    };
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-spelling-gate] cannot resolve target project: {error}");
            return exit(LaneExit::Usage);
        }
    };
    run_for_root(target.as_std_path(), &rule)
}

fn spelling_rule() -> Result<SpellingRule<'static>, ExitCode> {
    SpellingRule::parse(WRONG_SPELLING, CANONICAL_SPELLING).map_err(|error| match error {
        SpellingRuleError::IdenticalTerms => not_applicable_rule_exit(),
        SpellingRuleError::EmptyTerm => invalid_rule_exit(),
    })
}

fn not_applicable_rule_exit() -> ExitCode {
    eprintln!("NotApplicable: spelling rule has identical wrong/canonical terms");
    exit(LaneExit::NotApplicable)
}

fn invalid_rule_exit() -> ExitCode {
    eprintln!("InvalidInvocation: spelling rule contains an empty term");
    exit(LaneExit::Usage)
}

fn run_for_root(root: &Path, rule: &SpellingRule<'_>) -> ExitCode {
    eprintln!("=== Spelling Gate: {} vs {} ===", rule.bad(), rule.good());
    let mut report = LaneReport::new();
    collect_files(root).iter().for_each(|file| scan_file(file, rule, &mut report));
    eprintln!("=== Spelling Gate complete: {} violations ===", report.finding_count());
    eprint!("{}", report.render());
    if report.is_clean() { exit(LaneExit::Clean) } else { spelling_violations_exit(rule) }
}

fn spelling_violations_exit(rule: &SpellingRule<'_>) -> ExitCode {
    eprintln!("Hint: The canonical spelling is '{}'.", rule.good());
    eprintln!("Allowlisted path patterns (excluded entirely):");
    eprintln!("  - .beads/ (bead artifacts and CI output)");
    eprintln!("  - .jj/ (JJ internal state)");
    eprintln!("  - target/ (build artifacts)");
    eprintln!("  - tests/ and benches/ (test/bench clippy is not strict)");
    eprintln!("  - velvet-ballistics-MASTER.md (master contract file)");
    eprintln!("Allowlisted content patterns:");
    eprintln!("  - velvet-ballistics-MASTER.md (reference to master file)");
    eprintln!("  - source checkout path migration artifacts");
    eprintln!("  - FORBIDDEN_FEATURE_NAMES (spelling used as forbid-tag)");
    eprintln!("  - explicit rule statements");
    exit(LaneExit::Violations)
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    entries.filter_map(Result::ok).map(|entry| entry.path()).for_each(|path| visit_path(path, out));
}

fn visit_path(path: PathBuf, out: &mut Vec<PathBuf>) {
    if path.is_dir() && !is_heavy_tree(&path) {
        walk(&path, out);
    } else if is_scanned_file(&path) {
        out.push(path);
    }
}

fn is_heavy_tree(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| matches!(name, "target" | "node_modules" | ".git"))
}

fn is_scanned_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|ext| SCAN_EXTENSIONS.contains(&ext))
}

fn is_path_excluded(file: &Path) -> bool {
    let normalized = file.to_string_lossy().replace('\\', "/");
    contains_excluded_substring(&normalized) || has_excluded_filename(file)
}

fn contains_excluded_substring(normalized: &str) -> bool {
    EXCLUDED_SUBSTRINGS.iter().any(|s| normalized.contains(s))
}

fn has_excluded_filename(file: &Path) -> bool {
    let name = file.file_name().and_then(|n| n.to_str()).map_or("", core::convert::identity);
    name == "check-spelling-gate.sh"
        || name == "check_spelling_gate.rs"
        || name == "velvet-ballistics-MASTER.md"
        || name == "BIG-ASS-TESTING-TO-FIX.md"
        || name.ends_with("_tests.rs")
        || name.contains("final-")
        || name.contains("proof-repair-")
        || name.contains("black-hat-review-")
}

fn is_content_allowed(line: &str, rule: &SpellingRule<'_>) -> bool {
    let bad = rule.bad();
    line.contains("velvet-ballistics-MASTER.md")
        || (line.contains("/home/") && line.contains(bad))
        || line.contains("FORBIDDEN_FEATURE_NAMES")
        || line.contains("is invalid")
        || (line.contains("dolthub.com/") && line.contains(bad))
        || line.contains("velvet-ballistics/v2")
}

fn scan_file(file: &Path, rule: &SpellingRule<'_>, report: &mut LaneReport) {
    if is_path_excluded(file) {
        return;
    }
    let Ok(content) = std::fs::read_to_string(file) else {
        return;
    };
    let display = file.display().to_string();
    content
        .lines()
        .enumerate()
        .for_each(|(idx, line)| scan_line(line, idx, &display, rule, report));
}

fn scan_line(
    line: &str,
    idx: usize,
    display: &str,
    rule: &SpellingRule<'_>,
    report: &mut LaneReport,
) {
    if !line.contains(rule.bad()) || is_content_allowed(line, rule) {
        return;
    }
    let line_no = u32::try_from(idx.saturating_add(1)).map_or(u32::MAX, core::convert::identity);
    report.push(Finding::new(
        "SPELLING-GATE-001",
        display,
        line_no,
        format!("wrong spelling '{}' (use '{}')", rule.bad(), rule.good()),
    ));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SpellingRule<'a> {
    bad: &'a str,
    good: &'a str,
}

impl<'a> SpellingRule<'a> {
    fn parse(bad: &'a str, good: &'a str) -> Result<Self, SpellingRuleError> {
        if bad.trim().is_empty() || good.trim().is_empty() {
            return Err(SpellingRuleError::EmptyTerm);
        }
        if bad == good {
            return Err(SpellingRuleError::IdenticalTerms);
        }
        Ok(Self { bad, good })
    }

    fn bad(&self) -> &str {
        self.bad
    }

    fn good(&self) -> &str {
        self.good
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpellingRuleError {
    EmptyTerm,
    IdenticalTerms,
}
