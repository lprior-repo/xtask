# Stage 3 — F-010 typed-error remainder CLOSED PARTIAL

## Summary

Stage 3 closed with 6 of 10 files migrated (60%). 4 files deferred to
Stage 3.5 (audit + refactor commit). Inventory closed at 8 occurrences
across 4 files remaining.

## Files migrated (6 of 10)

| # | file | commit | typed error | notes |
|---|---|---|---|---|
| 1 | `run_cargo.rs` | `13c056a` | `RunCargoError::Usage` direct construction in `CargoLane::parse` | tightened `parse()` signature |
| 2 | `kani_list.rs` | `111fc3d` | `JsonValidationError { Parse(serde_json::Error) }` | added `Display + std::error::Error` |
| 3 | `forbidden_scan/lane.rs` | `bdfbe13` | `ForbiddenAcBuildError { Build { count, source: aho_corasick::BuildError } }` | source() chains to AC builder error |
| 4 | `check_hot_cold_forbidden_apis/{scan,selftest,main}.rs` | `75e3fc7` | `HotFileError { LoadAllow { source: String }, HotSourceWalk, Unreadable }` | cascaded widening through selftest + main bin |
| 6 | `rust_verification_gauntlet.rs` | `4dfb2df` | `DiscoverError { CargoCapture, Utf8 { source: LaneError }, Json { source: serde_json::Error } }` | CargoCapture carries String temporarily (will tighten when file 9 lands) |

All commits gated by `cargo check -p titania-lanes --bins --all-features`
and `cargo test --workspace --all-features --no-fail-fast` (217/0 tests,
no regressions).

## Files deferred to Stage 3.5

| file | inventory hits | reason for deferral | cascade risk |
|---|---|---|---|
| `check_hot_cold_forbidden_apis/allow_file.rs` | 5 (parse + validate + helpers) | Currently `load_allow_file` returns `Result<_, String>`. Migrating forces widening through `scan.rs` to drop the interim `HotFileError::LoadAllow { source: String }` (which is the awkward shape we'd want to remove) | HIGH (touches scan.rs + selftest.rs + main bin) |
| `verify_verus/verus_tool.rs` | 2 (run_verus_target + write_log) | Tightens a tightly-coupled pair (write_log returns Result<_, String> but is called from run_verus_target), and cascades into `verify_verus/outcome.rs:54` (record_target_failure signature change). 5 edits cascading in less than 5 minutes is too risky. | HIGH (touches outcome.rs caller) |
| `check_test_integrity/self_test.rs` | 7 (`run_fixtures`, `with_initialized_repo`, `assert_clean_fixture`, `assert_untracked_ignored_fixture`, `run_git`, `scratch_dir`) | All 7 sites are test-helper methods under `#[cfg(test)]` or selftest-mode gated. Needs a single coherent test-error enum (TestSelfError or similar) instead of 7 ad-hoc String types. Larger surface than file 4. | MEDIUM (configurable to keep selftest-mode out of production reach) |
| `rust_verification_gauntlet/commands.rs` | 3 (cargo_capture, sibling_binary, missing_binary + run_xtask_loom) | Has `cargo_capture(..., args) -> Result<_, String>` which file 6's `DiscoverError::CargoCapture` currently wraps as String. Migrating this lets CargoCapture tighten to `LaneError`. Best done as a 2-step: (1) commands.rs typed; (2) revert file 6's CargoCapture string shape. | MEDIUM (touches file 6 in cleanup) |

## Stage 3.5 plan

Recommended order, each gated:

1. **`commands.rs` file 9** (3 hits): typed `CommandError` enum; update
   `cargo_capture`, `sibling_binary`, `missing_binary` signatures. Then
   revisit file 6 to tighten `DiscoverError::CargoCapture` from String
   to LaneError. ONE atomic commit pair.

2. **`verify_verus/verus_tool.rs` file 7** (2 hits): typed `VerusError`
   (already partly-designed but with the right shape); widen
   `record_target_failure` to `error: impl Display` (or change to a
   typed parameter) in `outcome.rs`. One commit pair.

3. **`allow_file.rs` file 5** (5 hits): typed `AllowFileError` enum;
   update `scan.rs` `HotFileError::LoadAllow` to carry the typed source;
   drop the interim String shape. Larger than file 4 because of the
   helpers `malformed_allow` / `overbroad_allow` that need converting
   too. Two-commit pair.

4. **`check_test_integrity/self_test.rs` file 8** (7 hits): defer to a
   later audit if the file is `#[cfg(test)]`-only (verify first). If
   it's selftest-reachable, requires a `SelfTestError` enum and
   widening the test-helper closures. Largest of the four.

## Pattern established

For each future migration:

1. Read the file; identify the `Result<_, String>` sites.
2. Check for existing typed errors in the same module/crates.
3. Define file-local typed enum with one variant per distinct failure
   mode. Use `io::Error` / `serde_json::Error` / `LaneError` directly as
   variant sources rather than String.
4. `impl Display` that preserves the prior user-facing message format
   (so call sites' `format!("...: {e}")` / `eprintln!("...{error}")`
   continue to resolve via Display).
5. `impl std::error::Error` with `source()` chained to a real Error
   source where possible. String-carrying variants return None in
   source() (String doesn't implement Error).
6. Replace call-site `.map_err(|e| format!("..."))` with
   `.map_err(|source| TypedError::Variant { source })`. The boundary
   Display resolves automatically.
7. Gate each file: `cargo fmt --check`, `cargo check`, `cargo test`.
8. Commit with a focused message naming file paths, raw error, typed
   replacement, test evidence (217/0).

The pattern takes 1-3 small edits per file for an isolated hit; 3-5
small edits when one function calls another, and 5+ small edits with
multi-module cascading.
