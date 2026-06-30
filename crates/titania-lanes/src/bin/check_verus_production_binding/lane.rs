use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use titania_lanes::{
    Finding, LaneExit, LaneReport, current_target_project, exit, helpers::relative_path,
};

const VERIFICATION_DIR: &str = "verification/verus";
const FIXTURE_SMOKE_MARKER: &str = "titania-verus-binding: fixture-smoke";
const FORMAL_SETUP_SMOKE_FILE: &str = "verification/verus/formal_setup_smoke.rs";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Binding {
    Strong,
    Weak,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProofScan {
    Binding(Binding),
    NotApplicable(NotApplicableReason),
    Vacuum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NotApplicableReason {
    FixtureSmoke,
    NoVerusDirectory,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct BindingSummary {
    pub(crate) strong: u32,
    pub(crate) weak: u32,
    pub(crate) not_applicable: u32,
    pub(crate) vacuum: u32,
}

impl BindingSummary {
    fn record_binding(&mut self, binding: &Binding) {
        match binding {
            Binding::Strong => self.strong = self.strong.saturating_add(1),
            Binding::Weak => self.weak = self.weak.saturating_add(1),
        }
    }

    fn record_not_applicable(&mut self, reason: &NotApplicableReason) {
        match reason {
            NotApplicableReason::FixtureSmoke | NotApplicableReason::NoVerusDirectory => {
                self.not_applicable = self.not_applicable.saturating_add(1);
            }
        }
    }

    fn record_vacuum(&mut self) {
        self.vacuum = self.vacuum.saturating_add(1);
    }
}

pub(crate) fn main_exit() -> ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-verus-production-binding] target discovery failed: {error}");
            return exit(LaneExit::Usage);
        }
    };
    let mut report = LaneReport::new();
    let summary = run(target.as_std_path(), &mut report);
    print_summary(&summary);
    print_and_exit(&report)
}

pub(crate) fn run(root: &Path, report: &mut LaneReport) -> BindingSummary {
    let mut summary = BindingSummary::default();
    candidate_proof_files(root, report, &mut summary)
        .iter()
        .for_each(|path| scan_candidate(root, path, report, &mut summary));
    summary
}

fn candidate_proof_files(
    root: &Path,
    report: &mut LaneReport,
    summary: &mut BindingSummary,
) -> Vec<PathBuf> {
    let dir = root.join(VERIFICATION_DIR);
    let read = match std::fs::read_dir(&dir) {
        Ok(read) => read,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            summary.record_not_applicable(&NotApplicableReason::NoVerusDirectory);
            return Vec::new();
        }
        Err(error) => {
            report.push(Finding::new(
                "SCAN_ERROR",
                VERIFICATION_DIR,
                0,
                format!("cannot read verification dir: {error}"),
            ));
            return Vec::new();
        }
    };
    read.filter_map(|entry| entry_path(entry, report))
        .filter(|path| is_candidate_path(root, path))
        .collect()
}

fn entry_path(
    entry: std::io::Result<std::fs::DirEntry>,
    report: &mut LaneReport,
) -> Option<PathBuf> {
    match entry {
        Ok(entry) => Some(entry.path()),
        Err(error) => {
            report.push(Finding::new(
                "SCAN_ERROR",
                VERIFICATION_DIR,
                0,
                format!("cannot read verification entry: {error}"),
            ));
            None
        }
    }
}

fn is_candidate_path(root: &Path, path: &Path) -> bool {
    path.is_file()
        && path.extension().and_then(|e| e.to_str()) == Some("rs")
        && !is_skipped_rel(&relative_path(root, path))
}

fn is_skipped_rel(rel: &str) -> bool {
    rel.ends_with("extern_.rs") || rel.contains("extern_") || rel.contains("production_inner/")
}

fn scan_candidate(root: &Path, path: &Path, report: &mut LaneReport, summary: &mut BindingSummary) {
    let rel = relative_path(root, path);
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            report.push(Finding::new(
                "SCAN_ERROR",
                rel,
                0,
                format!("cannot read proof file: {error}"),
            ));
            return;
        }
    };
    if has_proof_fn(&text) {
        record_proof_scan(classify(&rel, &text), &rel, report, summary);
    }
}

fn record_proof_scan(
    scan: ProofScan,
    rel: &str,
    report: &mut LaneReport,
    summary: &mut BindingSummary,
) {
    match scan {
        ProofScan::Binding(binding) => {
            summary.record_binding(&binding);
            let message = binding_message(&binding);
            report.push(Finding::new("BINDING", rel, 0, message));
        }
        ProofScan::NotApplicable(reason) => summary.record_not_applicable(&reason),
        ProofScan::Vacuum => {
            summary.record_vacuum();
            report.push(Finding::new("VACUUM", rel, 0, "VACUUM no production binding"));
        }
    }
}

fn binding_message(binding: &Binding) -> &'static str {
    match binding {
        Binding::Strong => "STRONG direct crates/ binding",
        Binding::Weak => "WEAK production_inner/ mirror",
    }
}

fn print_summary(summary: &BindingSummary) {
    eprintln!(
        "STRONG: {}, WEAK: {}, NOT_APPLICABLE: {}, VACUUM: {}",
        summary.strong, summary.weak, summary.not_applicable, summary.vacuum
    );
}

fn print_and_exit(report: &LaneReport) -> ExitCode {
    let rendered = report.render();
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    let has_blocking =
        report.findings().iter().any(|f| matches!(f.rule(), "VACUUM" | "SCAN_ERROR"));
    if has_blocking { exit(LaneExit::Violations) } else { exit(LaneExit::Clean) }
}

fn has_proof_fn(text: &str) -> bool {
    find_subslice(text, 0, "proof fn").is_some()
}

fn classify(rel: &str, text: &str) -> ProofScan {
    if is_fixture_smoke(rel, text) {
        return ProofScan::NotApplicable(NotApplicableReason::FixtureSmoke);
    }
    match (first_path_target(text).as_deref(), has_assume_specification(text)) {
        (Some(p), true) if p.contains("crates/") && !p.contains("proof_kernels/") => {
            ProofScan::Binding(Binding::Strong)
        }
        (Some(p), true) if p.contains("production_inner/") || p.contains("proof_kernels/") => {
            ProofScan::Binding(Binding::Weak)
        }
        (Some(_), true) => ProofScan::Binding(Binding::Weak),
        (Some(_), false) | (None, true) | (None, false) => ProofScan::Vacuum,
    }
}

fn is_fixture_smoke(rel: &str, text: &str) -> bool {
    rel == FORMAL_SETUP_SMOKE_FILE
        && text.contains(FIXTURE_SMOKE_MARKER)
        && find_subslice(text, 0, "proof fn formal_setup_smoke").is_some()
}

fn first_path_target(text: &str) -> Option<String> {
    text.match_indices("#[path = \"")
        .next()
        .and_then(|(start, needle)| path_target_after(text, start.saturating_add(needle.len())))
}

fn path_target_after(text: &str, start: usize) -> Option<String> {
    text.get(start..).map(|rest| rest.chars().take_while(|c| *c != '"').collect())
}

fn has_assume_specification(text: &str) -> bool {
    text.contains("assume_specification[")
}

fn find_subslice(text: &str, start: usize, needle: &str) -> Option<usize> {
    let rest = text.get(start..)?;
    rest.find(needle).map(|off| start.saturating_add(off))
}
