# PR #1 — fix(lanes): repair four scanner-laneparser defects (D1-D5/D6)

> Permanent in-repo artifact of GitHub PR #1. Captured before the PR
> ref `pulls/1/head` is auto-pruned (90 days after close) so the defect
> table, test coverage breakdown, and gate verification transcript
> survive. The actual code changes are preserved in `main`'s git history
> at merge commit `383c93aacb042092dfacb1be59a4831ef8d86cca`.

## Summary

Four functional defects in titania-lanes scanner binaries, each
allowing production panics/forbidden tokens/disallowed features to
slip through silently. The build, lint, test, and moon ci gates
are all green; the defects were latent in the parsing logic and not
exercised by existing unit tests.

## Defects fixed

| # | Lane | Symptom | Root cause | Fix |
|---|-----|---------|------------|-----|
| **D1** | `check-panic-surface` | `assert!(true);` after `#[cfg(test)]` inside a function body was silently passed | The `let _ =` lines on cfg/kani open never set a baseline depth, so the outer fn's `{` was counted toward the cfg scope and the scope never closed | Rewrote the tracker to track a global brace depth and pop cfg/kani scopes whose snapshotted depth we have strictly returned to |
| **D2** | `check-nightly-features` | Single-line `#![feature(try_blocks)]` was reported as the literal feature name `try_blocks)` (with a stray `)`), so every `try_blocks` match failed | `push_closed_feature` sliced up to the `)` of `)]` but the `]` was outside the slice; the downstream `trim_end_matches(")]")` then failed | Include the `]` in the slice (`close_idx+1`) and guard with an `ends_with(")]")` check |
| **D3** | `forbidden-scan` | `.unwrap()` and `.expect("msg")` slipped through; only `unwrap()` (with both parens) was caught | Default token set was `unwrap()` / `expect()` as plain substrings; Rust method-call syntax never produces the literal `unwrap()` substring | Replace `unwrap` with a kind-aware type: `Macro` tokens (`panic!`, `todo!`, `unimplemented!`, `dbg!`) match as raw substrings with a word-boundary prefix; `Method` tokens (`unwrap`, `expect`) match only when preceded by `.` or `::` and followed by `(` |
| **D5+D6** | `check-panic-surface` | `/* assert!(true); */` was flagged as a violation; the `forbidden-scan` lane already had a proper SourceLineParser but the panic lane did not | `is_comment` only checked `//`; no shared lexer | Move `SourceLineParser` from `forbidden_scan/source_line.rs` to a new shared module `titania_lanes::source_line`; consume it from both `forbidden_scan` (now also strips string contents) and `check_panic_surface` |

## Files changed (8 files, +703/-172)

| file | change |
|---|---|
| `crates/titania-lanes/src/bin/check_nightly_features.rs` | +80/-? (D2 fix) |
| `crates/titania-lanes/src/bin/check_panic_surface.rs` | +94/? (D1 fix, D5/D6) |
| `crates/titania-lanes/src/bin/forbidden_scan.rs` | -3 (D3 fix) |
| `crates/titania-lanes/src/bin/forbidden_scan/lane.rs` | +161/-? (D3 fix) |
| `crates/titania-lanes/src/bin/forbidden_scan/source_line.rs` | -124 (deleted; D5/D6) |
| `crates/titania-lanes/src/lib.rs` | +2 (D5/D6 re-export) |
| `crates/titania-lanes/src/source_line.rs` | +226 (new shared lexer; D5/D6) |
| `crates/titania-lanes/tests/scanner_target_project.rs` | +185 (new tests) |

## Tests added (26 new tests)

- **7 unit tests** in `titania_lanes::source_line`: line comments, block
  comments, multi-line block comments, string contents blanked,
  escaped quotes, etc.
- **9 unit tests** in `forbidden_scan::lane` (`ForbiddenToken`):
  macro and method token kinds, identifier-prefix rejection, `::`
  receiver, `(` requirement, etc.
- **7 unit tests** in `check_nightly_features`: `push_closed_feature`
  (single line, multi-feature, no close), `collect_features` (single
  line, multi line, non-attribute text), `is_perf_scoped_path`.
- **12 integration tests** in `tests/scanner_target_project.rs` that
  run each bin against a hand-crafted fixture (cfg mod inside fn,
  top-level cfg mod, single-line try_blocks, multi-line perf
  features, .expect("msg"), .unwrap(), Result::unwrap, identifier
  FP rejection, block comments, string literals).

## Gate verification transcript (at merge)

```
cargo fmt --check               # clean
cargo clippy --workspace --all-targets --all-features \
  -- -D warnings -D unsafe_code \
  -D clippy::unwrap_used -D clippy::expect_used \
  -D clippy::panic -D clippy::todo -D clippy::unimplemented \
  -D clippy::dbg_macro -D clippy::indexing_slicing \
  -D clippy::string_slice -D clippy::get_unwrap \
  -D clippy::arithmetic_side_effects -D clippy::as_conversions \
  -D clippy::let_underscore_must_use -D clippy::await_holding_lock
                                  # clean
cargo test --workspace --all-features
                                  # 199 passed
moon run titania:ci               # 9/9 tasks (audit, deny, check,
                                  #        clippy-all, fmt, test,
                                  #        lint-src, geiger, ci)
```

## Beads closed

- `tn-dhr` (D1: check-panic-surface cfg/test scope never closes)
- `tn-mqu` (D2: check-nightly-features parse drops closing bracket)
- `tn-6g5` (D3: forbidden-scan .expect/.unwrap slips through)
- `tn-pwk` (D5+D6: check-panic-surface misses block comments; share
  SourceLineParser with forbidden-scan)
- `tn-l1s` (D10: regression tests for D1/D2/D3/D5/D6 parser fixes)

## Provenance

- **Merge commit SHA**: `383c93aacb042092dfacb1be59a4831ef8d86cca`
- **Merged**: 2026-07-01T03:03:07Z (14 seconds after creation — self-merge)
- **PR number**: #1
- **Branch**: `fix/lane-parser-defects` (deleted post-merge)
- **Author**: `lprior-repo`
- **Reviews**: 0
- **Comments**: 0
- **State**: MERGED → closed
