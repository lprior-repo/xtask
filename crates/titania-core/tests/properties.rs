//! Property tests using `proptest`. Each property is a hypothesis about
//! the invariants of the primitive types — exercised at scale.
//!
//! Test files are exempt from the strict production code lint policy;
//! they may use `as`, assertions, and complex types. The Holzman gate
//! ensures these never leak into crate source.

#![allow(clippy::as_conversions)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::type_complexity)]
#![allow(clippy::map_identity)]

use proptest::prelude::*;

use titania_core::{Digest, RuleId, TextRange, WorkspacePath};

proptest! {
    #[test]
    fn digest_from_bytes_is_deterministic(seed in any::<Vec<u8>>()) {
        let a = Digest::from_bytes(&seed);
        let b = Digest::from_bytes(&seed);
        prop_assert_eq!(a, b);
    }

    #[test]
    fn digest_is_always_64_lowercase_hex(_seed in any::<u32>()) {
        // Any random input should yield a 64-char lowercase-hex string.
        let d = Digest::from_bytes(&_seed.to_le_bytes());
        let s = d.as_hex();
        prop_assert_eq!(s.len(), 64);
        prop_assert!(
            s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "non-lowercase-hex byte in {s}"
        );
    }

    #[test]
    fn digest_sampled_injective(a in any::<[u8; 32]>(), b in any::<[u8; 32]>()) {
        // Two random 32-byte inputs collide with negligible probability.
        // If `a == b` skip; otherwise the digests must differ.
        if a != b {
            prop_assert_ne!(Digest::from_bytes(&a), Digest::from_bytes(&b));
        }
    }

    #[test]
    fn digest_serde_round_trip(seed in any::<Vec<u8>>()) {
        let d = Digest::from_bytes(&seed);
        let json = serde_json::to_string(&d).unwrap();
        let back: Digest = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(d, back);
    }

    // ---- RuleId ----

    #[test]
    fn rule_id_display_round_trip(s in "[A-Z][A-Z0-9_]*_[A-Z0-9_]+") {
        // Strategy: an uppercase-only string with at least one underscore.
        let id = RuleId::new(&s).unwrap();
        prop_assert_eq!(id.as_str(), s.as_str());
        let displayed = id.to_string();
        prop_assert_eq!(displayed, s);
    }

    #[test]
    fn rule_id_normalize_uppercases_input(s in "[a-zA-Z0-9_-]{1,64}") {
        let result = RuleId::normalize(&s);
        match result {
            Ok(id) => {
                // All chars in the produced id must be uppercase ASCII or '_'.
                for c in id.as_str().chars() {
                    prop_assert!(
                        matches!(c, 'A'..='Z' | '0'..='9' | '_'),
                        "unexpected char {:?} in normalized {id}", c
                    );
                }
                prop_assert!(id.as_str().contains('_'), "id {id} has no underscore");
            }
            Err(_) => {
                // Acceptable: every non-uppercase char was a non-`_` and
                // was dropped, leaving no underscore. That's documented.
            }
        }
    }

    #[test]
    fn rule_id_rejects_lowercase(s in "[a-z][a-zA-Z0-9_]+") {
        // Strings that contain at least one lowercase letter must fail.
        let result = RuleId::new(&s);
        prop_assert!(
            matches!(result, Err(titania_core::RuleIdError::NotUppercase(..))),
            "expected NotUppercase, got {result:?}"
        );
    }

    #[test]
    fn rule_id_serde_round_trip(s in "[A-Z][A-Z0-9_]*_[A-Z0-9_]+") {
        let id = RuleId::new(&s).unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let back: RuleId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(id, back);
    }

    // ---- WorkspacePath ----

    #[test]
    fn workspace_path_accepts_no_backslash_no_dotdot_no_leading(
        a in "[a-z][a-z0-9_-]{0,15}",
        b in "[a-z][a-z0-9_-]{0,15}",
    ) {
        // Two segments, no '..', no leading slash, no backslash, no control.
        let candidate = format!("{a}/{b}");
        let result = WorkspacePath::new(&candidate);
        prop_assert!(result.is_ok(), "expected ok for {candidate}, got {result:?}");
        if let Ok(p) = result {
            prop_assert_eq!(p.as_str(), candidate);
            prop_assert_eq!(p.segment_count(), 2);
        }
    }

    #[test]
    fn workspace_path_rejects_leading_slash(s in "[a-z][a-z0-9_/]{0,15}") {
        let bad = format!("/{s}");
        let result = WorkspacePath::new(&bad);
        prop_assert_eq!(
            result,
            Err(titania_core::WorkspacePathError::LeadingSlash)
        );
    }

    #[test]
    fn workspace_path_rejects_backslash(s in "[a-z][a-z0-9_]{0,15}") {
        let bad = format!("{s}\\\\");
        let result = WorkspacePath::new(&bad);
        prop_assert_eq!(
            result,
            Err(titania_core::WorkspacePathError::ContainsBackslash)
        );
    }

    #[test]
    fn workspace_path_rejects_dotdot_segment(
        a in "[a-z]{1,8}",
        b in "[a-z]{1,8}",
    ) {
        let bad = format!("{a}/../{b}");
        let result = WorkspacePath::new(&bad);
        prop_assert_eq!(
            result,
            Err(titania_core::WorkspacePathError::ContainsDotDot)
        );
    }

    #[test]
    fn workspace_path_serde_round_trip(s in "[a-z][a-z0-9_/-]{0,15}") {
        // Some inputs may have leading slash, control, etc. Filter them.
        if WorkspacePath::new(&s).is_ok() {
            let p = WorkspacePath::new(&s).unwrap();
            let json = serde_json::to_string(&p).unwrap();
            let back: WorkspacePath = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(p, back);
        }
    }

    // ---- TextRange ----

    #[test]
    fn text_range_width_is_end_minus_start(start in any::<u32>(), end in any::<u32>()) {
        let result = TextRange::new(start, end);
        if end < start {
            prop_assert!(result.is_err());
        } else {
            let r = result.unwrap();
            prop_assert_eq!(r.start(), start);
            prop_assert_eq!(r.end(), end);
            prop_assert_eq!(r.width(), end - start);
            prop_assert_eq!(r.is_empty(), start == end);
        }
    }

    #[test]
    fn text_range_contains_byte_is_inclusive_exclusive(
        start in 0u32..1000,
        width in 0u32..1000,
        probe in any::<i32>(),
    ) {
        // Map probe into a wider range than the text range so we exercise both sides.
        let p = if probe < 0 { 0u32 } else { probe as u32 };
        let end = start.saturating_add(width);
        let r = TextRange::new(start, end).unwrap();
        let inside = r.contains_byte(p);
        let expected = p >= start && p < end;
        let msg = format!("start={start} end={end} probe={p}");
        prop_assert_eq!(inside, expected, "{}", msg);
    }

    #[test]
    fn text_range_overlaps_semantics(
        a_start in 0u32..500,
        a_end in 500u32..1000,
        b_start in 0u32..500,
        b_end in 500u32..1000,
    ) {
        let ra = TextRange::new(a_start, a_end).unwrap();
        let rb = TextRange::new(b_start, b_end).unwrap();
        let expected = a_start < b_end && b_start < a_end;
        prop_assert_eq!(ra.overlaps(&rb), expected);
    }

    #[test]
    fn text_range_serde_round_trip(start in any::<u32>(), end in any::<u32>()) {
        if end >= start {
            let r = TextRange::new(start, end).unwrap();
            let json = serde_json::to_string(&r).unwrap();
            let back: TextRange = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(r, back);
        }
    }
}
