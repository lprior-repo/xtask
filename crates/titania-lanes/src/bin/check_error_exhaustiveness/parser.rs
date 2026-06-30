use std::collections::BTreeSet;

pub(super) fn extract_enum_variants(text: &str, name: &str) -> BTreeSet<String> {
    let Some(body) = extract_enum_body(text, name) else {
        return BTreeSet::new();
    };
    body.lines().filter_map(|line| extract_variant(line, name)).collect()
}

pub(super) fn find_function_body(text: &str, fn_name: &str) -> Option<String> {
    function_patterns(fn_name).iter().find_map(|pattern| {
        text.find(pattern.as_str()).and_then(|start| balanced_item(text, start))
    })
}

pub(super) fn collect_qualified_refs(text: &str, type_name: &str) -> BTreeSet<String> {
    let needle = format!("{type_name}::");
    let mut out = BTreeSet::new();
    let mut cursor = 0;
    while let Some((name, next_cursor)) = next_qualified_ref(text, &needle, cursor) {
        out.insert(name);
        cursor = next_cursor;
    }
    out
}

fn extract_enum_body(text: &str, name: &str) -> Option<String> {
    let marker = format!("pub enum {name}");
    text.find(&marker).and_then(|start| balanced_item(text, start))
}

fn balanced_item(text: &str, start: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let mut depth: i32 = 0;
    let mut started = false;
    let mut cursor = start;
    while let Some(&byte) = bytes.get(cursor) {
        if consume_body_byte(byte, &mut depth, &mut started) {
            return text.get(start..cursor.saturating_add(1)).map(str::to_string);
        }
        cursor = cursor.saturating_add(1);
    }
    None
}

fn consume_body_byte(byte: u8, depth: &mut i32, started: &mut bool) -> bool {
    if byte == b'{' {
        *depth = depth.saturating_add(1);
        *started = true;
    } else if byte == b'}' {
        *depth = depth.saturating_sub(1);
        return *started && *depth == 0;
    }
    false
}

fn extract_variant(line: &str, enum_name: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
        return None;
    }
    let first_word = trimmed.split_whitespace().next()?;
    let name = first_word.trim_end_matches(',').trim_end_matches('(');
    valid_variant_name(name, enum_name).then(|| name.to_string())
}

fn valid_variant_name(name: &str, enum_name: &str) -> bool {
    !name.is_empty()
        && name != enum_name
        && name.chars().next().is_some_and(|first| first.is_ascii_uppercase())
        && name.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
}

fn function_patterns(fn_name: &str) -> [String; 4] {
    [
        format!("pub fn {fn_name}("),
        format!("pub fn {fn_name}<"),
        format!("fn {fn_name}("),
        format!("fn {fn_name}<"),
    ]
}

fn next_qualified_ref(text: &str, needle: &str, cursor: usize) -> Option<(String, usize)> {
    let tail = text.get(cursor..)?;
    let offset = tail.find(needle)?;
    let name_start = cursor.saturating_add(offset).saturating_add(needle.len());
    let name = ref_name_at(text, name_start)?;
    let next_cursor = name_start.saturating_add(name.len());
    Some((name, next_cursor))
}

fn ref_name_at(text: &str, name_start: usize) -> Option<String> {
    let name: String = text
        .get(name_start..)?
        .chars()
        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}
