
package validation

import "list"

// Validation schema for bead: titania-20260630054337-sfplbyzi
// Title: lanes: BDD end-to-end target-project scenarios
//
// This schema validates that implementation is complete.
// Use: cue vet titania-20260630054337-sfplbyzi.cue implementation.cue

#BeadImplementation: {
  bead_id: "titania-20260630054337-sfplbyzi"
  title: "lanes: BDD end-to-end target-project scenarios"

  // Contract verification
  contracts_verified: {
    preconditions_checked: bool & true
    postconditions_verified: bool & true
    invariants_maintained: bool & true

    // Specific preconditions that must be verified
    precondition_checks: [
      "tasks 001-006 are closed",
      "tempfile dev-dep is in titania-lanes/Cargo.toml",
    ]

    // Specific postconditions that must be verified
    postcondition_checks: [
      "all 5 BDD scenarios pass with exact assertions",
      "test runtime under 30s for the full suite",
    ]

    // Specific invariants that must be maintained
    invariant_checks: [
      "every assertion is exact (eq/contains/matches, not is_ok())",
      "no checked-in fixture files — everything built at test time via tempfile",
    ]
  }

  // Test verification
  tests_passing: {
    all_tests_pass: bool & true

    happy_path_tests: [...string] & list.MinItems(4)
    error_path_tests: [...string] & list.MinItems(2)

    // Note: Actual test names provided by implementer, must include all required tests

    // Required happy path tests
    required_happy_tests: [
      "scenario 1: workspace discovery from sub-crate — cwd = <tmp>/workspace/crates/foo, run-cargo fmt finds <tmp>/workspace/Cargo.toml and reports findings relative to <tmp>/workspace",
      "scenario 2: single-crate fallback — cwd = <tmp>/single, run-cargo fmt uses <tmp>/single as target",
      "scenario 4: receipt records target_root — running a lane on a fixture, parsing the receipt JSON, asserting receipt.target_root == resolved path",
      "scenario 5: dogfood — cwd = titania repo, run-cargo fmt produces the same result as the pre-feature state",
    ]

    // Required error path tests
    required_error_tests: [
      "scenario 3: no Cargo.toml — cwd = empty tempfile dir, run-cargo fmt returns LaneExit::Usage with TargetProjectError::NoCargoToml",
      "construction with a malformed/empty input returns a typed error, never panics",
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