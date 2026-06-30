use std::{collections::BTreeSet, fs, io::ErrorKind, path::PathBuf, process::ExitCode};

use titania_core::TargetProject;
use titania_lanes::{Finding, LaneExit, LaneReport, current_target_project, exit};

const SRC: TargetRelativePath =
    TargetRelativePath::new("crates/vb_core/src/proof_kernels/step_state.rs");
const RULE_STEPSTATE: &str = "STEPSTATE";

type StateSet = BTreeSet<String>;

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

struct StepStateFacts {
    variants: StateSet,
    transitions: StateSet,
    is_terminal: StateSet,
    terminal_states: StateSet,
    non_terminal_fn: StateSet,
}

impl StepStateFacts {
    fn parse(text: &str) -> Self {
        let variants = extract_enum_variants(text, "StepState");
        let transitions = extract_block_after(text, "const VALID_TRANSITIONS", "];")
            .map_or_else(StateSet::new, |block| collect_stepstate_refs(&block));
        let is_terminal = find_function_body(text, "is_terminal")
            .map_or_else(StateSet::new, |block| collect_stepstate_refs(&block));
        let terminal_states = find_function_body(text, "terminal_states")
            .map_or_else(StateSet::new, |block| collect_stepstate_refs(&block));
        let non_terminal_fn = find_function_body(text, "non_terminal_states")
            .map_or_else(StateSet::new, |block| collect_stepstate_refs(&block));
        Self { variants, transitions, is_terminal, terminal_states, non_terminal_fn }
    }

    fn non_terminal_derived(&self) -> StateSet {
        self.variants.iter().filter(|v| !self.is_terminal.contains(*v)).cloned().collect()
    }
}

pub(crate) fn main_exit() -> ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-stepstate-matrix] target discovery failed: {error}");
            return exit(LaneExit::Usage);
        }
    };
    let mut report = LaneReport::new();
    run(&target, &mut report);
    print_and_exit(&report)
}

pub(crate) fn run(target: &TargetProject, report: &mut LaneReport) {
    let Some(text) = read_source(target, report) else {
        return;
    };
    let facts = StepStateFacts::parse(&text);
    check_transition_coverage(&facts.variants, &facts.transitions, report);
    check_terminal_consistency(&facts, report);
    check_non_terminal_consistency(&facts, report);
}

fn read_source(target: &TargetProject, report: &mut LaneReport) -> Option<String> {
    let path = SRC.in_target(target);
    match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            eprintln!(
                "[check-stepstate-matrix] not applicable: {} is absent under {}; skipping StepState matrix lane",
                SRC.as_str(),
                target
            );
            None
        }
        Err(error) => {
            report.push(Finding::new(
                RULE_STEPSTATE,
                SRC.as_str(),
                0,
                format!("source not readable: {:?}", error.kind()),
            ));
            None
        }
    }
}

fn check_transition_coverage(variants: &StateSet, transitions: &StateSet, report: &mut LaneReport) {
    variants.iter().filter(|v| !transitions.contains(*v)).for_each(|v| {
        report.push(Finding::new(
            RULE_STEPSTATE,
            SRC.as_str(),
            0,
            format!("variant {v} missing from VALID_TRANSITIONS"),
        ));
    });
    transitions.iter().filter(|t| !variants.contains(*t)).for_each(|t| {
        report.push(Finding::new(
            RULE_STEPSTATE,
            SRC.as_str(),
            0,
            format!("phantom state {t} in VALID_TRANSITIONS"),
        ));
    });
    if variants.len() != transitions.len() {
        report.push(Finding::new(
            RULE_STEPSTATE,
            SRC.as_str(),
            0,
            transition_count_message(variants, transitions),
        ));
    }
}

fn transition_count_message(variants: &StateSet, transitions: &StateSet) -> String {
    format!("variant count ({}) != transition state count ({})", variants.len(), transitions.len())
}

fn check_terminal_consistency(facts: &StepStateFacts, report: &mut LaneReport) {
    if facts.is_terminal != facts.terminal_states {
        report.push(Finding::new(
            RULE_STEPSTATE,
            SRC.as_str(),
            0,
            format!(
                "is_terminal/terminal_states inconsistent: {:?} vs {:?}",
                facts.is_terminal, facts.terminal_states
            ),
        ));
    }
}

fn check_non_terminal_consistency(facts: &StepStateFacts, report: &mut LaneReport) {
    let non_terminal_derived = facts.non_terminal_derived();
    if non_terminal_derived != facts.non_terminal_fn {
        report.push(Finding::new(
            RULE_STEPSTATE,
            SRC.as_str(),
            0,
            format!(
                "non_terminal_states inconsistent: {:?} vs {:?}",
                non_terminal_derived, facts.non_terminal_fn
            ),
        ));
    }
}

fn print_and_exit(report: &LaneReport) -> ExitCode {
    let rendered = report.render();
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    if report.is_clean() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}

fn find_char_in(text: &str, start: usize, target: char) -> Option<usize> {
    let rest = text.get(start..)?;
    rest.find(target).map(|off| start.saturating_add(off))
}

fn extract_enum_body(text: &str, enum_name: &str) -> Option<String> {
    let marker = format!("pub enum {enum_name}");
    let start = text.find(&marker)?;
    let open_pos = find_char_in(text, start, '{')?;
    balanced_block_from_open(text, open_pos)
}

fn balanced_block_from_open(text: &str, open_pos: usize) -> Option<String> {
    let mut depth: i32 = 0;
    let mut idx = open_pos;
    loop {
        let b = text.as_bytes().get(idx).copied()?;
        depth = next_depth(depth, b);
        if depth == 0 && b == b'}' {
            let end = idx.saturating_add(1);
            return text.get(open_pos..end).map(str::to_string);
        }
        idx = idx.saturating_add(1);
    }
}

fn next_depth(depth: i32, byte: u8) -> i32 {
    match byte {
        b'{' => depth.saturating_add(1),
        b'}' => depth.saturating_sub(1),
        _ => depth,
    }
}

fn extract_enum_variants(text: &str, enum_name: &str) -> StateSet {
    extract_enum_body(text, enum_name).map_or_else(StateSet::new, |body| {
        body.lines().filter_map(|line| variant_name(line, enum_name)).collect()
    })
}

fn variant_name(line: &str, enum_name: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
        return None;
    }
    let first_word = trimmed.split_whitespace().next()?;
    accepted_variant_name(first_word.trim_end_matches(',').trim_end_matches('('), enum_name)
}

fn accepted_variant_name(name: &str, enum_name: &str) -> Option<String> {
    let first = name.chars().next()?;
    let valid = !name.is_empty()
        && name != enum_name
        && first.is_ascii_uppercase()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_');
    if valid { Some(name.to_string()) } else { None }
}

fn extract_block_after(text: &str, marker: &str, end_marker: &str) -> Option<String> {
    let start = text.find(marker)?;
    let end = find_substr(text, start, end_marker)?;
    let end_inclusive = end.saturating_add(end_marker.len());
    text.get(start..end_inclusive).map(str::to_string)
}

fn find_substr(text: &str, start: usize, needle: &str) -> Option<usize> {
    let hay = text.get(start..)?;
    hay.find(needle).map(|off| start.saturating_add(off))
}

fn collect_stepstate_refs(text: &str) -> StateSet {
    text.match_indices("StepState::")
        .filter_map(|(start, needle)| stepstate_ref_at(text, start.saturating_add(needle.len())))
        .collect()
}

fn stepstate_ref_at(text: &str, start: usize) -> Option<String> {
    let name: String =
        text.get(start..)?.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    if name.is_empty() { None } else { Some(name) }
}

fn find_function_body(text: &str, fn_name: &str) -> Option<String> {
    function_patterns(fn_name).iter().find_map(|pat| body_after_pattern(text, pat))
}

fn function_patterns(fn_name: &str) -> [String; 4] {
    [
        format!("pub fn {fn_name}("),
        format!("pub fn {fn_name}<"),
        format!("fn {fn_name}("),
        format!("fn {fn_name}<"),
    ]
}

fn body_after_pattern(text: &str, pattern: &str) -> Option<String> {
    let start = text.find(pattern)?;
    let open_pos = find_char_in(text, start, '{')?;
    balanced_block_from_open(text, open_pos)
}
