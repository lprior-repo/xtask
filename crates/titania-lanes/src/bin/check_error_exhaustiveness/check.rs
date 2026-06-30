use std::{collections::BTreeSet, fs};

use titania_core::TargetProject;
use titania_lanes::{Finding, LaneReport};

use crate::{
    model::{CHECKS, Check, DomainFile, Oracle, TargetRelativePath},
    parser::{collect_qualified_refs, extract_enum_variants, find_function_body},
};

pub(super) fn run(target: &TargetProject, report: &mut LaneReport) {
    CHECKS.iter().for_each(|check| run_check(target, check, report));
}

fn run_check(target: &TargetProject, check: &Check, report: &mut LaneReport) {
    let Some(variants) = enum_variants(target, check, report) else {
        return;
    };
    check.oracles.iter().for_each(|oracle| check_oracle(target, check, oracle, &variants, report));
}

fn enum_variants(
    target: &TargetProject,
    check: &Check,
    report: &mut LaneReport,
) -> Option<BTreeSet<String>> {
    let text = match read_optional_file(target, check.enum_path) {
        DomainFile::Present(text) => text,
        DomainFile::Absent => return note_not_applicable(check),
        DomainFile::Unreadable(kind) => {
            push(report, check.enum_path.as_str(), format!("enum file not readable: {kind:?}"));
            return None;
        }
    };
    let variants = extract_enum_variants(&text, check.type_name);
    if variants.is_empty() {
        push(
            report,
            check.enum_path.as_str(),
            format!("no variants parsed for {}", check.type_name),
        );
        None
    } else {
        Some(variants)
    }
}

fn note_not_applicable(check: &Check) -> Option<BTreeSet<String>> {
    eprintln!(
        "[check-error-exhaustiveness] not applicable: {} absent; skipping {} ({}) exhaustiveness",
        check.enum_path.as_str(),
        check.type_name,
        check.domain_label
    );
    None
}

fn check_oracle(
    target: &TargetProject,
    check: &Check,
    oracle: &Oracle,
    variants: &BTreeSet<String>,
    report: &mut LaneReport,
) {
    let Some(body) = oracle_body(target, oracle, report) else {
        return;
    };
    let mentions = collect_qualified_refs(&body, check.type_name);
    let missing = missing_variants(variants, &mentions);
    if missing.is_empty() {
        print_ok(check, oracle, variants.len());
    } else {
        push_missing(report, check, oracle, missing);
    }
}

fn oracle_body(target: &TargetProject, oracle: &Oracle, report: &mut LaneReport) -> Option<String> {
    let abs = oracle.path.in_target(target);
    let Ok(text) = fs::read_to_string(&abs) else {
        push(report, oracle.path.as_str(), format!("oracle {} file not readable", oracle.function));
        return None;
    };
    match find_function_body(&text, oracle.function) {
        Some(body) => Some(body),
        None => {
            push(report, oracle.path.as_str(), format!("function {} not found", oracle.function));
            None
        }
    }
}

fn missing_variants(variants: &BTreeSet<String>, mentions: &BTreeSet<String>) -> Vec<String> {
    variants.iter().filter(|variant| !mentions.contains(*variant)).cloned().collect()
}

fn push_missing(report: &mut LaneReport, check: &Check, oracle: &Oracle, mut missing: Vec<String>) {
    missing.sort();
    push(
        report,
        oracle.path.as_str(),
        format!("{} missing {}: {}", check.type_name, oracle.function, missing.join(",")),
    );
}

fn print_ok(check: &Check, oracle: &Oracle, variant_count: usize) {
    eprintln!(
        "  OK {} in {}::{} ({} variants)",
        check.type_name,
        oracle.path.as_str(),
        oracle.function,
        variant_count
    );
}

fn read_optional_file(target: &TargetProject, rel: TargetRelativePath) -> DomainFile {
    match fs::read_to_string(rel.in_target(target)) {
        Ok(text) => DomainFile::Present(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DomainFile::Absent,
        Err(error) => DomainFile::Unreadable(error.kind()),
    }
}

fn push(report: &mut LaneReport, path: &'static str, message: String) {
    report.push(Finding::new("EXHAUST", path, 0, message));
}
