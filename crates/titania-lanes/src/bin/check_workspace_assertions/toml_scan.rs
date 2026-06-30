use std::collections::BTreeSet;

pub(super) fn quoted_values_in_line(line: &str) -> Vec<String> {
    line.split('"')
        .enumerate()
        .filter(|&(index, _)| index % 2 == 1)
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Locate the `[` that opens a TOML array for `key`. Tolerates arbitrary
/// whitespace, including compact arrays such as `members=["crates/foo"]`.
pub(super) fn array_open_after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let bytes = text.as_bytes();
    let key_bytes = key.as_bytes();
    let mut start = 0;
    while let Some(rel) = find_subslice(bytes.get(start..)?, key_bytes) {
        let match_start = start.saturating_add(rel);
        let key_end = match_start.saturating_add(key_bytes.len());
        if !is_line_key_start(bytes, match_start) {
            start = key_end;
            continue;
        }
        let Some(after_bracket) = bracket_after_key(bytes, key_end) else {
            start = key_end;
            continue;
        };
        return text.get(after_bracket..);
    }
    None
}

fn is_line_key_start(bytes: &[u8], match_start: usize) -> bool {
    bytes
        .get(..match_start)
        .and_then(|prefix| prefix.iter().rev().find(|byte| **byte != b' ' && **byte != b'\t'))
        .is_none_or(|byte| *byte == b'\n' || *byte == b'\r')
}

fn bracket_after_key(bytes: &[u8], key_end: usize) -> Option<usize> {
    let mut pos = skip_ascii_whitespace(bytes, key_end);
    if bytes.get(pos) != Some(&b'=') {
        return None;
    }
    pos = skip_ascii_whitespace(bytes, pos.saturating_add(1));
    if bytes.get(pos) == Some(&b'[') { Some(pos.saturating_add(1)) } else { None }
}

fn skip_ascii_whitespace(bytes: &[u8], pos: usize) -> usize {
    bytes
        .get(pos..)
        .and_then(|tail| tail.iter().position(|byte| !byte.is_ascii_whitespace()))
        .map_or(bytes.len(), |offset| pos.saturating_add(offset))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| window == needle)
}

pub(super) fn quoted_array_values(text: &str, key: &str) -> BTreeSet<String> {
    let Some(after_key) = array_open_after(text, key) else {
        return BTreeSet::new();
    };
    let Some(end) = after_key.find(']') else {
        return BTreeSet::new();
    };
    after_key
        .get(..end)
        .into_iter()
        .flat_map(str::lines)
        .flat_map(|line| quoted_values_in_line(line).into_iter())
        .filter(|value| !value.is_empty())
        .collect()
}

pub(super) fn package_name(manifest: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let after = trimmed.strip_prefix("name")?;
        let after = after.trim_start().strip_prefix('=')?;
        let value = after.trim_start().strip_prefix('"')?;
        Some(value.split_once('"')?.0.to_owned())
    })
}

pub(super) fn named_table_values(manifest: &str, table: &str) -> BTreeSet<String> {
    let mut in_table = false;
    let mut names = BTreeSet::new();
    manifest.lines().for_each(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_table = trimmed == table;
            return;
        }
        if in_table && trimmed.contains('=') {
            if let Some((name, _rest)) = trimmed.split_once('=') {
                let cleaned = name.trim();
                if !cleaned.is_empty() {
                    names.insert(cleaned.to_owned());
                }
            }
        }
    });
    names
}

pub(super) fn binary_names(manifest: &str) -> BTreeSet<String> {
    let mut in_bin = false;
    let mut names = BTreeSet::new();
    manifest.lines().for_each(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_bin = trimmed.starts_with("[[bin]]") || trimmed == "[[bin]]";
            return;
        }
        if in_bin && trimmed.starts_with("name") {
            if let Some((_key, value)) = trimmed.split_once('=') {
                let cleaned = value.trim().trim_matches('"');
                if !cleaned.is_empty() {
                    names.insert(cleaned.to_owned());
                }
            }
        }
    });
    names
}

#[cfg(test)]
mod tests {
    use super::{array_open_after, quoted_array_values};

    #[test]
    fn array_open_after_handles_single_space() {
        let text = "[workspace]\nmembers = [\n    \"a\",\n    \"b\",\n]\n";
        assert_eq!(array_open_after(text, "members"), Some("\n    \"a\",\n    \"b\",\n]\n"));
    }

    #[test]
    fn array_open_after_handles_double_space_around_eq() {
        let text = "[workspace]\nmembers  =  [\n    \"a\",\n]\n";
        assert_eq!(array_open_after(text, "members"), Some("\n    \"a\",\n]\n"));
    }

    #[test]
    fn array_open_after_handles_leading_indent() {
        let text = "[workspace]\n    members = [ \"a\", \"b\" ]\n";
        assert_eq!(array_open_after(text, "members"), Some(" \"a\", \"b\" ]\n"));
    }

    #[test]
    fn array_open_after_returns_none_for_missing_key() {
        let text = "[workspace]\nresolver = \"2\"\n";
        assert!(array_open_after(text, "members").is_none());
    }

    #[test]
    fn quoted_array_values_tolerates_double_space() {
        let text = "[workspace]\nmembers  = [\"crates/x\", \"crates/y\"]\n";
        let set = quoted_array_values(text, "members");
        assert!(set.contains("crates/x"));
        assert!(set.contains("crates/y"));
    }

    #[test]
    fn quoted_array_values_accepts_compact_member_array() {
        let text = "[workspace]\nmembers=[\"crates/foo\"]\n";
        let set = quoted_array_values(text, "members");
        assert!(set.contains("crates/foo"));
    }
}
