//! Behavior tests (Given/When/Then, BDD-style) for the public API.
//!
//! These tests exercise the API as a real consumer would: construct a
//! value, serialize it, deserialize it, and observe a stable outcome.

use titania_core::{Digest, RuleId, TextRange, WorkspacePath};

// ----------------------------------------------------------------------
// Scenario 1: A receiving pipeline computes a digest and stores it.
// ----------------------------------------------------------------------
#[test]
fn scenario_receiver_pipeline_digest() {
    // Given: a payload and a recipient.
    let payload = b"src/lib.rs:42: this is the source line we care about";
    // When: the recipient hashes the payload.
    let digest = Digest::from_bytes(payload);
    // Then: the digest is a stable 64-character lowercase-hex string.
    assert_eq!(digest.as_hex().len(), 64);
    assert!(digest.as_hex().bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
}

// ----------------------------------------------------------------------
// Scenario 2: A rule catalog names a finding in stable form.
// ----------------------------------------------------------------------
#[test]
fn scenario_rule_catalog_names_finding() {
    // Given: a rule id string with implicit lowercase.
    let raw = "func_loops_for";
    // When: we normalize and validate.
    let id = RuleId::normalize(raw).unwrap();
    // Then: it is uppercase, contains an underscore, and round-trips.
    assert_eq!(id.as_str(), "FUNC_LOOPS_FOR");
    assert!(id.has_prefix("FUNC"));
    assert_eq!(id.to_string(), "FUNC_LOOPS_FOR");
}

// ----------------------------------------------------------------------
// Scenario 3: A workspace path round-trips and rejects traversal.
// ----------------------------------------------------------------------
#[test]
fn scenario_path_round_trip_and_reject_traversal() {
    // Given: a benign path and a malicious path.
    let benign = WorkspacePath::new("src/parser.rs").unwrap();
    let malicious_attempts =
        ["../etc/passwd", "src/../../etc/shadow", "/etc/passwd", "ok\\..\\bad"];

    // When: we serialize the benign and try the malicious ones.
    let json = serde_json::to_string(&benign).unwrap();
    let recovered: WorkspacePath = serde_json::from_str(&json).unwrap();

    // Then: the benign round-trips; every malicious input is rejected.
    assert_eq!(benign, recovered);
    assert_eq!(benign.as_str(), "src/parser.rs");
    for bad in malicious_attempts {
        assert!(WorkspacePath::new(bad).is_err(), "expected {bad} to be rejected");
    }
}

// ----------------------------------------------------------------------
// Scenario 4: A patch describes a byte range that contains a target byte.
// ----------------------------------------------------------------------
#[test]
fn scenario_text_range_contains_target_byte() {
    // Given: a 12-byte source line "hello world\n"; we want to patch
    // bytes 6..11 (inclusive start, exclusive end — half-open).
    let r = TextRange::new(6, 11).unwrap();
    // When: we check containment.
    let inside = r.contains_byte(8);
    let before = r.contains_byte(3);
    let after = r.contains_byte(11); // exclusive end
    let empty_at_start = TextRange::new(6, 6).unwrap().contains_byte(6);
    // Then:
    assert!(inside);
    assert!(!before);
    assert!(!after);
    assert!(!empty_at_start); // empty range does not contain its own start.
    assert_eq!(r.width(), 5);
}
