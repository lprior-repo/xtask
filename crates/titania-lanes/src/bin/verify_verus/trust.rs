use std::{fs, path::Path};

use titania_core::TargetProject;
use titania_lanes::{Finding, LaneReport};

use super::walk::walk_rs_lines;

pub(crate) const EXTERNAL_RULE: &str = "VERUS-EXTERNAL-001";
pub(crate) const TRUSTED_BASE_WAIVER_FILE: &str = "trusted-base-waivers.txt";

const FORBIDDEN_RULE: &str = "FORBIDDEN-ASSUME";

#[must_use]
pub(crate) fn trusted_base_waiver_exists(evidence_dir: &Path) -> bool {
    fs::metadata(evidence_dir.join(TRUSTED_BASE_WAIVER_FILE))
        .is_ok_and(|meta| meta.is_file() && meta.len() != 0)
}

#[must_use]
pub(crate) fn scan_forbidden_trust(target: &TargetProject, report: &mut LaneReport) -> Vec<String> {
    let mut findings = Vec::new();
    trust_scan_roots(target).iter().for_each(|dir| {
        walk_rs_lines(dir, target.as_std_path(), |line, path, line_no| {
            if is_forbidden_trust_line(line) {
                findings.push(format!("{path}:{line_no}: {line}"));
                report.push(Finding::new(
                    FORBIDDEN_RULE,
                    path.to_owned(),
                    line_no,
                    "forbidden `assume(` or `axiom` outside comments",
                ));
            }
        });
    });
    findings
}

#[must_use]
pub(crate) fn scan_external_markers(target: &TargetProject) -> Vec<String> {
    let mut findings = Vec::new();
    trust_scan_roots(target).iter().for_each(|dir| {
        walk_rs_lines(dir, target.as_std_path(), |line, path, line_no| {
            if is_external_marker_line(line) {
                findings.push(format!("{path}:{line_no}: {line}"));
            }
        });
    });
    findings
}

pub(crate) fn report_unwaived_external_markers(report: &mut LaneReport, lines: &[String]) {
    lines.iter().for_each(|line| {
        let (path, line_no) = parse_finding_location(line);
        report.push(Finding::new(
            EXTERNAL_RULE,
            path,
            line_no,
            "Verus external marker requires explicit trusted-base waiver artifact",
        ));
    });
}

fn trust_scan_roots(target: &TargetProject) -> [std::path::PathBuf; 2] {
    [target.as_std_path().join("verification/verus"), target.as_std_path().join("contracts/verus")]
}

fn is_forbidden_trust_line(line: &str) -> bool {
    !is_rust_comment(line)
        && (line.contains("assume(") || line.contains("axiom ") || line.contains("axiom\t"))
}

fn is_external_marker_line(line: &str) -> bool {
    !is_rust_comment(line)
        && (line.contains("#[verifier::external_body]") || line.contains("#[verifier::external]"))
}

fn is_rust_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

fn parse_finding_location(line: &str) -> (String, u32) {
    let mut parts = line.splitn(3, ':');
    let path = parts.next().map_or_else(String::new, str::to_owned);
    let line_no = parts.next().and_then(|s| s.parse::<u32>().ok()).map_or(0, |n| n);
    (path, line_no)
}
