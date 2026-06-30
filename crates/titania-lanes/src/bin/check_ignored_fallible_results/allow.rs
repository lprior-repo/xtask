use std::{collections::BTreeMap, path::Path};

use titania_lanes::{Finding, LaneReport, helpers::line_no_from_idx};

const ALLOW_FILE: &str = "scripts/ignored-fallible-results.allow";

pub(super) fn load_allow(root: &Path, report: &mut LaneReport) -> BTreeMap<String, String> {
    let path = root.join(ALLOW_FILE);
    if !path.is_file() {
        return BTreeMap::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_allow_text(&text, report),
        Err(_error) => BTreeMap::new(),
    }
}

fn parse_allow_text(text: &str, report: &mut LaneReport) -> BTreeMap<String, String> {
    text.lines().enumerate().fold(BTreeMap::new(), |mut map, (idx, raw)| {
        if let Some((path, class)) = parse_allow_row(idx, raw, report) {
            map.insert(format!("{path}|{class}"), path);
        }
        map
    })
}

fn parse_allow_row(idx: usize, raw: &str, report: &mut LaneReport) -> Option<(String, String)> {
    let line_no = line_no_from_idx(idx);
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let parts: Vec<&str> = line.split('|').collect();
    let [path, class, owner, expiry, follow_up, reason, ..] = parts.as_slice() else {
        push_allow_finding(report, line_no, "malformed allow row");
        return None;
    };
    let row = AllowRow { line_no, path, class, owner, expiry, follow_up, reason };
    validate_allow_row(report, &row).then(|| ((*row.path).to_owned(), (*row.class).to_owned()))
}

struct AllowRow<'a> {
    line_no: u32,
    path: &'a str,
    class: &'a str,
    owner: &'a str,
    expiry: &'a str,
    follow_up: &'a str,
    reason: &'a str,
}

fn validate_allow_row(report: &mut LaneReport, row: &AllowRow<'_>) -> bool {
    if row.path.contains('*') || row.path.starts_with('/') {
        push_allow_finding(report, row.line_no, "overbroad path");
        return false;
    }
    if row.class == "*" || row.class == "ALL" || !row.class.starts_with("DISCARD-") {
        push_allow_finding(report, row.line_no, "overbroad class");
        return false;
    }
    validate_metadata(report, row)
}

fn validate_metadata(report: &mut LaneReport, row: &AllowRow<'_>) -> bool {
    let valid = row.owner.starts_with("owner=")
        && row.expiry.starts_with("expiry=")
        && row.follow_up.starts_with("follow_up=")
        && row.reason.starts_with("reason=");
    if !valid {
        push_allow_finding(report, row.line_no, "malformed owner/expiry/follow_up/reason");
    }
    valid
}

fn push_allow_finding(report: &mut LaneReport, line_no: u32, message: &'static str) {
    report.push(Finding::new("ALLOW-FILE", format!("{ALLOW_FILE}:{line_no}"), line_no, message));
}
