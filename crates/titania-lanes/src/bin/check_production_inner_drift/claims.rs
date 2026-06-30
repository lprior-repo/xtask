use std::path::Path;

#[derive(Debug)]
pub(crate) enum ClaimClass {
    Master,
    Claim,
    Skip,
}

#[derive(Debug)]
pub(crate) struct Claim {
    pub(crate) klass: ClaimClass,
    pub(crate) range: String,
}

pub(crate) fn parse_claims(text: &str) -> Vec<Claim> {
    let mut out: Vec<Claim> = Vec::new();
    let mut master_emitted = false;
    let mut drift_pending = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if emit_pending_drift(trimmed, &mut master_emitted, &mut drift_pending, &mut out) {
            continue;
        }
        if emit_drift_policy(trimmed, &mut master_emitted, &mut drift_pending, &mut out) {
            continue;
        }
        if let Some(path) = extract_claim_path(trimmed) {
            let klass =
                if trimmed.contains("REMOVED:") { ClaimClass::Skip } else { ClaimClass::Claim };
            out.push(Claim { klass, range: path });
        }
    }
    out
}

pub(crate) fn resolve_range(
    range: &str,
    master_dir: &str,
    root: &Path,
) -> (String, usize, usize, bool) {
    let (path_part, range_part) = range.split_once(':').map_or((range, ""), |parts| parts);
    let resolved_path = if path_part.contains('/') || master_dir.is_empty() || master_dir == "." {
        path_part.to_string()
    } else {
        format!("{master_dir}/{path_part}")
    };
    let abs = root.join(&resolved_path);
    if !abs.is_file() {
        return (resolved_path, 0, 0, false);
    }
    let Ok(text) = std::fs::read_to_string(&abs) else {
        return (resolved_path, 0, 0, false);
    };
    let total = text.lines().count();
    let (start, end) = parse_range(range_part, total);
    if end > total || start < 1 || start > end {
        return (resolved_path, start, end, false);
    }
    (resolved_path, start, end, true)
}

fn emit_pending_drift(
    trimmed: &str,
    master_emitted: &mut bool,
    drift_pending: &mut bool,
    out: &mut Vec<Claim>,
) -> bool {
    if !*drift_pending || !trimmed.starts_with("//") {
        return false;
    }
    if let Some(path) = extract_backtick_path(trimmed) {
        push_claim(out, master_emitted, path);
        *drift_pending = false;
        return true;
    }
    false
}

fn emit_drift_policy(
    trimmed: &str,
    master_emitted: &mut bool,
    drift_pending: &mut bool,
    out: &mut Vec<Claim>,
) -> bool {
    if !trimmed.starts_with("// DRIFT POLICY:") {
        return false;
    }
    if let Some(path) = extract_backtick_path(trimmed) {
        push_claim(out, master_emitted, path);
    } else {
        *drift_pending = true;
    }
    true
}

fn push_claim(out: &mut Vec<Claim>, master_emitted: &mut bool, range: String) {
    if *master_emitted {
        out.push(Claim { klass: ClaimClass::Claim, range });
    } else {
        out.push(Claim { klass: ClaimClass::Master, range });
        *master_emitted = true;
    }
}

fn extract_backtick_path(line: &str) -> Option<String> {
    let start = line.find('`')?;
    let rest = line.get(start.saturating_add(1)..)?;
    let end = rest.find('`')?;
    rest.get(..end).map(str::to_string)
}

fn extract_claim_path(line: &str) -> Option<String> {
    let after_marker = claim_marker_tail(line)?;
    extract_backtick_path(after_marker).or_else(|| extract_verbatim_path(after_marker))
}

fn claim_marker_tail(line: &str) -> Option<&str> {
    [
        "Production source:",
        "REMOVED:",
        "SUBSTITUTED:",
        "Source:",
        "Production",
        "VERBATIM PRODUCTION:",
    ]
    .into_iter()
    .find_map(|marker| {
        line.find(marker).and_then(|pos| line.get(pos.saturating_add(marker.len())..))
    })
}

fn extract_verbatim_path(after_marker: &str) -> Option<String> {
    let trimmed = after_marker.trim();
    let colon = trimmed.find(':')?;
    let head = trimmed.get(..colon)?;
    let valid = !head.is_empty()
        && head.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '/' || c == '.');
    valid.then(|| trimmed.to_string())
}

fn parse_range(range_part: &str, total: usize) -> (usize, usize) {
    if range_part.is_empty() {
        return (1, total);
    }
    let mut parts = range_part.splitn(2, '-');
    let start = parts.next().and_then(|s| s.parse::<usize>().ok()).map_or(1, |value| value);
    let end = parts.next().and_then(|s| s.parse::<usize>().ok()).map_or(total, |value| value);
    (start, end)
}
