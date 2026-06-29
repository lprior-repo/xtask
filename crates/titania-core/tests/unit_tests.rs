//! Unit tests for `titania-core` primitives.
//!
//! These live in `tests/` (rather than `#[cfg(test)] mod tests` in
//! `src/`) so that the Holzman rg gate — which excludes `tests/` — does
//! not flag `assert!`/`assert_eq!` calls. The tests exercise the
//! `pub` API surface end-to-end.

#![allow(clippy::as_conversions)] // ASCII byte round-trip is exact in tests.

use titania_core::{
    Digest, DigestError, RuleId, RuleIdError, TextRange, TextRangeError, WorkspacePath,
    WorkspacePathError,
};

const DIGEST_HEX_LEN: usize = 64;

#[inline]
const fn is_lower_hex(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f')
}

// ====================================================================
// Digest
// ====================================================================

#[test]
fn digest_from_bytes_is_deterministic() {
    let a = Digest::from_bytes(b"hello world");
    let b = Digest::from_bytes(b"hello world");
    assert_eq!(a, b);
}

#[test]
fn digest_from_bytes_is_64_lowercase_hex() {
    let d = Digest::from_bytes(b"hello world");
    assert_eq!(d.as_hex().len(), DIGEST_HEX_LEN);
    assert!(d.as_hex().bytes().all(is_lower_hex), "expected only lowercase hex, got {d}");
}

#[test]
fn digest_from_hex_rejects_wrong_length() {
    assert_eq!(Digest::from_hex("ab"), Err(DigestError::WrongLength(2)));
    assert_eq!(
        Digest::from_hex(&"a".repeat(DIGEST_HEX_LEN + 1)),
        Err(DigestError::WrongLength(DIGEST_HEX_LEN + 1))
    );
    assert_eq!(Digest::from_hex(""), Err(DigestError::WrongLength(0)));
    assert_eq!(
        Digest::from_hex(&"a".repeat(DIGEST_HEX_LEN - 1)),
        Err(DigestError::WrongLength(DIGEST_HEX_LEN - 1))
    );
}

#[test]
fn digest_from_hex_rejects_uppercase_at_each_position() {
    for pos in [0usize, 1, 31, 32, 63] {
        let mut s: Vec<u8> = vec![b'0'; DIGEST_HEX_LEN];
        s[pos] = b'A';
        let result = Digest::from_hex(std::str::from_utf8(&s).unwrap());
        assert_eq!(result, Err(DigestError::NonHexChar(pos)), "pos={pos}");
    }
}

#[test]
fn digest_from_hex_rejects_non_hex_at_each_position() {
    for pos in [0usize, 1, 31, 32, 63] {
        let mut s: Vec<u8> = vec![b'0'; DIGEST_HEX_LEN];
        s[pos] = b'g';
        let result = Digest::from_hex(std::str::from_utf8(&s).unwrap());
        assert_eq!(result, Err(DigestError::NonHexChar(pos)), "pos={pos}");
    }
}

#[test]
fn digest_from_hex_accepts_all_valid_chars() {
    let valid = "0123456789abcdef".repeat(4);
    let d = Digest::from_hex(&valid).unwrap();
    assert_eq!(d.as_hex(), valid);
}

#[test]
fn digest_display_matches_as_hex() {
    let d = Digest::from_bytes(b"abc");
    assert_eq!(format!("{d}"), d.as_hex());
}

#[test]
fn digest_debug_format_is_stable() {
    let d = Digest::from_bytes(b"abc");
    let s = format!("{d:?}");
    assert!(s.starts_with("Digest("));
    assert!(s.ends_with(')'));
}

#[test]
fn digest_fromstr_round_trip() {
    let d = Digest::from_bytes(b"abc");
    let s = d.to_string();
    let parsed: Digest = s.parse().unwrap();
    assert_eq!(d, parsed);
}

#[test]
#[allow(clippy::type_complexity)]
fn digest_distinct_inputs_produce_distinct_digests_sampled() {
    type BytePair = (&'static [u8], &'static [u8]);
    let pairs: &[BytePair] =
        &[(b"alpha", b"bravo"), (b"", b"\0"), (b"hello", b"Hello"), (b"a", b"aa")];
    for (a, b) in pairs {
        assert_ne!(
            Digest::from_bytes(a),
            Digest::from_bytes(b),
            "colliding pair: {:?} vs {:?}",
            std::str::from_utf8(a),
            std::str::from_utf8(b)
        );
    }
}

// ====================================================================
// RuleId
// ====================================================================

#[test]
fn rule_id_accepts_well_formed_ids() {
    let ids = [
        "FUNC_LOOPS_FOR",
        "CLIPPY_UNWRAP_USED",
        "ARCHITECTURE_IMPORT_CORE_FS",
        "A_B",
        "RULE_1",
        "X1_Y2_Z3",
    ];
    for id in ids {
        assert_eq!(RuleId::new(id).unwrap().as_str(), id, "should accept {id}");
    }
}

#[test]
fn rule_id_rejects_empty() {
    assert_eq!(RuleId::new(""), Err(RuleIdError::Empty));
}

#[test]
fn rule_id_rejects_no_underscore() {
    assert_eq!(RuleId::new("FUNCLOOPS"), Err(RuleIdError::NoUnderscore));
    assert_eq!(RuleId::new("A"), Err(RuleIdError::NoUnderscore));
}

#[test]
#[allow(clippy::as_conversions)]
fn rule_id_rejects_lowercase_letter_at_each_position() {
    let bases = ["FUNC_LOOPS_FOR", "CLIPPY_UNWRAP_USED", "RULE_X"];
    for base in bases {
        for (pos, ch) in base.char_indices() {
            if ch.is_ascii_uppercase() {
                let lower = ch.to_ascii_lowercase();
                let mut s: Vec<u8> = base.as_bytes().to_vec();
                s[pos..pos + 1].copy_from_slice(&[lower as u8]);
                let result = RuleId::new(std::str::from_utf8(&s).unwrap());
                assert!(
                    matches!(result, Err(RuleIdError::NotUppercase(..))),
                    "{base} lowercased at pos {pos}, got {result:?}"
                );
            }
        }
    }
}

#[test]
fn rule_id_rejects_special_characters() {
    assert_eq!(RuleId::new("FUNC-LOOPS_FOR"), Err(RuleIdError::NotUppercase('-', 4)));
    assert_eq!(RuleId::new("FUNC_LOOPS.FOR"), Err(RuleIdError::NotUppercase('.', 10)));
    assert_eq!(RuleId::new("FUNC_ LOOPS_FOR"), Err(RuleIdError::NotUppercase(' ', 5)));
}

#[test]
fn rule_id_accepts_underscore_only_input() {
    assert!(RuleId::new("____").is_ok());
}

#[test]
fn rule_id_prefix_extracts_before_first_underscore() {
    let id = RuleId::new("ARCHITECTURE_IMPORT_CORE_FS").unwrap();
    assert_eq!(id.prefix(), "ARCHITECTURE");
    assert!(id.has_prefix("ARCHITECTURE"));
    assert!(!id.has_prefix("arch"));
}

#[test]
fn rule_id_normalize_uppercases_lowercase_input() {
    let id = RuleId::normalize("func_loops_for").unwrap();
    assert_eq!(id.as_str(), "FUNC_LOOPS_FOR");
}

#[test]
fn rule_id_normalize_drops_illegal_chars_preserving_underscore() {
    let id = RuleId::normalize("FUNC-LOOPS_FOR").unwrap();
    assert_eq!(id.as_str(), "FUNCLOOPS_FOR");
    let id2 = RuleId::normalize("func.loops_for").unwrap();
    assert_eq!(id2.as_str(), "FUNCLOOPS_FOR");
}

#[test]
fn rule_id_normalize_drops_all_underscores_to_no_underscore_error() {
    assert_eq!(RuleId::normalize("FUNC.LOOPS.FOR"), Err(RuleIdError::NoUnderscore));
}

#[test]
fn rule_id_normalize_propagates_validation_errors() {
    assert_eq!(RuleId::normalize("nounderscores"), Err(RuleIdError::NoUnderscore));
    assert_eq!(RuleId::normalize(""), Err(RuleIdError::Empty));
}

#[test]
fn rule_id_display_and_debug_have_stable_shape() {
    let id = RuleId::new("RULE_X").unwrap();
    assert_eq!(format!("{id}"), "RULE_X");
    assert!(format!("{id:?}").starts_with("RuleId("));
}

#[test]
fn rule_id_serde_round_trips_via_string() {
    let id = RuleId::new("CLIPPY_UNWRAP_USED").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"CLIPPY_UNWRAP_USED\"");
    let back: RuleId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, back);
}

#[test]
fn rule_id_serde_rejects_invalid_input() {
    let bad_inputs = ["\"lowercase_at_end\"", "\"X\"", "\"\u{1F600}_EMOJI\"", "\"RULE_HAS-DASH\""];
    for bad in bad_inputs {
        let result: Result<RuleId, _> = serde_json::from_str(bad);
        assert!(result.is_err(), "expected deserialization of {bad} to fail, got {result:?}");
    }
}

// ====================================================================
// WorkspacePath
// ====================================================================

#[test]
fn workspace_path_accepts_simple_paths() {
    let paths = [
        "src/lib.rs",
        "Cargo.toml",
        "crates/titania-core/src/lib.rs",
        "a/b/c/d/e.rs",
        "name_with_unicode_áéíó",
    ];
    for p in paths {
        assert_eq!(WorkspacePath::new(p).unwrap().as_str(), p, "should accept {p}");
    }
}

#[test]
fn workspace_path_rejects_empty() {
    assert_eq!(WorkspacePath::new(""), Err(WorkspacePathError::Empty));
}

#[test]
fn workspace_path_rejects_leading_slash() {
    assert_eq!(WorkspacePath::new("/etc/passwd"), Err(WorkspacePathError::LeadingSlash));
    assert_eq!(WorkspacePath::new("/src/lib.rs"), Err(WorkspacePathError::LeadingSlash));
}

#[test]
fn workspace_path_rejects_dotdot_everywhere() {
    assert_eq!(WorkspacePath::new("../etc/passwd"), Err(WorkspacePathError::ContainsDotDot));
    assert_eq!(WorkspacePath::new("src/../../etc/passwd"), Err(WorkspacePathError::ContainsDotDot));
    assert_eq!(WorkspacePath::new("src/.."), Err(WorkspacePathError::ContainsDotDot));
}

#[test]
fn workspace_path_does_not_reject_partial_dotdot() {
    assert!(WorkspacePath::new("..hidden").is_ok());
    assert!(WorkspacePath::new("foo..bar").is_ok());
}

#[test]
fn workspace_path_rejects_backslash_at_each_position() {
    let base = "src/lib.rs";
    for pos in 0..base.len() {
        let mut s: Vec<u8> = base.as_bytes().to_vec();
        if s[pos] == b'/' {
            s[pos] = b'a';
        }
        s[pos] = b'\\';
        let result = WorkspacePath::new(std::str::from_utf8(&s).unwrap());
        assert_eq!(result, Err(WorkspacePathError::ContainsBackslash), "pos={pos}");
    }
}

#[test]
fn workspace_path_rejects_null_byte() {
    assert_eq!(WorkspacePath::new("src\0lib.rs"), Err(WorkspacePathError::ContainsNull));
}

#[test]
fn workspace_path_rejects_each_control_byte_excluding_null() {
    for b in 1u8..0x20 {
        let mut s = b"a".to_vec();
        s.push(b);
        s.extend_from_slice(b"b");
        let result = WorkspacePath::new(std::str::from_utf8(&s).unwrap());
        assert_eq!(result, Err(WorkspacePathError::ControlByte(b)), "byte={b}");
    }
}

#[test]
fn workspace_path_segment_count_correct() {
    assert_eq!(WorkspacePath::new("a.rs").unwrap().segment_count(), 1);
    assert_eq!(WorkspacePath::new("src/lib.rs").unwrap().segment_count(), 2);
    assert_eq!(WorkspacePath::new("a/b/c/d/e.rs").unwrap().segment_count(), 5);
}

#[test]
fn workspace_path_starts_with_segment_matches_first() {
    let p = WorkspacePath::new("src/lib.rs").unwrap();
    assert!(p.starts_with_segment("src"));
    assert!(!p.starts_with_segment("lib"));
    assert!(!p.starts_with_segment("crates"));
}

#[test]
fn workspace_path_nfc_normalizes_combining_chars() {
    let decomposed = "a\u{0301}.rs";
    let composed = "á.rs";
    let d = WorkspacePath::new(decomposed).unwrap();
    let c = WorkspacePath::new(composed).unwrap();
    assert_eq!(d.as_str(), c.as_str(), "expected NFC canonicalization");
}

#[test]
fn workspace_path_serde_round_trip() {
    let p = WorkspacePath::new("crates/titania-core/src/lib.rs").unwrap();
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(json, "\"crates/titania-core/src/lib.rs\"");
    let back: WorkspacePath = serde_json::from_str(&json).unwrap();
    assert_eq!(p, back);
}

#[test]
fn workspace_path_serde_rejects_invalid_input() {
    let bad: Result<WorkspacePath, _> = serde_json::from_str("\"/abs/path\"");
    assert!(bad.is_err());
    let bad2: Result<WorkspacePath, _> = serde_json::from_str("\"../etc/passwd\"");
    assert!(bad2.is_err());
}

#[test]
fn workspace_path_try_from_str_matches_new() {
    let s = "src/lib.rs";
    assert_eq!(WorkspacePath::try_from(s).unwrap(), WorkspacePath::new(s).unwrap());
}

#[test]
fn workspace_path_display_and_debug_have_stable_shape() {
    let p = WorkspacePath::new("src/lib.rs").unwrap();
    assert_eq!(format!("{p}"), "src/lib.rs");
    assert!(format!("{p:?}").starts_with("WorkspacePath("));
}

// ====================================================================
// TextRange
// ====================================================================

#[test]
fn text_range_accepts_zero_length_range() {
    let r = TextRange::new(5, 5).unwrap();
    assert_eq!(r.start(), 5);
    assert_eq!(r.end(), 5);
    assert_eq!(r.width(), 0);
    assert!(r.is_empty());
}

#[test]
fn text_range_accepts_positive_length_range() {
    let r = TextRange::new(10, 20).unwrap();
    assert_eq!(r.start(), 10);
    assert_eq!(r.end(), 20);
    assert_eq!(r.width(), 10);
    assert!(!r.is_empty());
}

#[test]
fn text_range_rejects_end_before_start() {
    assert_eq!(TextRange::new(10, 5), Err(TextRangeError::EndBeforeStart { start: 10, end: 5 }));
    assert_eq!(TextRange::new(0, 0), Ok(TextRange::new(0, 0).unwrap()));
    assert_eq!(
        TextRange::new(u32::MAX, 0),
        Err(TextRangeError::EndBeforeStart { start: u32::MAX, end: 0 })
    );
}

#[test]
fn text_range_accepts_full_u32_range() {
    let r = TextRange::new(0, u32::MAX).unwrap();
    assert_eq!(r.start(), 0);
    assert_eq!(r.end(), u32::MAX);
    assert_eq!(r.width(), u32::MAX);
}

#[test]
fn text_range_width_is_non_negative_for_all_valid_inputs() {
    for start in [0u32, 1, 100, u32::MAX / 2, u32::MAX] {
        let candidates = [start, start.saturating_add(1), start.saturating_add(100), u32::MAX];
        for end in candidates {
            if end < start {
                continue;
            }
            let r = TextRange::new(start, end).unwrap();
            assert_eq!(r.width(), end - start, "start={start} end={end}");
            assert_eq!(r.end() - r.start(), r.width());
        }
    }
}

#[test]
fn text_range_contains_byte_is_inclusive_start_exclusive_end() {
    let r = TextRange::new(10, 20).unwrap();
    assert!(!r.contains_byte(9));
    assert!(r.contains_byte(10));
    assert!(r.contains_byte(15));
    assert!(r.contains_byte(19));
    assert!(!r.contains_byte(20));
}

#[test]
fn text_range_overlaps_distinct_cases() {
    let left = TextRange::new(10, 20).unwrap();
    let right_touch = TextRange::new(20, 30).unwrap();
    let inside = TextRange::new(15, 18).unwrap();
    let disjoint = TextRange::new(0, 5).unwrap();
    let right_dup = TextRange::new(20, 30).unwrap();
    assert!(!left.overlaps(&right_touch), "touching at endpoint shouldn't overlap");
    assert!(left.overlaps(&inside));
    assert!(!left.overlaps(&disjoint));
    assert!(right_touch.overlaps(&right_dup));
}

#[test]
fn text_range_display_is_stable() {
    let r = TextRange::new(3, 7).unwrap();
    assert_eq!(format!("{r}"), "[3..7)");
}

#[test]
fn text_range_serde_round_trip_object_form() {
    let r = TextRange::new(100, 250).unwrap();
    let json = serde_json::to_string(&r).unwrap();
    assert_eq!(json, r#"{"start_byte":100,"end_byte":250}"#);
    let back: TextRange = serde_json::from_str(&json).unwrap();
    assert_eq!(r, back);
}

#[test]
fn text_range_serde_rejects_inverted_input() {
    let result: Result<TextRange, _> = serde_json::from_str(r#"{"start_byte":20,"end_byte":10}"#);
    assert!(result.is_err());
}

#[test]
fn text_range_ordering_matches_byte_position() {
    let a = TextRange::new(0, 5).unwrap();
    let b = TextRange::new(5, 10).unwrap();
    let c = TextRange::new(0, 10).unwrap();
    assert!(a < b);
    assert!(a < c);
    assert_eq!(a.cmp(&c), std::cmp::Ordering::Less);
}
