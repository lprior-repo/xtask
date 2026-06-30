
package validation

import "list"

// Validation schema for bead: titania-20260630054337-77bp1cb3
// Title: lanes: migrate existing lane bins to TargetProject + CommandIn
//
// This schema validates that implementation is complete.
// Use: cue vet titania-20260630054337-77bp1cb3.cue implementation.cue

#BeadImplementation: {
  bead_id: "titania-20260630054337-77bp1cb3"
  title: "lanes: migrate existing lane bins to TargetProject + CommandIn"

  // Contract verification
  contracts_verified: {
    preconditions_checked: bool & true
    postconditions_verified: bool & true
    invariants_maintained: bool & true

    // Specific preconditions that must be verified
    precondition_checks: [
      "task-001 (TargetProject) and task-002 (discover_target) and task-003 (CommandIn) are closed",
    ]

    // Specific postconditions that must be verified
    postcondition_checks: [
      "every migrated bin accepts a target and shells out via CommandIn",
      "cargo test --workspace --all-features still passes",
      "dogfood test produces same exit codes as before migration",
    ]

    // Specific invariants that must be maintained
    invariant_checks: [
      "zero .unwrap()/.expect()/.panic!() in any migrated bin",
      "every Command::new call goes through CommandIn",
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
      "each migrated bin runs successfully on the titania workspace (dogfood) and on a fixture workspace under tempfile",
      "findings.path values are relative to the target root, not the CWD",
    ]

    // Required error path tests
    required_error_tests: [
      "running a migrated bin on a directory with no Cargo.toml returns LaneExit::Usage with TargetProjectError::NoCargoToml",
      "running a migrated bin on a directory with a [package] but no [workspace] still works (single-crate fallback)",
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