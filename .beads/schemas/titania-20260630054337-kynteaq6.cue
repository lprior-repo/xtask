
package validation

import "list"

// Validation schema for bead: titania-20260630054337-kynteaq6
// Title: core: QualityReceipt records target_root
//
// This schema validates that implementation is complete.
// Use: cue vet titania-20260630054337-kynteaq6.cue implementation.cue

#BeadImplementation: {
  bead_id: "titania-20260630054337-kynteaq6"
  title: "core: QualityReceipt records target_root"

  // Contract verification
  contracts_verified: {
    preconditions_checked: bool & true
    postconditions_verified: bool & true
    invariants_maintained: bool & true

    // Specific preconditions that must be verified
    precondition_checks: [
      "target_root is a valid TargetProject",
      "all four digests are 64-char lowercase hex (validated by Digest::new)",
    ]

    // Specific postconditions that must be verified
    postcondition_checks: [
      "round-trip serde preserves all fields",
      "schema_version 2 receipts are forward-compatible with adding new optional fields later",
    ]

    // Specific invariants that must be maintained
    invariant_checks: [
      "schema_version is always present in the serialized form",
      "target_root serializes to a stable string (camino Utf8PathBuf serializes to its string form)",
    ]
  }

  // Test verification
  tests_passing: {
    all_tests_pass: bool & true

    happy_path_tests: [...string] & list.MinItems(2)
    error_path_tests: [...string] & list.MinItems(2)

    // Note: Actual test names provided by implementer, must include all required tests

    // Required happy path tests
    required_happy_tests: [
      "build a Receipt with synthetic digests, serialize, deserialize, assert all fields equal",
      "schema_version 2 deserializes a v2 fixture successfully",
    ]

    // Required error path tests
    required_error_tests: [
      "deserializing a v1 schema receipt returns UnsupportedSchemaVersion",
      "serializing a Receipt with a malformed digest (not 64 hex chars) fails at Digest::new, not at serde",
    ]
  }

  // Code completion
  code_complete: {
    implementation_exists: string  // Path to implementation file
    tests_exist: string  // Path to test file
    ci_passing: bool & true
    no_unwrap_calls: bool & true  // Rust/functional constraint
    no_panics: bool & true  // Rust constraint
  }

  // Completion criteria
  completion: {
    all_sections_complete: bool & true
    documentation_updated: bool
    beads_closed: bool
    timestamp: string  // ISO8601 completion timestamp
  }
}

// Example implementation proof - create this file to validate completion:
//
// implementation.cue:
// package validation
//
// implementation: #BeadImplementation & {
//   contracts_verified: {
//     preconditions_checked: true
//     postconditions_verified: true
//     invariants_maintained: true
//     precondition_checks: [/* documented checks */]
//     postcondition_checks: [/* documented verifications */]
//     invariant_checks: [/* documented invariants */]
//   }
//   tests_passing: {
//     all_tests_pass: true
//     happy_path_tests: ["test_version_flag_works", "test_version_format", "test_exit_code_zero"]
//     error_path_tests: ["test_invalid_flag_errors", "test_no_flags_normal_behavior"]
//   }
//   code_complete: {
//     implementation_exists: "src/main.rs"
//     tests_exist: "tests/cli_test.rs"
//     ci_passing: true
//     no_unwrap_calls: true
//     no_panics: true
//   }
//   completion: {
//     all_sections_complete: true
//     documentation_updated: true
//     beads_closed: false
//     timestamp: "2026-06-30T05:43:37Z"
//   }
// }