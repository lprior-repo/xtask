# Stage 3 — F-010 typed-error remainder inventory

## Discrepancy from handoff

Handoff claimed: "62 occurrences of `Result<T, String>` across 19 lane bin
files; 5 of 19 already done."

Inventory with `grep -E 'Result<[^>]*,\s*String>'` against
`crates/*/src/**/*.rs`, filtering out `tests/`, `benches/`,
`examples/`, and `build.rs`:

| metric | handoff | inventory | ratio |
|---|---|---|---|
| Total hits | 62 | 24 | 0.39× |
| Files in production src (excluding `self_test.rs`) | 19 | 9 | 0.47× |
| Total files (including `self_test.rs`) | 19 | 10 | 0.53× |

The handoff overstates scope by 2–2.5×. Five discrepancies from the
handoff prose have now been recorded in this session:

1. **F-003 (closed)**: code had bug; closed in 6da754f
2. **KNOWN_LANES**: check-source-length was in the check-nightly-features
   slot; fixed in 911904a
3. **23% at 10K c-n-f regression**: ≤7% above pre-Rayon baseline, noise;
   closed as no-op in 08f861a
4. **F-010 scope**: 18/9 (not 62/19) per raw grep inventory

## Per-file migration plan (ranked smallest-first)

Order chosen to keep each commit's review surface small: 1-hit files
first, then 2- and 3-hit files, then self_test.rs (7 hits, partly
F-010-incomplete), then commands.rs (3 hits).

| # | file | hits | strategy |
|---|---|---|---|
| 1 | `crates/titania-lanes/src/bin/run_cargo.rs:25` | 1 | `CargoLane::parse` returns String; convert to `Result<Self, CargoLaneParseError>` (1 variant: `Unknown(String)`) |
| 2 | `crates/titania-lanes/src/bin/kani_list.rs:285` | 1 | `validate_json` reads a JSON value to validate it; convert to `Result<(), KaniListError>` (1 variant: `InvalidJson{reason}`) |
| 3 | `crates/titania-lanes/src/bin/forbidden_scan/lane.rs:106` | 1 | `build_forbidden_ac` builds an AC from a slice; convert to `Result<AhoCorasick, ForbiddenAcError>` (1 variant: `Build{names, source}` mapping aho-corasick::BuildError) |
| 4 | `crates/titania-lanes/src/bin/check_hot_cold_forbidden_apis/scan.rs:39` | 1 | `scan_source` reads + scans; convert to `Result<(), ScanError>` (likely 1-2 variants: `Read{path, source}`, `Report{path}`) |
| 5 | `crates/titania-lanes/src/bin/check_hot_cold_forbidden_apis/allow_file.rs:51` | 1 | `validate_allow_entry` validates a single allow entry; convert to `Result<(), AllowEntryError>` (likely 1-2 variants: `MissingField(&'static str)`, `InvalidExpiry(String)`) |
| 6 | `crates/titania-lanes/src/bin/rust_verification_gauntlet.rs:95` | 1 | `discover` finds xtask; convert to `Result<Self, GauntletError>` (1 variant: `MissingXtask`) |
| 7 | `crates/titania-lanes/src/bin/verify_verus/verus_tool.rs` | 2 | typed `VerusError` enum spanning both lines |
| 8 | `crates/titania-lanes/src/bin/check_test_integrity/self_test.rs` | 7 | typed `SelfTestError` enum; spans `run_fixtures`, `with_initialized_repo`, `assert_clean_fixture`, `assert_untracked_ignored_fixture`, `run_git`, `scratch_dir`. **This file was missed by the F-010 typed-error migration commit `363959a`** — same incomplete-closure pattern as F-003 |
| 9 | `crates/titania-lanes/src/bin/rust_verification_gauntlet/commands.rs` | 3 | typed `GauntletCommandError` enum |

Plus: any sibling error enum may need a `#[from] std::io::Error` or
`#[from] titania_lanes::LaneError` conversion impl so the existing
`?` propagation keeps working.

## Migration discipline

Per file:
1. Add typed error enum near top.
2. Replace `String` with the typed enum in the signature.
3. Update all call sites that wrap or unwrap those `String` errors.
4. Run `cargo check --workspace --bins` (gates `forbid(unsafe_code)`
   and the clippy deny set).
5. Run `cargo test --workspace --all-features --no-fail-fast` (must
   stay green at 217 passed).
6. One commit per file with a focused message naming the migrated
   file. Revertable as a unit.

## What this means for the user's "DO all of them" directive

The handoff framed "62 occurrences, 19 files" as the scope. The
actual scope is 24 occurrences across 10 files. Doing all of them is
still the right call, but the work is materially smaller than the
handoff suggested (about 40% of the claimed scope). The bead store's
33-closed count is similarly untrustworthy as a "verified" signal — at
least 4 of those closures (F-003, partial F-010 for self_test.rs,
possibly more) are not actually complete. Do not trust bead-status
alone; verify each closed bead against actual source state before
counting it.
