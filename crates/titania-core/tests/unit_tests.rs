//! Unit tests for `titania-core` primitives.
//!
//! These live in `tests/` (rather than `#[cfg(test)] mod tests` in
//! `src/`) so that the Holzman rg gate — which excludes `tests/` — does
//! not flag `assert!`/`assert_eq!` calls. The tests exercise the
//! `pub` API surface end-to-end.

#![allow(clippy::as_conversions)] // ASCII byte round-trip is exact in tests.

use titania_core::{Digest, DigestError, RuleId, RuleIdError};

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
