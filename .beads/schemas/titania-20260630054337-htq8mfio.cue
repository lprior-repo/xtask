
package validation

import "list"

// Validation schema for bead: titania-20260630054337-htq8mfio
// Title: core: TargetProject value object with smart constructor
//
// This schema validates that implementation is complete.
// Use: cue vet titania-20260630054337-htq8mfio.cue implementation.cue

#BeadImplementation: {
  bead_id: "titania-20260630054337-htq8mfio"
  title: "core: TargetProject value object with smart constructor"

  // Contract verification
  contracts_verified: {
    preconditions_checked: bool & true
    postconditions_verified: bool & true
    invariants_maintained: bool & true

    // Specific preconditions that must be verified
    precondition_checks: [
      "input path is &Path",
      "filesystem is readable",
    ]

    // Specific postconditions that must be verified
    postcondition_checks: [
      "constructed TargetProject.as_path() returns absolute UTF-8 path",
      "constructed TargetProject.manifest_path() returns {root}/Cargo.toml",
    ]

    // Specific invariants that must be maintained
    invariant_checks: [
      "TargetProject inner is always absolute",
      "TargetProject inner is always valid UTF-8",
      "TargetProject always has a Cargo.toml at manifest_path",
    ]
  }

  // Test verification
  tests_passing: {
    all_tests_pass: bool & true

    happy_path_tests: [...string] & list.MinItems(2)
    error_path_tests: [...string] & list.MinItems(3)

    // Note: Actual test names provided by implementer, must include all required tests

    // Required happy path tests
    required_happy_tests: [
      "try_from_path on a tempdir containing Cargo.toml returns Ok with manifest_path == root/Cargo.toml",
      "round-trip: construct the value, serialize to JSON, deserialize, assert equality of all fields",
    ]

    // Required error path tests
    required_error_tests: [
      "try_from_path on /nonexistent returns NotFound",
      "try_from_path on a regular file returns NotADirectory",
      "try_from_path on an empty tempdir returns NoCargoToml",
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