use std::{collections::BTreeSet, path::Path};

use titania_lanes::{Finding, LaneReport};

use super::{
    claims::{Claim, ClaimClass, parse_claims, resolve_range},
    identifiers::{extract_identifiers, filter_noise_words},
};

pub(crate) fn per_mirror_pass(root: &Path, mirror_dir: &str, report: &mut LaneReport) {
    let dir = root.join(mirror_dir);
    let Ok(read) = std::fs::read_dir(&dir) else {
        return;
    };
    read.filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_rust_file(path))
        .for_each(|path| scan_mirror_file(root, &path, report));
}

fn scan_mirror_file(root: &Path, path: &Path, report: &mut LaneReport) {
    let rel = rel_path(root, path);
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let claims = parse_claims(&text);
    if claims.is_empty() {
        report_missing_header(&rel, report);
        return;
    }
    let master_dir = master_dir(&claims);
    let mirror_ids = filter_noise_words(extract_identifiers(&text));
    let mut context =
        MirrorContext { root, rel: &rel, master_dir: &master_dir, mirror_ids: &mirror_ids, report };
    claims
        .iter()
        .filter(|claim| matches!(claim.klass, ClaimClass::Claim))
        .for_each(|claim| check_claim(&mut context, claim));
}

struct MirrorContext<'a> {
    root: &'a Path,
    rel: &'a str,
    master_dir: &'a str,
    mirror_ids: &'a BTreeSet<String>,
    report: &'a mut LaneReport,
}

fn check_claim(context: &mut MirrorContext<'_>, claim: &Claim) {
    let (resolved, start, end, ok) = resolve_range(&claim.range, context.master_dir, context.root);
    if !ok {
        report_unresolvable(context.rel, &claim.range, start, end, context.report);
        return;
    }
    let Ok(prod_text) = std::fs::read_to_string(context.root.join(&resolved)) else {
        report_missing_source(context.rel, &resolved, context.report);
        return;
    };
    let prod_ids =
        filter_noise_words(extract_identifiers(&production_slice(&prod_text, start, end)));
    let missing: BTreeSet<&String> = prod_ids.difference(context.mirror_ids).collect();
    report_missing_ids(context.rel, &resolved, &missing, context.report);
}

fn production_slice(text: &str, start: usize, end: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let slice_start = start.saturating_sub(1).min(total);
    let slice_end = end.min(total);
    lines.get(slice_start..slice_end).map_or_else(String::new, |lines| lines.join("\n"))
}

fn report_missing_ids(
    rel: &str,
    resolved: &str,
    missing: &BTreeSet<&String>,
    report: &mut LaneReport,
) {
    if missing.is_empty() {
        return;
    }
    let mut list: Vec<String> = missing.iter().map(|name| (*name).clone()).collect();
    list.sort();
    report.push(Finding::new(
        "DRIFT",
        rel.to_owned(),
        0,
        format!("missing identifiers in {resolved}: {}", list.join(",")),
    ));
}

fn master_dir(claims: &[Claim]) -> String {
    claims
        .iter()
        .find_map(|claim| matches!(claim.klass, ClaimClass::Master).then(|| claim.range.clone()))
        .as_deref()
        .and_then(|master| master.rfind('/').and_then(|idx| master.get(..idx)))
        .map_or_else(|| ".".to_string(), str::to_string)
}

fn report_missing_header(rel: &str, report: &mut LaneReport) {
    report.push(Finding::new(
        "DRIFT",
        rel.to_owned(),
        0,
        "no claimed production source range found in header".to_string(),
    ));
}

fn report_unresolvable(rel: &str, range: &str, start: usize, end: usize, report: &mut LaneReport) {
    report.push(Finding::new(
        "DRIFT",
        rel.to_owned(),
        0,
        format!("unresolvable range {range} (start={start} end={end})"),
    ));
}

fn report_missing_source(rel: &str, resolved: &str, report: &mut LaneReport) {
    report.push(Finding::new(
        "DRIFT",
        rel.to_owned(),
        0,
        format!("production source missing: {resolved}"),
    ));
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).map_or_else(
        |_| path.to_string_lossy().into_owned(),
        |rel| rel.to_string_lossy().replace('\\', "/"),
    )
}

fn is_rust_file(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs")
}
