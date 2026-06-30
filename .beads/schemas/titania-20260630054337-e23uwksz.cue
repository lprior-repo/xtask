
package validation

import "list"

// Validation schema for bead: titania-20260630054337-e23uwksz
// Title: lanes: CommandIn helper for shelling out in a target
//
// This schema validates that implementation is complete.
// Use: cue vet titania-20260630054337-e23uwksz.cue implementation.cue

#BeadImplementation: {
  bead_id: "titania-20260630054337-e23uwksz"
  title: "lanes: CommandIn helper for shelling out in a target"

  // Contract verification
  contracts_verified: {
    preconditions_checked: bool & true
    postconditions_verified: bool & true
    invariants_maintained: bool & true

    // Specific preconditions that must be verified
    precondition_checks: [
      "cwd is a valid TargetProject",
      "program is a non-empty string resolvable on PATH or as a literal path",
    ]

    // Specific postconditions that must be verified
    postcondition_checks: [
      "CommandIn::run returns Ok(CommandOutput) on exit code 0",
      "CommandIn::run returns Err(LaneError::NonZeroExit) on non-zero exit",
    ]

    // Specific invariants that must be maintained
    invariant_checks: [
      "current_dir is set on every CommandIn execution",
      "no .unwrap()/.expect() in the helper module",
      "no allocation in the builder methods after the first call (small Vec reuses)",
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
      "CommandIn::new(&target, \"/bin/echo\").arg(\"hi\").run() returns Ok with stdout 'hi\\n'",
      "CommandIn with env() passes env to the subprocess (assert via /bin/sh -c 'echo $FOO')",
    ]

    // Required error path tests
    required_error_tests: [
      "CommandIn::new(&target, \"/bin/false\").run() returns Err(NonZeroExit { code: 1, .. })",
      "CommandIn with a non-existent program returns Err(Io(NotFound))",
      "stdout_str on a binary output returns Err(NonUtf8Output)",
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