use std::{collections::BTreeSet, fs, path::Path};

pub(super) fn load_allow_file(root: &Path) -> Result<BTreeSet<(String, String)>, String> {
    let allow_path = root.join("scripts/hot-cold-forbidden-apis.allow");
    if !allow_path.exists() {
        eprintln!("NotApplicable: hot/cold forbidden API allow file absent");
        return Ok(BTreeSet::new());
    }
    let text = fs::read_to_string(&allow_path)
        .map_err(|error| format!("scripts/hot-cold-forbidden-apis.allow: unreadable: {error}"))?;
    text.lines().enumerate().try_fold(BTreeSet::new(), collect_allow_entry)
}

fn collect_allow_entry(
    mut acc: BTreeSet<(String, String)>,
    (index, line): (usize, &str),
) -> Result<BTreeSet<(String, String)>, String> {
    if let Some(entry) = parse_allow_entry(index, line)? {
        acc.insert(entry);
    }
    Ok(acc)
}

fn parse_allow_entry(index: usize, line: &str) -> Result<Option<(String, String)>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }
    let parts: Vec<&str> = trimmed.split('|').collect();
    let [path, class, owner, reviewed_by, test, reason] = parts.as_slice() else {
        return Err(malformed_allow(
            index,
            "expected path|class|owner=...|reviewed_by=...|test=...|reason=...",
        ));
    };
    let entry = AllowEntry { index, path, class, owner, reviewed_by, test, reason };
    validate_allow_entry(&entry)?;
    Ok(Some((entry.path.to_owned(), entry.class.to_owned())))
}

struct AllowEntry<'a> {
    index: usize,
    path: &'a str,
    class: &'a str,
    owner: &'a str,
    reviewed_by: &'a str,
    test: &'a str,
    reason: &'a str,
}

fn validate_allow_entry(entry: &AllowEntry<'_>) -> Result<(), String> {
    if entry.path.contains('*')
        || !entry.path.starts_with("crates/")
        || !entry.path.ends_with(".rs")
    {
        return Err(overbroad_allow(entry.index, "path must be exact crates/*/src/*.rs"));
    }
    if entry.class == "ALL" || entry.class.contains('*') {
        return Err(overbroad_allow(entry.index, "class must be exact"));
    }
    if !entry.owner.starts_with("owner=")
        || !entry.reviewed_by.starts_with("reviewed_by=")
        || !entry.test.starts_with("test=")
        || !entry.reason.starts_with("reason=")
    {
        return Err(malformed_allow(entry.index, "missing owner/reviewed_by/test/reason"));
    }
    Ok(())
}

fn malformed_allow(index: usize, detail: &str) -> String {
    format!(
        "MalformedException: scripts/hot-cold-forbidden-apis.allow:{} {detail}",
        index.saturating_add(1)
    )
}

fn overbroad_allow(index: usize, detail: &str) -> String {
    format!(
        "OverbroadException: scripts/hot-cold-forbidden-apis.allow:{} {detail}",
        index.saturating_add(1)
    )
}
