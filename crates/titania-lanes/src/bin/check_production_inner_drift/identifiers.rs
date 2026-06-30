use std::collections::BTreeSet;

const NOISE_WORDS: &[&str] = &[
    "crate",
    "derive",
    "non_exhaustive",
    "must_use",
    "repr",
    "error",
    "from",
    "allow",
    "forbid",
    "deny",
    "warn",
    "line",
    "lines",
    "verbatim",
    "byte",
    "preserved",
    "removed",
    "declared",
    "production",
    "mirror",
    "stub",
    "local",
    "header",
    "note",
    "comment",
    "doc",
    "string",
    "section",
    "substitution",
    "variant",
    "discriminant",
    "block",
    "fn_name",
    "impl_block",
    "the",
    "and",
    "for",
    "with",
    "that",
    "this",
    "of",
    "to",
    "or",
    "is",
    "as",
    "by",
    "on",
    "at",
    "are",
    "be",
    "it",
    "an",
    "see",
    "via",
    "per",
    "all",
    "any",
    "each",
    "their",
    "these",
];

const DROPPED_KEYWORDS: &[&str] = &[
    "pub",
    "fn",
    "impl",
    "struct",
    "enum",
    "match",
    "use",
    "mod",
    "self",
    "Self",
    "return",
    "let",
    "const",
    "static",
    "where",
    "for",
    "in",
    "if",
    "else",
    "while",
    "loop",
    "break",
    "continue",
    "true",
    "false",
    "None",
    "Some",
    "Ok",
    "Err",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "bool",
    "str",
    "String",
    "Vec",
    "Box",
    "Option",
    "Result",
    "HashSet",
    "HashMap",
    "BTreeSet",
    "BTreeMap",
    "PhantomData",
    "Default",
    "copy",
    "clone",
    "debug",
];

pub(crate) fn extract_identifiers(text: &str) -> BTreeSet<String> {
    collect_identifiers(strip_noise(text).chars(), 3, |token| {
        !is_dropped_keyword(token) && !is_pure_screaming_short(token) && !is_pure_lowercase(token)
    })
}

pub(crate) fn filter_noise_words(set: BTreeSet<String>) -> BTreeSet<String> {
    set.into_iter().filter(|s| !NOISE_WORDS.contains(&s.as_str())).collect()
}

pub(crate) fn extract_id_extern(text: &str) -> BTreeSet<String> {
    collect_identifiers(text.chars(), 2, |token| !is_dropped_keyword(token))
}

pub(crate) fn candidate_tokens(name: &str) -> Vec<String> {
    let last = name.rsplit("::").next().map_or(name, |value| value);
    let mut out: Vec<String> = vec![last.to_string()];
    ["Mirror", "Spec", "production_", "spec_"]
        .into_iter()
        .filter_map(|prefix| last.strip_prefix(prefix))
        .map(str::to_string)
        .for_each(|token| out.push(token));
    ["_pure", "_decision"]
        .into_iter()
        .filter_map(|suffix| last.strip_suffix(suffix))
        .map(str::to_string)
        .for_each(|token| out.push(token));
    out
}

fn collect_identifiers(
    chars: impl Iterator<Item = char>,
    min_len: usize,
    keep: impl Fn(&str) -> bool,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut current = String::new();
    for ch in chars {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            flush_identifier(&mut current, min_len, &keep, &mut out);
        }
    }
    flush_identifier(&mut current, min_len, &keep, &mut out);
    out
}

fn flush_identifier(
    current: &mut String,
    min_len: usize,
    keep: &impl Fn(&str) -> bool,
    out: &mut BTreeSet<String>,
) {
    if current.len() >= min_len && keep(current) {
        out.insert(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn strip_noise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    text.lines().for_each(|line| {
        let stripped = line.find("//").and_then(|idx| line.get(..idx)).map_or(line, |value| value);
        out.push_str(stripped);
        out.push('\n');
    });
    out.replace("pub(crate)", "pub").replace("#![from]", "(")
}

fn is_pure_lowercase(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_lowercase() || c == '_')
}

fn is_pure_screaming_short(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_uppercase() || c == '_') && s.len() < 7
}

fn is_dropped_keyword(s: &str) -> bool {
    DROPPED_KEYWORDS.contains(&s)
}
