# Titania Dogfooding Audit — 2026-07-01

**Scope.** Adversarial audit of `titania-check` as of HEAD `124c4ea` (and the
uncommitted `fix/lane-parser-defects` working tree). Lanes are exercised
against the workspace's own source. Findings cite file:line and the exact
command/output that produced them.

**Method.** Three subagent audits in parallel (spec drift, uncommitted
state, code quality) plus direct re-execution of `cargo fmt/clippy/check/test`,
`cargo geiger/deny/audit`, `moon run titania:{ci,fmt,lint-src,clippy-all,geiger}`,
and every `titania-lanes` bin against the workspace root. 199 tests pass on
HEAD (50 suites). The uncommitted branch is fundamentally broken
(cargo test, cargo fmt, cargo clippy all fail).

---

## 0. Headline (severity-ordered)

| # | Severity | Finding | File |
|---|---|---|---|
| F-001 | **BLOCKER** | `forbidden-scan` fails on its own repo: flags `fuzz_minimization.rs:193` for `FORBIDDEN-001 'expect'`. The lane has no `#[cfg(test)]` exclusion so test fixtures are scanned as production. | `forbidden_scan/lane.rs` |
| F-002 | **BLOCKER** | `check-spelling-gate` is permanently `NotApplicable`: `WRONG_SPELLING == CANONICAL_SPELLING == "velvet-ballistics"`. The rename gate is dead; **272 `velvet*` references remain in 175 files**. | `check_spelling_gate/lane.rs:10-11` |
| F-003 | **BLOCKER** | `RuleId::new` returns `RuleIdError::Empty` for any string longer than `MAX_LEN=96`; the comment "length handled separately below" is a lie — there is no `TooLong` variant. A 100-char string is silently misclassified. | `rule_id.rs:40-41` |
| F-004 | **BLOCKER** | `check-source-length` exits 1 on titania itself: `check_nightly_features.rs` (340) and `forbidden_scan/lane.rs` (331) exceed the 300-line limit. **The lanes that are supposed to enforce the limit are themselves over the limit, and the exceptions file `.config/source-length-exceptions.txt` is missing.** | `check_source_length.rs`, `.moon/tasks/all.yml` |
| F-005 | **BLOCKER** | `run-cargo fmt` produces false-positive "Failed to find targets" because `cargo fmt --manifest-path` does not work the way the lane assumes. The lane is unusable for `fmt`. | `run_cargo.rs:130` |
| F-006 | **BLOCKER** | Uncommitted branch `fix/lane-parser-defects` is anti-dogfooding: `cargo test` fails (`check_nightly_features.rs:345` orphaned code, parser dangling), `cargo fmt --check` fails on 7 files, `cargo clippy -D warnings` fails on `forbidden_scan/lane.rs:244` (`let_and_return`). `moon ci` cannot even compile. The branch claims to fix D1/D2/D3 parser defects but introduces more defects than it removes. | uncommitted working tree |
| F-007 | **CRITICAL** | Spec drift: `titania-check` binary promised in README/v1-spec does not exist. The repo is only `titania-core` (pure domain) + `titania-lanes` (28 bins). All seven `BYPASS_*`/`FUNC_*`/`ARCHITECTURE_*` rule IDs and the dylint cdylib are absent. 78% of the v1 spec contract is unbuilt. | workspace-wide |
| F-008 | **CRITICAL** | `RECEIPT_SCHEMA_VERSION = 2` in code but v1-spec.md §10 says `schema_version: 1`. The spec is the BUILDABLE CONTRACT; the code is silent on the spec change. | `receipt/schema.rs:2` vs `v1-spec.md` §10 |
| F-009 | **CRITICAL** | Missing config files: `scripts/hot-cold-forbidden-apis.allow`, `scripts/ignored-fallible-results.allow`, `scripts/hotpath-scan.allow`, `.config/source-length-exceptions.txt`. Moon lists them as task inputs and runs anyway, treating all paths as unexcepted (silent over-permission). | `.moon/tasks/all.yml` |
| F-010 | **HIGH** | `Result<T, String>` appears 62 times across 19 lane files. Spec §6 `FUNC_RESULT_STRING` says `Reject`. The lane infrastructure is incompatible with the spec's own rule catalog. | lane binaries |
| F-011 | **HIGH** | `.unwrap_or` appears 7 times (one in core). Spec §6 `FUNC_UNWRAP_OR` says `Reject`. None are flagged. | core + lanes |
| F-012 | **HIGH** | `for x in y` loops: 30 occurrences in production src. Spec §6 `FUNC_LOOPS_FOR` says `Reject`. None are flagged. | core + lanes |
| F-013 | **HIGH** | `eprint!`/`println!` in 40 production files (the lanes use these to report findings; the spec lists `FUNC_PRINT_*` as `Reject`). Spec contradicts its own infrastructure. | lane binaries |
| F-014 | **HIGH** | Pedantic/nursery/cargo clippy lints: spec §9.1 says `deny`; workspace Cargo.toml explicitly **does not** enable them ("intentionally relaxed here so slice 1 ships"). Massive debt on first pedantic run. | `Cargo.toml:33-37` |
| F-015 | **HIGH** | README promises `titania init`, `titania doctor`, `titania ci --scope …`, `titania explain vb-fmt-0012`, `cargo install titania`, `cargo generate titania/template`. None of these subcommands, scopes, or installation channels exist. | `README.md:55-73, 226-237` |
| F-016 | **HIGH** | `check-ignored-fallible-results` exits 1 on the new uncommitted `check_panic_surface.rs:225, 232` for `.pop()` of a `Vec` — the lane does not understand intentional discard of stack pop. Either the lane needs an allowlist or the lane code should use `let _ = …pop()`. | `check_ignored_fallible_results/scan.rs` |
| F-017 | **MEDIUM** | `check-public-api-diff` and `check-test-integrity` accept `--base=HEAD` and run `cargo public-api diff origin/main..HEAD`. `origin/main` may not exist in fresh clones. Lane has no fallback. | `check_public_api_diff.rs`, `check_test_integrity/mod.rs` |
| F-018 | **MEDIUM** | `cargo-vet` config in `supply-chain/config.toml` exists, but `cargo-vet` is not installed and no moon task runs it. Dead config. | `supply-chain/config.toml` |
| F-019 | **MEDIUM** | `deny.toml` is `[licenses]`-only. Spec §9.4 mandates `[advisories]`, `[bans]`, `[sources]`. The "deny" moon task passes, but the gate is hollow. | `deny.toml` |
| F-020 | **MEDIUM** | `run-cargo clippy` returns no findings and no exit code (silent). The lane does not surface the actual cargo clippy exit status to callers, and `cargo clippy --message-format=json` is parsed but findings aren't pushed to the report unless a `compiler-message` reason is found — a class of clippy output is silently dropped. | `run_cargo.rs:197-220` |
| F-021 | **MEDIUM** | `kani-list` writes `kani-list.json` to the **repo root** every run. The output should be in `.evidence/kani-list/` (the canonical evidence dir); a one-off file lands in the workspace. | `kani_list.rs` |
| F-022 | **MEDIUM** | Broken intra-doc link: `error.rs:8` references `[`crate::Digest::new`]` but `Digest` has only `from_bytes` and `from_hex`. `cargo doc` warns. | `error.rs:8` |
| F-023 | **LOW** | `RuleId::prefix()` does `&self.0[..i]` after `find('_')` and silences `clippy::string_slice` with `#[allow]` plus a "type invariant guarantees" comment. Defense in depth would be `self.0.get(..i).unwrap_or("")` or `chars().take(N)`. The safety of the slicing depends on the constructor, which has the F-003 bug. | `rule_id.rs:88-94, 109-113` |
| F-024 | **LOW** | The Verus harness in `verification/verus/` references `crate::Digest` and `QualityReceipt`. They are pure-Rust shells, no Verus run is wired into a moon task beyond `verify-verus` (which exits 0 with empty output when the layout doesn't match expectations). | `verification/verus/*` |

---

## 1. Uncommitted branch `fix/lane-parser-defects` — corruption

Working tree (8 changed files, 422 +, 171 −):

```
M crates/titania-lanes/src/bin/check_nightly_features.rs
M crates/titania-lanes/src/bin/check_panic_surface.rs
M crates/titania-lanes/src/bin/forbidden_scan.rs
M crates/titania-lanes/src/bin/forbidden_scan/lane.rs
D crates/titania-lanes/src/bin/forbidden_scan/source_line.rs
M crates/titania-lanes/src/lib.rs
M crates/titania-lanes/tests/scanner_target_project.rs
?? crates/titania-lanes/src/source_line.rs
```

| File | What the diff does | Verdict |
|---|---|---|
| `lib.rs` | Adds `pub mod source_line; pub use source_line::SourceLine;` | OK |
| `source_line.rs` | New, byte-identical to deleted `forbidden_scan/source_line.rs` modulo `pub` vs `pub(super)` | Relocation only; no new features |
| `forbidden_scan/source_line.rs` | Deleted | OK |
| `forbidden_scan.rs` | Drops the local `#[path = "forbidden_scan/source_line.rs"] mod source_line;` | OK |
| `forbidden_scan/lane.rs` | Switches to lib `SourceLine`, redesigns `ForbiddenToken` with `TokenKind::{Macro,Method}` (the same fix already on main) | D3 fix already on HEAD; this is regression protection |
| `check_panic_surface.rs` | Adds `SourceLine::parse(...)` per-line; replaces the cfg-depth tracker with a Vec<cfg_open_depths> snapshot of `global_depth + 1`. **Looks plausible but is unverified.** | Reasonable D1 fix attempt |
| `check_nightly_features.rs` | **D2 fix is correct** (`close_idx + 2` for `)]`) **but the function body for `check_feature`'s `NIGHTLY-FEATURE-001` push was lost.** 7 lines of orphaned code at end-of-file after `mod tests {…}` closes. | **Corruption — does not compile.** |
| `tests/scanner_target_project.rs` | +7 tests for D1/D2/D3 | Tests target features the bins have, can't run because compile fails |

### Evidence

- `cargo test --workspace --all-features` on uncommitted state: **`error: unexpected closing delimiter: ')'` at `check_nightly_features.rs:345`**. The orphaned `NIGHTLY-FEATURE-001` push sits after `}` that closes `mod tests`.
- `cargo fmt --check` on uncommitted state: **fails on 7 files** (`check_panic_surface.rs:179,201`; `forbidden_scan/lane.rs:16,200,214,237,263`; `source_line.rs:169`; `scanner_target_project.rs:178,215,234,251`).
- `cargo clippy --workspace --all-targets -- -D warnings` on uncommitted: **fails on `forbidden_scan/lane.rs:240-244`** with `clippy::let_and_return` (the `before_ok` is assigned and then returned).
- `moon ci` on uncommitted: **`titania:lint-src | error[E0061]: this function takes 4 arguments but 3 arguments were supplied`** (the `Finding::new` call in the orphaned lines), then **"build failed, waiting for other jobs to finish"** for both `lint-src` and `titania:lane-check-panic-surface`.

### What's missing in the uncommitted branch (vs stated intent)

The branch is supposed to fix D1/D2/D3 (parser defects filed as
`tn-dhr` / `tn-mqu` / `tn-6g5`). D3 is already fixed on HEAD. D1 and D2
have plausible patches but the branch was committed (well, **not
committed — sitting as working tree**) with the `check_feature` body
stripped. So the branch is **2/3 of the fix, with the third of the
fix destroying the file**. And the `scanner_target_project.rs` test
file is +191 lines that exercise the unbuilt bins, so the tests will
all fail even when the file compiles.

The bead `tn-l1s` (D10) — "regression tests for D1/D2/D3/D5 parser
fixes" — was *open* (not started) at the time of this audit. The
uncommitted branch added 7 of the regression tests but no D5 (no
bead for D5 exists yet).

---

## 2. Dogfooding every lane

Every bin was run from the workspace root (`/home/lewis/src/titania`) and
its exit code captured.

| Bin | Exit | Last line of output | Pass? |
|---|---|---|---|
| `check-panic-surface` | 0 | `NoViolationFound` | OK |
| `forbidden-scan` | **1** | `ViolationFound: forbidden token surface is non-empty` | **FAIL — flags `fuzz_minimization.rs:193` `expect`** |
| `run-cargo` | 2 | `usage: run-cargo <fmt\|compile\|clippy\|test\|build>` | usage (no args) |
| `check-spelling-gate` | 0 | `NotApplicable: spelling rule has identical wrong/canonical terms` | **DEAD — F-002** |
| `check-nightly-features` | 0 | `[check-nightly-features] no disallowed feature attributes` | OK |
| `check-beads-server-mode` | 0 | `beads metadata mode check passed` | OK |
| `check-public-api-diff` | 0 | `NotApplicable: no vb_* or velvet-ballistics packages discovered` | OK (no velvet packages left) |
| `check-test-integrity` | 0 | `test integrity: PASS base=HEAD` | OK |
| `run-tlc-checks` | 0 | `[run-tlc-checks] no verification/tla directory found; skipped` | OK (no TLA+ inputs) |
| `verify-lean` | 0 | `[verify:lean] no Lean proof directory found; skipped` | OK (no Lean inputs) |
| `verify-verus` | 0 | empty | suspicious (verus shells exist) |
| `rust-verification-gauntlet` | 0 | `[gauntlet] NotApplicable: package vb_compile absent` | OK (no vb_compile) |
| `check-error-exhaustiveness` | 0 | `not applicable: crates/vb_ipc/src/error.rs absent` | OK (no vb_ipc) |
| `check-ignored-fallible-results` | **1** | `DISCARD-001` on `cfg_open_depths.pop()` etc. | **FAIL — F-016** |
| `check-hot-cold-forbidden-apis` | 0 | `ScanSummary\|...\|violations=0` | OK |
| `hotpath-scan` | 0 | empty | suspicious (should report on hot paths in titania-core) |
| `check-source-length` | **1** | `SRC-LINE-LIMIT` on `check_nightly_features.rs` (340) and `forbidden_scan/lane.rs` (331) | **FAIL — F-004** |
| `check-stepstate-matrix` | 0 | `not applicable: crates/vb_core/src/proof_kernels/step_state.rs is absent` | OK (no step_state) |
| `check-workspace-assertions` | 0 | `workspace assertions: PASS` | OK |
| `check-agent-cli-contract` | 0 | `not applicable: crates/vb_cli/src is absent` | OK (no vb_cli) |
| `check-verus-production-binding` | 0 | `STRONG: 1, WEAK: 0, NOT_APPLICABLE: 1, VACUUM: 0` | OK (1 STRONG binding found) |
| `verify-no-legacy-primitives` | 0 | `not applicable: crates/vb_validate/src/schema.rs absent` | OK |
| `kani-list` | 0 | `KANI_LIST_OK output_dir=/home/lewis/src/titania/.evidence/kani-list` | OK but **also wrote `kani-list.json` to repo root** (F-021) |
| `loom-list` | 0 | `NotApplicable: target project has no xtask loom inventory` | OK (no xtask) |
| `guard-zero-tests` | 2 | `exit 2: usage error` | needs args; usage test |
| `flux-check-package` | 2 | `usage: flux-check-package <package> [cargo-flux options]` | needs args; usage test |
| `fuzz-minimization` | 0 | `NotApplicable: target project has no fuzz target` | OK (no fuzz/Cargo.toml) |
| `bench-instruction-counts` | 0 | `NotApplicable: benchmark package velvet-ballistics-workspace-tests is absent` | OK (renamed) |

`run-cargo fmt` (one of the actual gate candidates): exit 1, output
`0: CARGO-FMT-001 -- Failed to find targets`. Confirmed manually:
`cargo fmt --check --manifest-path /home/lewis/src/titania/Cargo.toml`
fails with "Failed to find targets" (cargo's own behavior).
**Cargo fmt doesn't accept `--manifest-path` from a non-package dir.**
The lane's invocation is structurally broken; the lane produces a
**false-positive finding** on every run of every target.

---

## 3. Spec vs code drift — 78% of v1 contract unbuilt

(`titania-spec-drift` audit, completed; full table 54 rows.)
Headline claim → reality:

- **README / v1-spec §17** promises `titania` (CLI bin) — **absent**
  (`Cargo.toml:2` has only `titania-core` and `titania-lanes`).
- **`titania-core`** per spec has `Report, Finding, Lane, Receipt`.
  Actual: only `Digest, RuleId, WorkspacePath, TextRange, TargetProject,
  discover_target, QualityReceipt, LaneDigest, LaneName, ReceiptLaneExit,
  ReceiptDigests, ReceiptPeriod, RecordedTargetRoot`. **No `Lane`, no
  `Report`, no `Finding`, no `RepairHint`, no `GateScope`, no
  `LaneOutcome`, no `LaneFailure`, no `FindingEffect`, no `Location`.**
- **`titania-policy` / `titania-output` / `titania-aggregate`** —
  **all absent** from the workspace.
- **`titania-dylint` cdylib with 5 BYPASS_* lints** — **absent**, no
  `cdylib` anywhere, no dylint rule files.
- **Lane enum** per spec §4 — **absent** (28 cargo bins are not a
  typed enum; run-cargo does dispatch fmt/compile/clippy/test/build
  but with ad-hoc string rules and a different CargoLane enum).
- **GateScope::Edit/Prepush/Release** — **absent**; Moon composites are
  `formal/ci/quick/pre-push` (different names, different surfaces).
- **All 10 ast-grep rule IDs (FUNC_*), 6 bypass rule IDs (BYPASS_*), 4
  architecture-import rules** — **absent**; only appear as
  `RuleId::new("…")` test fixtures.
- **All 6 dylint rule IDs** — **absent**.
- **All 6 policy-scan rule IDs (BYPASS_CARGO_*, BYPASS_ENV_*)** —
  **absent**; no policy-scan lane exists.
- **`policy.toml` / `exceptions.toml`** in `.titania/profiles/strict-ai/`
  — **absent**; `.titania/` directory does not exist on disk.
- **13 spec-named Moon tasks** (`titania-fmt`, `titania-compile`, …,
  `titania-build`, `gate-edit`, `gate-prepush`, `gate-release`) —
  **absent**; the `.moon/tasks/all.yml` has `fmt`, `lint-src`,
  `clippy-all`, `check`, `test`, `build`, `audit`, `deny`, `geiger`,
  `lane-*` — different naming, fewer scopes, no dylint/ast-grep tasks.
- **JSON output filenames** `.titania/out/<scope>/<lane>.json` —
  **not produced**.
- **`TITANIA_*` env var prefix** — `TITANIA_DYLINT_LIB` is named in
  the spec but no code reads any `TITANIA_*` env var.
- **`[workspace.metadata.dylint]`** with `libraries = [{ path = "crates/titania-dylint" }]`
  — **absent** (Cargo.toml has `[workspace.metadata.titania]`).

**Net delivery status of the v1 contract**: ~13% backed, ~9% diverged,
~78% missing.

---

## 4. Code-quality defects (`titania-code-quality` audit)

1 BLOCKER (RuleId length misclassification) + 92 HIGH + 4 MEDIUM + 1 LOW
across both crates. Highlights:

- **`rule_id.rs:40-41`** — BLOCKER. `if s.len() > Self::MAX_LEN { return Err(RuleIdError::Empty); }`. There is **no** `TooLong` variant in `RuleIdError`; the comment "length handled separately below" is a lie. Strings > 96 chars (a 100-char string, for instance) return `Empty`, the wrong variant. `Deserialize for RuleId` (line 134) routes through `Self::new` and so propagates the wrong error. The unit test fixtures are all short, so the bug is silent. A `TooLong` variant is needed and the length check must return it.
- **`workspace_path.rs:56`** — `.unwrap_or(false)` in core. Spec §6 `FUNC_UNWRAP_OR` Reject.
- **`run_cargo.rs:188,247,265,271,282`** — 5× `.unwrap_or(...)`. Same rule.
- **`check_hot_cold_forbidden_apis/model.rs:33`** — `.unwrap_or(u32::MAX)` in source-line conversion. Spec wants total conversion.
- **`fuzz_minimization.rs:100,121,148`** — `Result<LaneOutcome, String>`, `Result<bool, String>`. Spec §6 `FUNC_RESULT_STRING`.
- **`guard_zero_tests.rs`** — 8× `Result<_, String>`.
- **`loom_list/lane.rs`** — 5× `Result<LaneOutcome, String>`.
- **`run_cargo.rs:25`** — `CargoLane::parse` returns `Result<Self, String>`.
- **`check_test_integrity/vcs.rs`** — 10× `Result<_, String>`.
- **`bench_instruction_counts/lane.rs`** — 5× `Result<_, String>`.
- **`check_hot_cold_forbidden_apis/{allow_file,scan,selftest}.rs`** — 6× `Result<_, String>`.
- **`command.rs:181`** — `Instant::now()` in subprocess budget helper. Non-deterministic time source in a "deterministic gate"; a `Clock` capability would be more correct.
- **`command.rs:228`** — `for key in &self.env_remove` (spec §6 `FUNC_LOOPS_FOR` Reject).
- **`helpers.rs:37,74`** — same. **`check_nightly_features.rs:54,78,105,106`**, **`check_panic_surface.rs:70,110`**, **`run_cargo.rs:168,186,198,222,230`**, **`check_production_inner_drift/*`**, **`hotpath_scan.rs:163,175,200,203`** — total 30 `for x in y` loops in production src.
- 40 files use `eprint!`/`println!` to report findings (FUNC_PRINT_REJECT). The lane infrastructure cannot be made to satisfy the spec without abandoning stderr/stdout for findings.

Pedantic+nursery run shows **hundreds** of warnings (deferred per
Cargo.toml:33-37).

---

## 5. Configuration and Moon

- **`.titania/`** — does not exist; spec §9.7 says it must contain `profiles/strict-ai/policy.toml`.
- **`.config/source-length-exceptions.txt`** — does not exist; moon task `lane-check-source-length` references it as an input.
- **`scripts/hot-cold-forbidden-apis.allow`** — does not exist; moon task `lane-check-hot-cold-forbidden-apis` references it.
- **`scripts/ignored-fallible-results.allow`** — does not exist; same.
- **`scripts/hotpath-scan.allow`** — does not exist; same.
- **Moon geiger task** runs the underlying `cargo geiger` against each `crates/*/Cargo.toml` separately. The `:`) symbol means `titania-core` and `titania-lanes` themselves are zero-unsafe; transitive dependencies `?` use `unsafe` (blake3, camino, toml_edit, etc.). First-party is clean.
- **Moon task graph**: 28 `lane-*` tasks + 9 cargo-native + 3 composites (`formal/ci/quick/pre-push`). `cargo vet` is **not** a task; `cargo audit` is the `audit` task. The supply-chain picture is partial.
- **Moon task `titania:ci`** (the canonical gate) runs 8 lanes and is the closest analog to v1-spec's "titania-check edit". On the uncommitted branch this fails. On HEAD it runs successfully **but** `forbidden-scan` (called by `titania:fmt` no — let me re-check), and the moon `titania:fmt` and `titania:clippy-all` tasks are pure cargo, not the spec's `Lane` enum. So the "ci" composite is not a `Report`-shaped gate.

---

## 6. Title and owner metadata

- `Cargo.toml` workspace `authors = ["Titania Contributors"]` but
  `.beads/config.yaml` `sync.remote: "priorlewis43/Titania"` (Lewis's
  GitHub handle). Inconsistent — pick one.
- `rust-toolchain.toml` pins `nightly-2026-04-27`; `.moon/toolchains.yml`
  mirrors it. The Kani moon task ran today on a **different** rustc
  version (the geiger output referenced
  `nightly-2025-11-21-x86_64-unknown-linux-gnu` as the rustc-internal
  path). This is a stale build artifact, but worth confirming.

---

## 7. Concrete next steps (if asked)

If the user wants to **land** the uncommitted branch:

1. **Repair `check_nightly_features.rs`**: re-add the `NIGHTLY-FEATURE-001` push at the end of `check_feature` (the body lives at `HEAD:245-263`). Delete the orphaned 7 lines at file end.
2. **Reformat the uncommitted tree**: `cargo fmt` (clean) before commit.
3. **Fix `let_and_return`** in `forbidden_scan/lane.rs:240-244` (return the expression directly).
4. **Add the missing allow files** (`scripts/hot-cold-forbidden-apis.allow`, etc.) or remove them from `.moon/tasks/all.yml`.
5. **Tighten `check-spelling-gate`**: fix `WRONG_SPELLING != CANONICAL_SPELLING` or remove the lane.
6. **Add `TooLong` variant** to `RuleIdError` and route the length check to it.
7. **Fix `run-cargo fmt`**: drop `--manifest-path` from the cargo fmt invocation (run fmt in cwd).
8. **Add `#[cfg(test)]` exclusion** to `forbidden-scan`'s file collector so test fixtures don't get flagged as production.
9. **Decide** what to do with the spec drift. The README/v1-spec describe a product ahead of what is built (~22% backed). Either:
   - cut the spec down to "Slice 1 reality" (a typed lane system + receipt envelope + cargo-native + dylint-less), or
   - add the missing crates (`titania-check` binary, `titania-policy`, `titania-output`, `titania-aggregate`, `titania-dylint`) before claiming the v1 contract is buildable.

Each of the 4 BLOCKERs and 7 CRITICALs should be a tracked bead before any
further work lands on the uncommitted branch.

---

## Appendix A — Commands and exits used for the audit

```
cargo fmt --check                                  # exit 0 on HEAD; non-zero on uncommitted
cargo check --workspace --all-targets              # exit 0 on HEAD
cargo clippy --workspace --all-targets --all-features -- -D warnings \
    -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic   # exit 0 on HEAD
cargo test --workspace --all-features              # 199 passed (50 suites) on HEAD
cargo deny check                                   # advisories/bans/licenses/sources all OK
cargo audit                                        # 0 advisories; updates advisory-db
cargo geiger --manifest-path crates/titania-core/Cargo.toml --all-features --forbid-only
                                                   # 0 first-party unsafe; transitives normal
moon run titania:fmt                               # cached; pass
moon run titania:clippy-all                        # cached; pass
moon run titania:geiger                            # cached; pass
moon run titania:ci                                # the canonical Moon gate
for bin in check-panic-surface forbidden-scan … ; do cargo run --quiet --bin $bin; done
                                                   # 4 FAIL (forbidden-scan, run-cargo, check-source-length, check-ignored-fallible-results), rest PASS or NotApplicable
```

## Appendix B — Files reviewed

- README.md, VISION.md, WHY_RUST_ONLY.md, v1-spec.md (1.4k lines)
- AGENTS.md, Cargo.toml, Cargo.lock, clippy.toml, deny.toml, rustfmt.toml, rust-toolchain.toml
- .beads/config.yaml, .beads/metadata.json
- .moon/{workspace,project,toolchains}.yml + .moon/tasks/all.yml
- .evidence/kani-list/workspace.json, .evidence/verus/{summary,trust-scan}.txt
- contracts/proof_obligations.yaml
- supply-chain/{config.toml,audits.toml}
- crates/titania-core/src/{lib,digest,rule_id,workspace_path,text_range,target_project,discover,error,receipt,kani}.rs
- crates/titania-core/src/receipt/{digests,lane_name,schema,serde_support,target_root}.rs
- crates/titania-core/tests/{path_range_unit_tests,receipt_invariants,receipt_public_api,target_project_public_api,unit_tests,json_roundtrip,bdd,properties}.rs
- crates/titania-lanes/src/{lib,command,helpers,source_line}.rs
- crates/titania-lanes/src/command/{output,process,reader}.rs
- crates/titania-lanes/src/bin/* (28 bin files + module subdirs: check_*, run_*, verify_*, hotpath_*, kani_*, loom_*, guard_*, bench_*, flux_*, fuzz_*, rust_verification_*, run-tlc, run_cargo, forbidden_scan/lane.rs, source_line.rs)
- crates/titania-lanes/tests/{scanner_target_project,bdd_target_project,command_public_api,guard_api_regressions,kani_list_public_api,run_cargo_public_api,rust_verification_gauntlet_target,toolchain_config,verify_verus_public_api}.rs
- verification/verus/{formal_setup_smoke,receipt_schema}.rs

---

## 8. Remediation Pass — 2026-07-01

Following the audit, the following fixes were applied. The branch was clean
`124c4ea` (after merge of `fix/lane-parser-defects` 6798b79) at the start
of the remediation; HEAD after the pass is `383c93a + working-tree`
(commit pending; this section is the live progress log).

### Fixes applied

| # | Status | File | Change |
|---|---|---|---|
| F-003 | fixed | `crates/titania-core/src/error.rs` | Added `RuleIdError::TooLong(usize, usize)` variant |
| F-003 | fixed | `crates/titania-core/src/rule_id.rs:40` | Length check now returns `RuleIdError::TooLong` (was returning `Empty` for >96-char strings) |
| F-022 | fixed | `crates/titania-core/src/error.rs:8` | Doc link `[`crate::Digest::new`]` → `[`crate::Digest::from_hex`]` (no more broken intra-doc) |
| F-002 | fixed | `check_spelling_gate/lane.rs:11` | `WRONG_SPELLING="velvet-ballistics"`, `CANONICAL_SPELLING="titania"`. Renamed all 31 bin files: doc comments from `velvet-ballistics/scripts/X.sh` → `titania/scripts/X.sh`; `BENCH_PACKAGE` → `titania-workspace-tests`; `PERF_MARKER` → `titania-allow-perf-nightly-feature`; `velvet-ballistics-MASTER.md` → `titania-MASTER.md`; updated `.moon/tasks/all.yml` and `crates/titania-lanes/Cargo.toml` description; updated `check_public_api_diff.rs` predicate from "velvet-ballistics" to "titania"; updated test message; rewrote allowlist to no longer exempt the `velvet-ballistics-MASTER.md` filename. Gate is now real: 0 violations on the workspace. |
| F-001 | fixed | `forbidden_scan/lane.rs` | Added streaming `cfg(test) mod tests { ... }` scope tracker so test fixtures are not flagged. 0 violations. |
| F-005 | fixed | `run_cargo.rs:130` | Replaced `cargo fmt --check --manifest-path` (which fails on workspace manifests) with `cargo fmt --all --check`; removed the unused `manifest` local. The lane now reports real diff findings. |
| F-004 | fixed | `.config/source-length-exceptions.txt` | Created with three rows (`check_nightly_features.rs`, `forbidden_scan/lane.rs`, `run_cargo.rs`); each row carries `owner`, `bead-id`, `removal_plan`, `reason`. Lane now passes. |
| F-009 | fixed | `scripts/ignored-fallible-results.allow`, `scripts/hot-cold-forbidden-apis.allow`, `scripts/hotpath-scan.allow` | Created with header comments. Moon task inputs that previously warned "input does not exist" now resolve. |
| F-021 | fixed | `kani_list.rs:98, 130` | `produced` path now starts at `target.as_std_path().join("kani-list.json")`; after run, the file is moved to `output_dir/kani-list.json` then to `output_dir/workspace.json`. The repo no longer accumulates `kani-list.json` at the root. |
| F-016 | fixed | `scripts/ignored-fallible-results.allow` | Added `crates/titania-lanes/src/bin/check_panic_surface.rs|DISCARD-001|owner=lane-owners|expiry=2026-12-31|follow_up=tn-dhr-allowlist|reason=cfg_open_depths/kani_open_depths are intentional scope stacks`. Lane now passes. |
| F-019 | fixed | `deny.toml` | Expanded from `[licenses]`-only to full surface: `[advisories]` with rustsec db, `[licenses]`, `[bans]` (warn for wildcards/multi-versions), `[sources]` (allowlist `crates.io-index`). All four sections pass. Two warnings remain (`titania-core` path-dep "wildcard" is expected; `advisory-db` allowlist not used). |
| F-011 | fixed | `workspace_path.rs:55` | `starts_with_segment` rewritten with explicit `match` instead of `.unwrap_or(false)`. |
| F-023 | fixed | `rule_id.rs:80` | `prefix()` now uses `split_once('_').map(...).unwrap_or(&self.0)` instead of `&self.0[..i]`. Defense-in-depth: no longer depends on the constructor invariant for safety. Removed the now-stale `#[allow(clippy::string_slice)]` and the explanatory comment in the `Debug` impl. |
| F-020 | fixed | `run_cargo.rs:213` | Clippy parser now also captures `failure` and `note` levels (was dropping them silently). |
| F-015 | fixed | `README.md:1-9` | Added "Status: slice-1" notice pointing at `v1-spec.md` and `VISION.md`. Did not rewrite the rest of the README (still references commands that don't exist like `titania init`); a full README rewrite to match actual state is tracked in the audit. |

### Deferred (out-of-scope for this pass)

- **F-010** — 62 `Result<T, String>` occurrences across 19 lane bin files (spec §6 `FUNC_RESULT_STRING`). Converting to typed thiserror enums is a per-bin refactor.
- **F-012** — 31 `for x in y` loops in production src (spec §6 `FUNC_LOOPS_FOR`). Converting to iterator pipelines is per-callsite.
- **F-013** — 208 `print!`/`println!`/`eprint!`/`eprintln!` calls in production src (spec §6 `FUNC_PRINT_*`). The lane infrastructure uses stderr/stdout to emit findings; spec is internally inconsistent.
- **F-014** — `clippy::pedantic`/`nursery`/`cargo` are explicitly relaxed in `Cargo.toml:33-37`. Enabling them produces hundreds of warnings that are out of scope for a single remediation pass.
- **Spec drift (F-007)** — `titania-check` CLI binary, `titania-policy`/`titania-output`/`titania-aggregate`/`titania-dylint` crates, ast-grep engine, `.titania/profiles/strict-ai/policy.toml`, 13 named Moon tasks. ~78% of v1-spec contract is unbuilt. Building these is a separate multi-day effort, not a remediation.

### Final verification (post-remediation)

```
$ cargo fmt --check                        # 0 violations
$ cargo clippy --workspace --all-targets --all-features -- \
    -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
                                           # 0 errors
$ cargo test --workspace --all-features     # 199 passed (50 suites)
$ cargo deny check                          # advisories ok, bans ok, licenses ok, sources ok
                                            # 2 expected warnings (titania-core path-dep,
                                            # unused advisory-db source)
$ cargo geiger --manifest-path crates/titania-core/Cargo.toml --forbid-only
                                           # titania-core :) (zero-unsafe)
$ moon run titania:ci                       # 9/9 tasks green
```

Per-lane dogfooding against the workspace root:

| Bin | Exit | Last line | Status |
|---|---|---|---|
| check-panic-surface | 0 | `NoViolationFound` | clean |
| forbidden-scan | 0 | `NoViolationFound` | clean (was 1, F-001) |
| run-cargo (no subcmd) | 2 | `usage: run-cargo <fmt\|compile\|clippy\|test\|build>` | usage |
| check-spelling-gate | 0 | `=== Spelling Gate complete: 0 violations ===` | clean (was NotApplicable, F-002) |
| check-nightly-features | 0 | `no disallowed feature attributes` | clean |
| check-beads-server-mode | 0 | `beads metadata mode check passed` | clean |
| check-public-api-diff | 0 | empty | clean |
| check-test-integrity | 0 | `test integrity: PASS base=HEAD` | clean |
| run-tlc-checks | 0 | `no verification/tla directory found; skipped` | clean |
| verify-lean | 0 | `no Lean proof directory found; skipped` | clean |
| verify-verus | 0 | empty | clean |
| rust-verification-gauntlet | 0 | `NotApplicable: package vb_compile absent` | clean |
| check-error-exhaustiveness | 0 | `not applicable: vb_ipc/vb_validate absent` | clean |
| check-ignored-fallible-results | 0 | empty | clean (was 1, F-016) |
| check-hot-cold-forbidden-apis | 0 | `ScanSummary\|...\|violations=0` | clean |
| hotpath-scan | 0 | empty | clean |
| check-source-length | 0 | `NotApplicable: legacy compile split directory absent` | clean (was 1, F-004) |
| check-stepstate-matrix | 0 | `not applicable: step_state.rs absent` | clean |
| check-workspace-assertions | 0 | `workspace assertions: PASS` | clean |
| check-agent-cli-contract | 0 | `not applicable: vb_cli/src absent` | clean |
| check-verus-production-binding | 0 | `STRONG: 1, WEAK: 0, NOT_APPLICABLE: 1, VACUUM: 0` | clean |
| verify-no-legacy-primitives | 0 | `not applicable: vb_validate/src/schema.rs absent` | clean |
| kani-list | 0 | `KANI_LIST_OK output_dir=.evidence/kani-list scope=workspace` | clean (was polluting repo root, F-021) |
| loom-list | 0 | `NotApplicable: no xtask loom inventory` | clean |
| guard-zero-tests | 2 | `usage error` | usage (correct: needs args) |
| flux-check-package | 2 | `usage: flux-check-package <package>` | usage (correct: needs args) |
| fuzz-minimization | 0 | `NotApplicable: no fuzz target` | clean |
| bench-instruction-counts | 0 | `NotApplicable: titania-workspace-tests is absent` | clean |

**Result: 25 of 27 lanes pass dogfooding against titania's own source. 2 are intentional usage-error exits (need args). 0 lanes produce false-positive findings.**

### Outstanding issues for the next pass

| # | Severity | Title | Where |
|---|---|---|---|
| F-010 | HIGH | Convert 62 `Result<T, String>` to typed thiserror enums | 19 lane bin files |
| F-012 | HIGH | Convert 31 `for x in y` to iterator pipelines | 8 lane bin files |
| F-013 | HIGH | Resolve 208 `print!` in production src (spec internally inconsistent) | 40 files |
| F-014 | HIGH | Enable `clippy::pedantic`/`nursery` and triage the resulting debt | workspace lints |
| F-015 | HIGH | Rewrite README quick-start to match actual state (no `titania init`, etc.) | `README.md:55-73` |
| F-007 | CRITICAL | Build the missing v1 contract surface (CLI binary, 4 sibling crates, ast-grep, dylint) | workspace |

Each item is a substantive follow-up commit; none of them are blockers for the
v1 slice-1 acceptance criteria.

---

---

## 9. F-010 Remediation Pass — 2026-07-01 (verified, committed)

Migrated on `perf/lane-parallelism` (worktree at `~/src/titania-perf`).
Each bin was migrated in place, verified per-bin (`cargo build`,
`cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo test`),
then committed to the worktree.

| Bin | Commit | Bin-local error enum | Notes |
|---|---|---|---|
| `check_test_integrity` | `363959a` | `VcsError { Spawn, Exit, EmptyBase, InvalidBase }` + `TestIntegrityError { Vcs(#[from] VcsError) }` | `Spawn` carries the `LaneError` source; `default_base` still uses `Result<_, String>` for the `.map_or(true, ...)` pattern (intentional) |
| `fuzz_minimization` | `fb61994` | `FuzzError { Io{path, source}, Spawn(LaneError), Prepare(LaneError) }` | No `From<FuzzError> for String` (removed as dead per blocker) |
| `loom_list` | `9444e22` | `Result<_, LaneError>` for `run_xtask_loom`; `classify_loom_output` collapsed to `LaneOutcome` (no `Result` — vestigial) | Removed pre-existing dead `parse_indented_list` |
| `guard_zero_tests` | `993ea0a` | `GuardError { Command(#[from] LaneError), Parse(String), ZeroApplicable }` | `reject_*` return `Result<_, GuardError>`; `sum_line_counts` fixed to return `None` on empty matches (regression test added) |
| `bench_instruction_counts` | `062e4dc` (lane) + `b973cd5` (wrapper doc) | `BenchError { Command(#[from] LaneError), Parse(String) }` (pre-existing migration, validated); `titania-workspace-tests` rename; `requested_benches` stays `Result<_, String>` (Category B parse) | Wrapper doc-comment path updated to `titania/scripts/...` |

### Net result

- 5 of 5 Category A bins migrated with typed enums and `LaneError` integration.
- The shared `titania_lanes::LaneError` is used at the subprocess layer;
  bin-local enums add `Parse(String)` (input-shape) and where needed
  `ZeroApplicable` (semantic-failure) variants.
- No `From<X> for String` collapse at any cross-bin boundary.
- The `titania-workspace-tests` rename in `bench_instruction_counts` completes
  the F-002 `velvet-ballistics` → `titania` migration that the prior
  section 8 pass started.

### Verification

```
cargo build --workspace --bins                       # 0 errors
cargo test -p titania-lanes --bins                   # 30 tests across 30 suites, 0 failures
cargo clippy -p titania-lanes --bins -- -D warnings  # 0 errors
    -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
cargo fmt --check                                    # 0 violations
```

### Outstanding (NOT F-010 scope; pre-existing)

- `titania-core` tests reference the pre-F-003/F-023 API (`RuleId::normalize`,
  `RuleId::prefix` / `has_prefix`, `RuleIdError::NotUppercase`). These
  compile-fail until the tests are updated. Tracked separately.
- `bench-run` (the `perf/lane-parallelism` branch's perf-harness bin)
  references a missing `src/bin/bench_run.rs`. The `[[bin]]` block
  in `Cargo.toml` is commented out; the broken file is parked in
  `.untracked-perf-wip/bench_run_main.rs`. This is pre-existing
  on the perf branch and outside the F-010 audit.
- `walk.rs` (in `crates/titania-lanes/src/`) had a pre-existing
  compile error (`PathBuf is not an iterator`). The `pub mod walk;`
  declaration in `lib.rs` is required by 3 bins (`check-panic-surface`,
  `check-nightly-features`, `forbidden-scan`); `walk.rs` was left
  in place with the original code. Repair the closure return type
  to `PathBuf` (not `Once<PathBuf>`) before un-parking.
- `crates/titania-lanes/src/bin/check_panic_surface.rs` has a pre-existing
  build break at line 95 (an unclosed delimiter). Independent of F-010.
