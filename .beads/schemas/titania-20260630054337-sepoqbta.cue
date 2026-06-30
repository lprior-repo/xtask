
package validation

import "list"

// Validation schema for bead: titania-20260630054337-sepoqbta
// Title: lanes: run-cargo bin dispatching fmt/clippy/test/build in target
//
// This schema validates that implementation is complete.
// Use: cue vet titania-20260630054337-sepoqbta.cue implementation.cue

#BeadImplementation: {
  bead_id: "titania-20260630054337-sepoqbta"
  title: "lanes: run-cargo bin dispatching fmt/clippy/test/build in target"

  // Contract verification
  contracts_verified: {
    preconditions_checked: bool & true
    postconditions_verified: bool & true
    invariants_maintained: bool & true

    // Specific preconditions that must be verified
    precondition_checks: [
      "argv[1] is one of {fmt, compile, clippy, test, build}",
      "CWD resolves to a valid target via discover_target",
    ]

    // Specific postconditions that must be verified
    postcondition_checks: [
      "a typed LaneReport is produced and written to stdout in the lane's standard render format",
      "process exit code is LaneExit::Clean (0), Violations (1), Usage (2), or Failure (3) per the convention",
    ]

    // Specific invariants that must be maintained
    invariant_checks: [
      "subcommand is matched exhaustively (no _ => ...)",
      "no .unwrap()/.expect() in main or dispatch",
      "every subprocess invocation goes through CommandIn",
    ]
  }

  // Test verification
  tests_passing: {
    all_tests_pass: bool & true

    happy_path_tests: [...string] & list.MinItems(4)
    error_path_tests: [...string] & list.MinItems(4)

    // Note: Actual test names provided by implementer, must include all required tests

    // Required happy path tests
    required_happy_tests: [
      "run-cargo fmt on a clean workspace returns LaneExit::Clean with zero findings",
      "run-cargo clippy on titania workspace (which is clean) returns LaneExit::Clean",
      "run-cargo test on titania workspace returns LaneExit::Clean",
      "run-cargo build on titania workspace returns LaneExit::Clean",
    ]

    // Required error path tests
    required_error_tests: [
      "run-cargo with no subcommand returns LaneExit::Usage",
      "run-cargo with unknown subcommand `frobnicate` returns LaneExit::Usage with message listing valid subcommands",
      "run-cargo fmt on a workspace with bad formatting returns LaneExit::Violations with at least one CARGO-FMT-001 finding",
      "run-cargo clippy on a workspace with a clippy violation (e.g. an .unwrap() in a fixture) returns LaneExit::Violations with a CARGO-CLIPPY-001 finding",
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