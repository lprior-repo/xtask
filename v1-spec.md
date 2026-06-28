# Xtask v1 Specification

> **Status: BUILDABLE CONTRACT** — Moon 2026
> Companion: [`VISION.md`](./VISION.md) (the grand ambition)

This document is the implementation contract for xtask v1. Every section is
actionable. If a developer reads this and cannot determine what to build,
the spec is wrong — fix the spec, not the developer.

---

## 1. Product Statement

Xtask v1 is the typed Rust quality layer for Moon CI/CD. It enforces the
`strict-ai` coding standard via typed lanes, structured findings, and a
reproducible `QualityReceipt`. Distribution is a single binary plus a
co-located dylint library, orchestrated by Moon's task graph.

**What v1 proves:** the code conforms to strict structural and style rules,
compiles cleanly, passes tests, and has a clean supply chain.

**What v1 does NOT prove:** panic-freedom (v1.5), type invariants (v2.0),
functional correctness (v2.5). These are the verification batches — see
[`VISION.md`](./VISION.md) §3 and §14 below.

---

## 2. Scope

### In scope for v1

- Moon CI/CD as orchestration substrate (required dependency)
- Single binary `xtask` (CLI + lane runners + aggregator + doctor + explain)
- Co-located dylint dynamic library for type-aware lint scans
- ast-grep embedded as Rust dependency for structural rules
- Built-in cargo lanes: `fmt`, `check`, `clippy`, `test`
- `cargo-deny` for supply chain (advisories, licenses, bans, sources, dupes)
- `panic-scan` lane (rg-based, parser-prefiltered)
- `strict-ai` policy as the only profile
- 3 scope tiers: `edit`, `prepush`, `release`
- `QualityReceipt` with 4 digests (source, lock, policy, toolchain)
- `cargo generate xtask/template` for workspace adoption
- Cross-platform (Unix + Windows)

### Out of scope for v1 (deferred)

See §14 for the full deferred roadmap. Summary:

- Formal verification: Kani, Flux, Verus
- Mutation testing: cargo-mutants
- Supply chain depth: cargo-vet, cargo-geiger, cargo-machete, cargo-audit
- Concurrency testing: Shuttle, Loom
- Deterministic simulation: Antithesis
- Anti-circular meta-policy enforcement
- Init wizard (use `cargo generate` template)
- Multiple policy profiles
- Persistent Receipt ledger

---

## 3. Architecture Overview

```
┌──────────────────────────────────────────────────────┐
│  Moon CI/CD                                          │
│  (.moon/tasks/all.yml — lane DAG, composites)       │
│                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ xtask run-lane│  │ xtask run-lane│  │ xtask      │ │
│  │   fmt        │  │   clippy     │  │ aggregate  │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬─────┘ │
│         │                 │                  │       │
│         ▼                 ▼                  ▼       │
│  .xtask/out/<lane>.json  (typed findings)            │
│         │                 │                  │       │
│         └─────────────────┼──────────────────┘       │
│                           ▼                          │
│                    Report + Receipt                  │
└──────────────────────────────────────────────────────┘

xtask binary:
  ├── CLI dispatcher (gate, run-lane, aggregate, doctor, explain)
  ├── Lane runners (shell out to cargo/clippy/rg/cargo-deny/ast-grep/dylint)
  ├── ast-grep rules (embedded via include_str!)
  ├── Policy loader (strict-ai defaults + file overrides)
  ├── Digest computation (source, lock, policy, toolchain)
  ├── Report/Receipt serialization (versioned JSON)
  └── Doctor (tool presence + version checks)

dylint library (separate .so/.dylib/.dll, co-located):
  ├── BYPASS_PUB_ALLOW lint
  ├── BYPASS_REQUIRED_LINT_WEAKENING lint
  └── BYPASS_ATTR_CONTEXT lint
```

---

## 4. The Lane DAG

### Lanes

| Lane | Tool | Scope | Depends on compile? | What it rejects |
|---|---|---|---|---|
| `fmt` | `cargo fmt --check` | edit | No | formatting drift |
| `check` | `cargo check --workspace --frozen` | edit | — | compile errors, denied rustc lints |
| `clippy` | `cargo clippy --workspace --lib --bins --frozen` with `-F` | edit | Yes | denied clippy lints |
| `ast-grep` | embedded ast-grep-core | edit | No | structural rule violations |
| `dylint` | `cargo dylint xtask` (loads libxtask_dylint) | edit | No | type-aware bypass violations |
| `panic-scan` | `rg` with parser prefilter | edit | No | production assert!/unreachable! |
| `test` | `cargo test --workspace --frozen -- --test-threads=1` | prepush | Yes | failing tests |
| `deny` | `cargo deny check` | prepush | No | advisories, licenses, bans, sources, dupes |
| `build` | `cargo build --workspace --release --frozen` | release | Yes | release build failure |

### Scopes (composite gates)

```
edit     = fmt + check + clippy + ast-grep + dylint + panic-scan
prepush  = edit + test + deny
release  = prepush + build
```

### Skip rules

- Lanes marked "Depends on compile?" skip with `SkipReason::PriorCompilationFailure`
  if `check` fails.
- All other lanes run regardless.
- `ast-grep` and `dylint` run on source files regardless of compilation status.

### Execution model

Each lane is a Moon task. Moon handles:
- Parallel execution of independent lanes
- Per-lane `CARGO_TARGET_DIR` isolation (breaks cargo lock contention)
- 3-layer caching (sccache + bazel-remote + Cargo)
- Affected-file detection (`moon ci` skips lanes whose `@globs(sources)` are unchanged)
- CI integration via `moonrepo/setup-toolchain@v0` + `moonrepo/run-report-action@v1`

---

## 5. Enforcement Layers

The strict-ai standard is enforced by multiple layers, each catching what the
previous cannot:

| Layer | Tool | Catches | Cannot catch |
|---|---|---|---|
| 0 | `cargo fmt --check` | formatting drift | — |
| 1 | `cargo check` + rustc lints | compile errors, `unsafe_code=forbid` | — |
| 2 | `cargo clippy` with `-F` critical lints | unwrap/expect/panic/todo/indexing/arithmetic | third-party panics, macro-expanded panics |
| 3 | `ast-grep` embedded rules | structural patterns, bypass attribute presence | type-aware context |
| 4 | `dylint` library | type-aware bypass detection (scope, required-lint weakening) | — |
| 5 | `panic-scan` (rg) | production `assert!`/`unreachable!` macros | block comments, string literals (heuristic) |
| 6 | `cargo test` | behavior failures | missing tests |
| 7 | `cargo deny check` | supply chain violations | unknown vulnerabilities |
| 8 | `cargo build --release` | release-mode build failures | runtime behavior |

---

## 6. ast-grep Rule Catalog

Rules embedded in the binary via `include_str!("rules/*.yml")`. Each rule
produces a `Finding` with `FindingEffect::Reject` unless noted.

### Structural rules (functional-style enforcement)

| Rule ID | Pattern | Effect | Notes |
|---|---|---|---|
| `FUNC_LOOPS_FOR` | `for $LOOP in $ITER { ... }` in production source | Reject | Excludes tests/benches/examples/build.rs |
| `FUNC_LOOPS_WHILE` | `while $COND { ... }` in production source | Reject | Same exclusions |
| `FUNC_LOOPS_LOOP` | `loop { ... }` in production source | Reject | Same exclusions |
| `FUNC_NESTING_DEPTH` | Nesting depth > 2 (AST depth measurement) | Reject | Measured from function body root |
| `FUNC_RESULT_STRING` | `Result<$T, String>` type annotation | Reject | Pattern match on type |
| `FUNC_RECURSION_DIRECT` | Function calls itself by name | Reject | Syntactic only (cannot catch mutual/trait/fn-pointer) |
| `FUNC_PRINT_STDOUT` | `print!` or `println!` in production source | Reject | Use structured logging instead |
| `FUNC_PRINT_STDERR` | `eprint!` or `eprintln!` in production source | Reject | Use structured logging instead |
| `FUNC_UNWRAP_OR` | `.unwrap_or($X)` / `.unwrap_or_default()` / `.unwrap_or_else($F)` | Reject | Style rule — `unwrap_or` does not panic |
| `FUNC_WILDCARD_IMPORT` | `use $PATH::*;` | Informational | Advisory; does not reject |

### Bypass detection rules (policy consistency)

| Rule ID | Pattern | Effect | Notes |
|---|---|---|---|
| `BYPASS_ALLOW_ATTR` | `#[allow($LINT)]` on any item | Reject | Must be in checked-in policy with owner/reason/expiry |
| `BYPASS_EXPECT_ATTR` | `#[expect($LINT)]` on any item | Reject | Same policy requirement |
| `BYPASS_CFG_ATTR_ALLOW` | `cfg_attr($COND, allow($LINT))` | Reject | Same policy requirement |
| `BYPASS_CRATE_ALLOW` | `#![allow($LINT)]` at crate level | Reject | Same policy requirement |
| `BYPASS_CRATE_EXPECT` | `#![expect($LINT)]` at crate level | Reject | Same policy requirement |
| `BYPASS_SEMGREP_IGNORE` | `// nosemgrep` or `// nosemgrep: $ID` | Reject | Suppresses structural rules |

### Non-Rust bypass detection (TOML scanning, not ast-grep)

| Rule ID | Target | Effect | Notes |
|---|---|---|---|
| `BYPASS_CARGO_LINTS_WEAKENING` | `Cargo.toml` `[lints]` sections | Reject | Detects lowering of required lints |
| `BYPASS_CARGO_CONFIG_WRAPPER` | `.cargo/config.toml` `rustc-wrapper` | Reject | Must match policy-allowed wrappers (sccache only) |
| `BYPASS_CARGO_CONFIG_FLAGS` | `.cargo/config.toml` `rustflags` | Reject | Must match policy-allowed flags |
| `BYPASS_ENV_RUSTFLAGS` | `$RUSTFLAGS` / `$CARGO_ENCODED_RUSTFLAGS` | Reject | Unexpected values at runtime |
| `BYPASS_ENV_RUSTC_WRAPPER` | `$RUSTC_WRAPPER` / `$RUSTC_WORKSPACE_WRAPPER` | Reject | Unexpected wrapper at runtime |
| `BYPASS_ENV_RUSTC_BOOTSTRAP` | `$RUSTC_BOOTSTRAP` set | Reject | Always a violation |

---

## 7. dylint Rule Catalog

The dylint library (`libxtask_dylint`) contains type-aware lints that ast-grep
cannot express. These run via `cargo dylint xtask` which loads the library
into the clippy driver process.

| Rule ID | What it detects | Why dylint (not ast-grep) |
|---|---|---|
| `BYPASS_PUB_ALLOW` | `#[allow(...)]` on `pub` items specifically (not `pub(crate)`) | Needs scope/visibility resolution |
| `BYPASS_REQUIRED_LINT_WEAKENING` | `#![allow(...)]` of lints that are in the required set | Needs knowledge of which lints are required (clippy config awareness) |
| `BYPASS_ATTR_CONTEXT` | `#[allow]` in proc-macro-expanded code vs hand-written source | Needs macro expansion context |
| `BYPASS_INTERNAL_UNSTABLE` | `#[allow_internal_unstable]` | Rare but critical escape hatch |
| `BYPASS_INTERNAL_UNSAFE` | `#[allow_internal_unsafe]` | Rare but critical escape hatch |

---

## 8. The strict-ai Policy

The policy is the single source of truth for what "strict Rust" means. It is
embedded as defaults in the xtask binary and overridable only via checked-in
policy files (`.xtask/profiles/strict-ai/policy.toml`). Changes require
CODEOWNER approval.

### 8.1 Cargo.toml `[workspace.lints]`

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unused_must_use = "deny"
unreachable_pub = "deny"
rust_2018_idioms = { level = "deny", priority = -1 }
non_exhaustive_omitted_patterns = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
cargo = { level = "deny", priority = -1 }
multiple_crate_versions = { level = "allow", priority = 1 }

# Restriction lints: specific critical ones denied
unwrap_or_default = "deny"
exit = "deny"
default_numeric_fallback = "deny"
missing_errors_doc = "deny"
```

### 8.2 Clippy critical lints via `-F` (command line, cannot be `#[allow]`'d)

```bash
cargo clippy --workspace --lib --bins --frozen -- \
  -F clippy::unwrap_used \
  -F clippy::expect_used \
  -F clippy::panic \
  -F clippy::panic_in_result_fn \
  -F clippy::todo \
  -F clippy::unimplemented \
  -F clippy::indexing_slicing \
  -F clippy::string_slice \
  -F clippy::get_unwrap \
  -F clippy::arithmetic_side_effects \
  -F clippy::dbg_macro \
  -F clippy::as_conversions \
  -F clippy::let_underscore_must_use \
  -F clippy::await_holding_lock \
  -D warnings
```

**Critical:** clippy runs `--lib --bins` only, NOT `--all-targets`. Tests
compile via `cargo test` and are behavior-gated, NOT style-gated.

### 8.3 clippy.toml thresholds

```toml
too-many-lines-threshold = 40
too-many-arguments-threshold = 5
max-fn-params-bools = 1
```

### 8.4 deny.toml (supply chain)

```toml
[advisories]
db-path = "~/.cargo/advisory-dbs"
db-urls = ["https://github.com/rustsec/advisory-db"]
yanked = "deny"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC", "Unicode-DFS-2016"]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rustsec/advisory-db"]
```

### 8.5 Hermeticity rules

- `--frozen` everywhere (not `--locked`). `--frozen` = `--locked` + `--offline`.
- `rust-toolchain.toml` pinned (Moon-managed via `syncToolchainConfig`).
- `CARGO_HOME` set to a controlled, read-only directory.
- `RUSTUP_HOME` same.
- Reject parent-directory cargo configs.
- `.cargo/config` (extensionless) AND `.cargo/config.toml` both checked.
- `RUSTC_BOOTSTRAP` = violation.
- `RUSTC_WRAPPER` must be sccache or absent.

### 8.6 Generated code

`include!(concat!(env!("OUT_DIR"), ...))` is banned. Generated Rust must be
checked into source and gated like normal code.

---

## 9. Domain Model

### Report (single disjoint root)

```rust
pub enum Report {
    Pass {
        receipt: QualityReceipt,
        per_lane: Box<[LaneOutcome]>,
    },
    Reject {
        code_findings: Box<[Finding]>,
        gate_failures: Box<[LaneFailure]>,
        per_lane: Box<[LaneOutcome]>,
    },
    PolicyError {
        diagnostics: Box<[PolicyDiagnostic]>,
    },
    InputError {
        diagnostics: Box<[InputDiagnostic]>,
    },
}
```

A run CAN produce both code findings AND tool failures. `reject_kind()`
returns `CodeOnly | GateOnly | Mixed`.

### LaneOutcome

```rust
pub enum LaneOutcome {
    Clean { evidence: LaneEvidence },
    Findings(Box<[Finding]>),
    Failed(LaneFailure),
    Skipped(SkipReason),
}

pub enum SkipReason {
    PriorCompilationFailure,
    NotSelectedByScope,
    NotApplicable,
    PolicyDisabled,
}
```

### Finding

```rust
pub struct Finding {
    pub lane: Lane,
    pub rule_id: RuleId,
    pub location: Location,
    pub message: String,
    pub repair: RepairHint,
    pub effect: FindingEffect,
}

pub enum FindingEffect {
    Reject,
    Informational,
}

pub enum Location {
    Span { file: WorkspacePath, line_start: u32, col_start: u32, line_end: u32, col_end: u32 },
    Manifest { file: WorkspacePath },
    Workspace,
    Tool { name: String, version: String },
}

pub enum RepairHint {
    Patch { file: String, range: TextRange, replacement: String },
    RequiresHumanReview { note: String },
}
```

Line/column: 1-based lines, 0-based columns (Unicode scalar values).
`TextRange` uses byte offsets for deterministic patching.

### LaneFailure

```rust
pub enum LaneFailure {
    InfraFailure { tool: String, reason: String },
    ToolFailure { tool: String, termination: ProcessTermination },
    ResourceFailure { tool: String, limit: String },
    SuspiciousFailure { tool: String, evidence: String },
}

pub enum ProcessTermination {
    Exited { code: i32 },
    Signaled { signal: i32 },
    TimedOut,
    MemoryLimitExceeded,
    SpawnFailed,
}
```

### QualityReceipt

```rust
pub struct QualityReceipt {
    pub schema_version: u16,
    pub scope: GateScope,
    pub source_digest: Digest,
    pub cargo_lock_digest: Digest,
    pub policy_digest: Digest,
    pub toolchain_digest: Digest,
    pub lanes: Box<[LaneReceipt]>,
}

pub struct LaneReceipt {
    pub lane: Lane,
    pub evidence_digest: Digest,
    pub clean: bool,
}

pub enum GateScope { Edit, Prepush, Release }
```

**4 digests only** for v1. The 4 additional digests from the v6.0 spec
(`dependency_source_digest`, `advisory_db_digest`, `feature_profile_digest`,
`mutation_baseline_digest`) are deferred — they correspond to tools not in v1.

---

## 10. CLI Surface

```
xtask gate [--scope edit|prepush|release] [--emit json] [--out <path>]
    Run scoped quality lanes via Moon. Emit report JSON + quality receipt on pass.

xtask run-lane <lane-name>
    Run a single lane and write findings to .xtask/out/<scope>/<lane>.json.
    Invoked by Moon tasks.

xtask aggregate --scope <scope> [--emit json] [--out <path>]
    Read all lane outputs for the scope, assemble Report, emit JSON.
    Invoked by Moon composite gate tasks.

xtask doctor [--scope <scope>]
    Report required tools, installed status, and versions for the scope.

xtask explain <rule-id>
    Print rule description and metadata.
```

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Pass |
| 1 | Reject (code findings and/or gate failures) |
| 2 | PolicyError |
| 3 | InputError |
| ≥4 | Internal error |

---

## 11. Moon Integration

### .moon/toolchains.yml

```yaml
rust:
  version: '<pinned-stable>'
  bins:
    - 'cargo-deny@latest'
    - 'cargo-binstall@latest'
  syncToolchainConfig: true
```

### .moon/tasks/all.yml (xtask-managed)

```yaml
# Lane runners — each invokes xtask run-lane
xtask-fmt:
  command: 'xtask run-lane fmt'
  toolchains: [rust]
  options: { runInCI: true }
  inputs: ['@globs(sources)', 'rustfmt.toml', '.xtask/**']

xtask-check:
  command: 'xtask run-lane check'
  env:
    CARGO_TARGET_DIR: '${workspace.root}/.xtask/cache/check'
  toolchains: [rust]
  options: { runInCI: true }
  inputs: ['@globs(sources)', 'Cargo.toml', 'Cargo.lock', '.cargo/**', 'rust-toolchain.toml']

xtask-clippy:
  command: 'xtask run-lane clippy'
  env:
    CARGO_TARGET_DIR: '${workspace.root}/.xtask/cache/clippy'
  toolchains: [rust]
  deps: [':xtask-check']
  options: { runInCI: true }
  inputs: ['@globs(sources)', 'clippy.toml', 'Cargo.toml', '.xtask/**']

xtask-ast-grep:
  command: 'xtask run-lane ast-grep'
  options: { runInCI: true }
  inputs: ['@globs(sources)']

xtask-dylint:
  command: 'xtask run-lane dylint'
  toolchains: [rust]
  options: { runInCI: true }
  inputs: ['@globs(sources)']

xtask-panic-scan:
  command: 'xtask run-lane panic-scan'
  options: { runInCI: true }
  inputs: ['@globs(sources)']

xtask-test:
  command: 'xtask run-lane test'
  env:
    CARGO_TARGET_DIR: '${workspace.root}/.xtask/cache/test'
  toolchains: [rust]
  deps: [':xtask-check']
  options: { runInCI: true }
  inputs: ['@globs(sources)', '@globs(tests)']

xtask-deny:
  command: 'xtask run-lane deny'
  options: { runInCI: true }
  inputs: ['Cargo.lock', 'deny.toml']

xtask-build:
  command: 'xtask run-lane build'
  env:
    CARGO_TARGET_DIR: '${workspace.root}/.xtask/cache/release'
  toolchains: [rust]
  deps: [':xtask-check']
  options: { runInCI: true }
  inputs: ['@globs(sources)']
  outputs: ['target/release/xtask']

# Composite gates
gate-edit:
  command: 'xtask aggregate --scope edit'
  deps: [':xtask-fmt', ':xtask-check', ':xtask-clippy', ':xtask-ast-grep', ':xtask-dylint', ':xtask-panic-scan']
  options: { runInCI: true }

gate-prepush:
  command: 'xtask aggregate --scope prepush'
  deps: [':gate-edit', ':xtask-test', ':xtask-deny']
  options: { runInCI: true }

gate-release:
  command: 'xtask aggregate --scope release'
  deps: [':gate-prepush', ':xtask-build']
  options: { runInCI: true }
```

### Cache layers

| Layer | Tool | Scope |
|---|---|---|
| 1 | sccache (rustc artifacts) | Global, all Rust projects |
| 2 | bazel-remote (Moon task outputs, AC+CAS) | Per-machine, all Moon projects |
| 3 | Cargo incremental (per-lake `CARGO_TARGET_DIR`) | Per-lane, per-repo |

Per-lane `CARGO_TARGET_DIR` breaks cargo lock contention. sccache recovers
duplicate-compile cost across lanes.

---

## 12. Distribution

### Binary installation

```bash
# Primary: cargo-binstall (pre-built binary + dylint library)
cargo binstall xtask

# Alternative: cargo install (builds from source)
cargo install xtask
```

Both install the `xtask` binary AND the co-located `libxtask_dylint` library
(`.so`/`.dylib`/`.dll`). xtask locates the library via:
1. `XTASK_DYLINT_LIB` env var (explicit override)
2. Co-located path relative to the xtask binary (default)

### Workspace adoption

```bash
# Scaffold a new Rust workspace with xtask + Moon configured
cargo generate xtask/template
```

The template provides:
- `.moon/workspace.yml` (Moon v2 config, bazel-remote hook)
- `.moon/toolchains.yml` (pinned Rust + cargo-binstall list)
- `.moon/tasks/all.yml` (the lane DAG from §11)
- `.xtask/profiles/strict-ai/policy.toml` (strict-ai defaults)
- `.cargo/config.toml` (sccache wrapper hook)
- `clippy.toml` (thresholds)
- `rustfmt.toml`
- `deny.toml`
- `rust-toolchain.toml` (Moon-managed)
- `Cargo.toml` with `[workspace.lints]` from §8.1

### Prerequisites

Adopting repos must have installed:
- Moon v2+ (orchestration)
- Rust toolchain (Moon-managed via `.moon/toolchains.yml`)
- `rg` (for panic-scan lane)
- sccache (optional but recommended — transparent if installed)
- bazel-remote (optional — for team cache sharing)

`xtask doctor --scope <scope>` verifies the right tools are present and bails
with `InputError` if not.

---

## 13. Definition of Done

Xtask v1 is done when:

1. `xtask gate --scope edit` runs fmt, check, clippy, ast-grep, dylint, and
   panic-scan lanes via Moon.
2. `xtask gate --scope prepush` adds test and cargo-deny lanes.
3. `xtask gate --scope release` adds release build.
4. Each lane writes typed findings to `.xtask/out/<scope>/<lane>.json` with
   schema versioning.
5. `xtask aggregate --scope <scope>` reads lane outputs and produces a typed
   `Report` JSON.
6. The report schema is stable, versioned, machine-readable, and separates
   code findings from gate/tool failures.
7. The `strict-ai` policy forbids first-party unsafe, unwrap/expect, panic
   macros, unchecked indexing, unchecked arithmetic, unapproved lint
   suppressions, imperative loops, excessive nesting, and core
   `Result<T, String>`.
8. All policy exceptions live in checked-in policy files with owner, reason,
   and expiry.
9. `xtask doctor --scope <scope>` reports the exact tools required for that
   scope and verifies presence + versions.
10. `cargo generate xtask/template` produces a working Moon+xtask-configured
    Rust workspace that passes `xtask gate --scope prepush` out of the box.
11. Xtask's own repository passes `xtask gate --scope release`.
12. The dylint library is co-located with the binary and loads correctly via
    `cargo dylint xtask`.

### Killer demo

An AI writes Rust with a `for` loop and `.unwrap()`. `xtask gate --scope edit`
rejects with typed `FUNC_LOOPS_FOR` and `HOLZMAN_PANIC_UNWRAP` findings, each
with a `RepairHint`. The AI repairs. The gate passes and emits a
`QualityReceipt` with source/policy/toolchain digests.

---

## 14. Deferred to v1.5+ (Roadmap)

These items are explicitly out of scope for v1. They are listed here to
prevent scope creep during implementation.

### v1.5 — Kani panic-freedom

- `cargo kani` lane in `full` scope (new scope tier)
- Kani harnesses on parsing/dispatch/arithmetic functions
- Panic-freedom proofs replace lint heuristics on critical paths
- `PROOF_KANI_*` rule families

### v2.0 — Flux refinement types

- `cargo flux` lane in `full` scope
- Flux annotations on domain newtypes (`WorkspacePath`, `TextRange`, `Digest`)
- Type-level invariant proofs
- `PROOF_FLUX_*` rule families

### v2.5 — Verus functional correctness

- `cargo verus` lane in new `verify` scope tier
- Verus specs (`spec fn`, `proof fn`) on pure domain logic
- Functional correctness proofs
- `PROOF_VERUS_*` rule families
- Receipt records which functions are formally verified

### Post-v2.5 — Team scale

- cargo-vet (third-party audit story)
- cargo-geiger (unsafe-in-deps measurement)
- cargo-machete (unused dep detection, with baseline)
- cargo-mutants (mutation testing, with baseline)
- cargo-audit (defense-in-depth with cargo-deny)
- Shuttle concurrency verification (when internal parallelization lands)
- Antithesis deterministic simulation (when stateful components land)
- Anti-circular meta-policy enforcement
- Persistent Receipt ledger (`xtask-ledger` SQLite crate)
- Multiple policy profiles (if demand exists)
- Init wizard (if `cargo generate` template proves insufficient)

---

## 15. Component Map

### Crates (first-party)

| Crate | Responsibility | v1 status |
|---|---|---|
| `xtask-bin` | CLI entrypoint, subcommand dispatch, lane runners | Rewrite for Moon-native model |
| `xtask-core` | Domain types: Report, Finding, Lane, Receipt, etc. | Exists (~527 LOC), minor updates |
| `xtask-policy` | strict-ai policy loading, validation, digest | New implementation |
| `xtask-lanes` | Lane runner implementations (fmt, check, clippy, etc.) | Partially exists, needs Moon-native refactor |
| `xtask-dylint` | dylint library (loaded by clippy driver) | New |
| `xtask-output` | Report JSON schema, doctor diagnostics, explain catalog | New |
| `xtask-aggregate` | Reads lane outputs, assembles Report, computes Receipt | New |

### Template repository

| Path | Purpose |
|---|---|
| `xtask-template/` | `cargo generate` template for workspace adoption |

---

## 16. References

- [`VISION.md`](./VISION.md) — the grand ambition
- [`architecture-spec.md`](./architecture-spec.md) — v6.0 historical spec (superseded)
- Moon v2 skill — `.agents/skills/moon-v2/SKILL.md`
- Holzman Rust skill — `.agents/skills/holzman-rust/SKILL.md`
- Functional Rust skill — `.agents/skills/functional-rust/SKILL.md`
- ast-grep documentation — https://ast-grep.github.io/
- dylint documentation — https://github.com/trailofbits/dylint
