# Titania-Check v1 Specification

> **Status: BUILDABLE CONTRACT** — Moon 2026
> **Moon is non-negotiable:** MOON CI/CD is the absolute foundation for all of this work.
> Every v1 lane is specified as a typed Rust check that Moon orchestrates.
> Companion: [`VISION.md`](./VISION.md) (the grand ambition)
>
> This document is the implementation contract. Every type is defined here.
> If a type, file format, lane, rule, or public behavior is referenced, this
> document must define it directly.

---

## 1. Product Statement

Titania-check is the typed Rust quality layer for Moon CI/CD. Moon is the
absolute foundation: Titania-check exists to give Moon CI/CD typed, auditable
Rust quality lanes instead of bash-in-YAML log soup. It enforces the
`strict-ai` coding standard via typed lanes, structured findings, and a
reproducible `QualityReceipt`. Distribution is a single binary plus a
co-located dylint library, orchestrated by Moon's task graph.

Titania is Rust tooling only. It judges Rust/Cargo workspaces and Rust-focused
Moon pipelines; it is not a polyglot QA framework. That constraint is a product
feature: Rust gives AI-assisted development a hard compiler, strict ownership
model, rich lint ecosystem, typed errors, newtype discipline, and a path to
deeper verification through Kani, Flux, Verus, Miri, Loom, fuzzing, and related
tools.

**Binary name:** `titania-check`
**Config directory:** `.titania/`
**Crate prefix:** `titania-*`
**Env var prefix:** `TITANIA_*`

**What v1 proves:** the code conforms to strict structural and style rules,
compiles cleanly, passes tests, and has a clean supply chain.

**What v1 does NOT prove:** panic-freedom (v1.5), type invariants (v2.0),
functional correctness (v2.5). These are the verification batches — see
[`VISION.md`](./VISION.md) §4 and §14 below.

---

## 2. Scope

### In scope for v1

- Moon CI/CD as orchestration substrate (required dependency)
- Rust/Cargo workspaces as the only judged source domain
- Single binary `titania-check` (CLI + lane runners + aggregator + doctor + explain)
- Co-located dylint dynamic library for type-aware lint scans
- ast-grep embedded as Rust dependency for structural rules
- Built-in cargo lanes: `fmt`, `compile`, `clippy`, `test`
- `cargo-deny` for supply chain (advisories, licenses, bans, sources, dupes)
- `panic-scan` lane (rg-based, parser-prefiltered)
- `policy-scan` lane (TOML/env bypass detection)
- `strict-ai` policy as the only profile
- 3 scope tiers: `edit`, `prepush`, `release`
- `QualityReceipt` with 4 digests (source, lock, policy, toolchain)
- `cargo generate titania/template` for workspace adoption
- Cross-platform (Unix + Windows)

### Out of scope for v1 (deferred)

See §14 for the full deferred roadmap. Summary:

- Formal verification: Kani, Flux, Verus
- Mutation testing: cargo-mutants (v1.5)
- Supply chain depth: cargo-vet, cargo-geiger, cargo-machete, cargo-audit
- Concurrency testing: Shuttle, Loom
- Allocation-scan (requires "critical module" definition — deferred)
- Anti-circular meta-policy enforcement
- Init wizard (use `cargo generate` template)
- Multiple policy profiles (`strict-critical-rust` is future direction, not specified)
- Non-Rust language policy engines or polyglot CI quality gates
- Persistent Receipt ledger

---

## 3. Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│  Moon CI/CD                                               │
│  (.moon/tasks/all.yml — lane DAG, composites)            │
│                                                           │
│  ┌───────────────┐  ┌───────────────┐  ┌──────────────┐  │
│  │ titania-check  │  │ titania-check  │  │ titania-check│  │
│  │ run-lane fmt   │  │ run-lane clippy│  │ aggregate    │  │
│  └───────┬───────┘  └───────┬───────┘  └──────┬───────┘  │
│          │                  │                  │          │
│          ▼                  ▼                  ▼          │
│  .titania/out/<scope>/<lane>.json  (typed findings)      │
│          │                  │                  │          │
│          └──────────────────┼──────────────────┘          │
│                            ▼                             │
│                     Report + Receipt                     │
└──────────────────────────────────────────────────────────┘

titania-check binary:
  ├── CLI dispatcher (check, run-lane, aggregate, doctor, explain)
  ├── Lane runners (shell out to cargo/clippy/rg/cargo-deny/ast-grep/dylint)
  ├── ast-grep rules (embedded via include_str!)
  ├── Policy loader (strict-ai defaults + file overrides)
  ├── Digest computation (source, lock, policy, toolchain)
  ├── Report/Receipt serialization (versioned JSON)
  ├── Finding normalizer (clippy/deny output → typed Findings)
  └── Doctor (tool presence + version checks)

titania-dylint library (separate .so/.dylib/.dll, co-located):
  ├── BYPASS_PUB_ALLOW lint
  ├── BYPASS_REQUIRED_LINT_WEAKENING lint
  ├── BYPASS_ATTR_CONTEXT lint
  ├── BYPASS_INTERNAL_UNSTABLE lint
  └── BYPASS_INTERNAL_UNSAFE lint

Crate DAG (dependency graph):
  titania-check (bin)
    ├── titania-aggregate
    │     └── titania-core
    ├── titania-lanes
    │     ├── titania-core
    │     └── titania-policy
    ├── titania-output
    │     └── titania-core
    ├── titania-policy
    │     └── titania-core
    └── titania-core (no internal deps)
```

---

## 4. The Lane DAG

### Lane enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lane {
    Fmt,
    Compile,
    Clippy,
    AstGrep,
    Dylint,
    PanicScan,
    PolicyScan,
    Test,
    Deny,
    Build,
}
```

### Lanes

| Lane | Tool | Scope | Depends on compile? | What it rejects |
|---|---|---|---|---|
| `Fmt` | `cargo fmt --check` | edit | No | formatting drift |
| `Compile` | `cargo check --workspace --frozen` | edit | — | compile errors, denied rustc lints |
| `Clippy` | `cargo clippy --workspace --lib --bins --frozen` with `-F` | edit | Yes | denied clippy lints |
| `AstGrep` | embedded ast-grep-core | edit | No | structural rule violations, bypass attribute presence, architecture import violations |
| `Dylint` | `cargo dylint titania` (loads libtitania_dylint) | edit | No | type-aware bypass violations |
| `PanicScan` | `rg` with parser prefilter | edit | No | production `assert!`/`assert_eq!`/`assert_ne!`/`unreachable!` |
| `PolicyScan` | titania-check native (TOML + env scanner) | edit | No | Cargo.toml `[lints]` weakening, `.cargo/config.toml` overrides, env var violations |
| `Test` | `cargo test --workspace --frozen -- --test-threads=1` | prepush | Yes | failing tests |
| `Deny` | `cargo deny check` | prepush | No | advisories, licenses, bans, sources, dupes |
| `Build` | `cargo build --workspace --release --frozen` | release | Yes | release build failure |

### Scopes (composite gates)

```
edit     = Fmt + Compile + Clippy + AstGrep + Dylint + PanicScan + PolicyScan
prepush  = edit + Test + Deny
release  = prepush + Build
```

### GateScope enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GateScope {
    Edit,
    Prepush,
    Release,
    // Future (v1.5+): Full, Deep — not constructible in v1.
    // #[non_exhaustive] ensures forward compatibility: external
    // consumers cannot exhaustively match without a default arm.
}
```

**Forward compatibility:** `#[non_exhaustive]` is applied so that v1.5 can add
`Full` and v2.5 can add `Deep` without breaking downstream match expressions.
v1 constructors only produce `Edit`, `Prepush`, `Release`.

### Skip rules

- Lanes marked "Depends on compile?" skip with `SkipReason::PriorCompilationFailure`
  if `Compile` fails.
- All other lanes run regardless.
- `AstGrep`, `Dylint`, `PanicScan`, `PolicyScan` run on source/config files
  regardless of compilation status.

### Execution model

Each lane is a Moon task. Moon handles:
- Parallel execution of independent lanes
- Per-lane `CARGO_TARGET_DIR` isolation (breaks cargo lock contention)
- 3-layer caching (sccache + bazel-remote + Cargo)
- Affected-file detection (`moon ci` skips lanes whose inputs are unchanged)
- CI integration via `moonrepo/setup-toolchain@v0` + `moonrepo/run-report-action@v1`

**sccache requirement:** sccache is **strongly recommended** but not hard-required.
Without sccache, per-lane `CARGO_TARGET_DIR` means each cargo-invoking lane
recompiles from scratch. With sccache, the first lane populates the cache and
subsequent lanes hit it. `titania-check doctor` reports sccache availability.

---

## 5. Enforcement Layers

The strict-ai standard is enforced by multiple layers, each catching what the
previous cannot:

| Layer | Tool | Catches | Cannot catch |
|---|---|---|---|
| 0 | `cargo fmt --check` | formatting drift | — |
| 1 | `cargo check` + rustc lints | compile errors, `unsafe_code=forbid` | — |
| 2 | `cargo clippy` with `-F` critical lints | unwrap/expect/panic/todo/indexing/arithmetic | third-party panics, macro-expanded panics |
| 3 | `ast-grep` embedded rules | structural patterns, bypass attributes, architecture imports | type-aware context |
| 4 | `dylint` library | type-aware bypass detection (scope, required-lint weakening) | — |
| 5 | `panic-scan` (rg) | production `assert!`/`unreachable!` macros | block comments, string literals (heuristic) |
| 6 | `policy-scan` (native) | Cargo.toml `[lints]` weakening, `.cargo/config.toml` overrides, env violations | — |
| 7 | `cargo test` | behavior failures | missing tests |
| 8 | `cargo deny check` | supply chain violations | unknown vulnerabilities |
| 9 | `cargo build --release` | release-mode build failures | runtime behavior |

### Finding normalization

External tools (clippy, cargo-deny) produce their own output formats. Titania
normalizes their output into typed `Finding` values:

| Source tool | Rule ID prefix | Example rule ID | How findings are produced |
|---|---|---|---|
| clippy | `CLIPPY_*` | `CLIPPY_UNWRAP_USED`, `CLIPPY_INDEXING_SLICING` | Parse `--message-format=json`, map lint name to uppercase rule ID |
| cargo-deny | `DENY_*` | `DENY_ADVISORY`, `DENY_LICENSE`, `DENY_BANNED_CRATE` | Parse cargo-deny JSON output, map check type to rule ID |
| ast-grep | (as defined in §6) | `FUNC_LOOPS_FOR`, `BYPASS_ALLOW_ATTR` | Parse ast-grep JSON output directly |
| dylint | (as defined in §7) | `BYPASS_PUB_ALLOW` | Parse dylint output via clippy JSON format |
| panic-scan | `HOLZMAN_PANIC_*` | `HOLZMAN_PANIC_ASSERT` | Parse rg output, construct Finding per match |
| policy-scan | `BYPASS_*` | `BYPASS_CARGO_LINTS_WEAKENING` | Native scan, construct Finding per violation |

---

## 6. ast-grep Rule Catalog

Rules embedded in the binary via `include_str!("rules/*.yml")`. Each rule
produces a `Finding` with `FindingEffect::Reject` unless noted.

### Structural rules (functional-style enforcement)

| Rule ID | Pattern | Effect | RepairHint |
|---|---|---|---|
| `FUNC_LOOPS_FOR` | `for $LOOP in $ITER { ... }` in production source | Reject | `UseIteratorPipeline` |
| `FUNC_LOOPS_WHILE` | `while $COND { ... }` in production source | Reject | `UseIteratorPipeline` |
| `FUNC_LOOPS_LOOP` | `loop { ... }` in production source | Reject | `UseIteratorPipeline` |
| `FUNC_NESTING_DEPTH` | Nesting depth > 2 (AST depth measurement) | Reject | `FlattenNesting` |
| `FUNC_RESULT_STRING` | `Result<$T, String>` type annotation | Reject | `RequiresHumanReview` |
| `FUNC_RECURSION_DIRECT` | Function calls itself by name | Reject | `RequiresHumanReview` |
| `FUNC_PRINT_STDOUT` | `print!` or `println!` in production source | Reject | `RequiresHumanReview` |
| `FUNC_PRINT_STDERR` | `eprint!` or `eprintln!` in production source | Reject | `RequiresHumanReview` |
| `FUNC_UNWRAP_OR` | `.unwrap_or($X)` / `.unwrap_or_default()` / `.unwrap_or_else($F)` | Reject | `RequiresHumanReview` |
| `FUNC_WILDCARD_IMPORT` | `use $PATH::*;` | Informational | — |

All structural rules exclude `tests/`, `benches/`, `examples/`, `build.rs`.

### Bypass detection rules (attribute presence)

| Rule ID | Pattern | Effect |
|---|---|---|
| `BYPASS_ALLOW_ATTR` | `#[allow($LINT)]` on any item | Reject |
| `BYPASS_EXPECT_ATTR` | `#[expect($LINT)]` on any item | Reject |
| `BYPASS_CFG_ATTR_ALLOW` | `cfg_attr($COND, allow($LINT))` | Reject |
| `BYPASS_CRATE_ALLOW` | `#![allow($LINT)]` at crate level | Reject |
| `BYPASS_CRATE_EXPECT` | `#![expect($LINT)]` at crate level | Reject |
| `BYPASS_INLINE_SUPPRESSION` | `// ast-grep-ignore` or `// sg-ignore` | Reject |

### Architecture import rules

| Rule ID | Pattern | Effect | Notes |
|---|---|---|---|
| `ARCHITECTURE_IMPORT_CORE_INFRA` | `use tokio::*` / `use axum::*` / `use sqlx::*` / `use reqwest::*` in `core/` or `domain/` | Reject | Core must not import infrastructure |
| `ARCHITECTURE_IMPORT_CORE_FS` | `use std::fs::*` / `use std::env::*` / `use std::net::*` in `core/` or `domain/` | Reject | Core must not do direct I/O |
| `ARCHITECTURE_IMPORT_CORE_TIME` | `use std::time::SystemTime` / `use std::time::Instant` in `core/` or `domain/` | Reject | Core must not read wall clock |
| `ARCHITECTURE_IMPORT_CORE_RANDOM` | `use rand::thread_rng` in `core/` or `domain/` | Reject | Core must not use entropy source |

Architecture rules check file path prefix: if the file is under `core/`,
`domain/`, or a crate named `*-core` / `*-domain`, the rules apply.

---

## 7. dylint Rule Catalog

The dylint library (`libtitania_dylint`) contains type-aware lints that ast-grep
cannot express. These run via `cargo dylint titania` which loads the library
into the clippy driver process.

| Rule ID | What it detects | Why dylint (not ast-grep) |
|---|---|---|
| `BYPASS_PUB_ALLOW` | `#[allow(...)]` on `pub` items specifically (not `pub(crate)`) | Needs scope/visibility resolution |
| `BYPASS_REQUIRED_LINT_WEAKENING` | `#![allow(...)]` of lints that are in the required set | Needs knowledge of which lints are required |
| `BYPASS_ATTR_CONTEXT` | `#[allow]` in proc-macro-expanded code vs hand-written source | Needs macro expansion context |
| `BYPASS_INTERNAL_UNSTABLE` | `#[allow_internal_unstable]` | Rare but critical escape hatch |
| `BYPASS_INTERNAL_UNSAFE` | `#[allow_internal_unsafe]` | Rare but critical escape hatch |

### dylint loading specification

dylint libraries are loaded by the clippy driver. Titania configures this via
`Cargo.toml`:

```toml
# In the checked-in workspace Cargo.toml:
[workspace.metadata.dylint]
libraries = [
    { path = "crates/titania-dylint" },
]
```

**Path resolution algorithm:**
1. If `TITANIA_DYLINT_LIB` env var is set, use that path directly.
2. If `[workspace.metadata.dylint]` is configured, `cargo dylint titania` uses it.
3. If neither, titania-check searches for `libtitania_dylint` in the same
   directory as the `titania-check` binary.

**Failure mode:** if the library is missing or has an ABI mismatch (wrong rustc
version), the dylint lane fails with `LaneFailure::InfraFailure` containing the
error message. The Report records this as a lane failure, not a silent skip.

**Cross-platform distribution:** `cargo-binstall` ships per-target packages:
- Linux: `libtitania_dylint.so`
- macOS: `libtitania_dylint.dylib`
- Windows: `titania_dylint.dll`

**ABI compatibility:** the dylint library must be compiled against the same
rustc internals version as the active toolchain. `titania-check doctor` verifies
compatibility and reports mismatches.

---

## 8. Policy-Scan Lane (TOML + env bypass detection)

These rules run as native Rust code in `titania-check` (not ast-grep or dylint)
because they scan TOML files and environment variables, not Rust source.

### Non-Rust bypass detection rules

| Rule ID | Target | Effect | Notes |
|---|---|---|---|
| `BYPASS_CARGO_LINTS_WEAKENING` | `Cargo.toml` `[lints]` sections | Reject | Detects lowering of required lints |
| `BYPASS_CARGO_CONFIG_WRAPPER` | `.cargo/config.toml` `rustc-wrapper` | Reject | Must be absent or `sccache` |
| `BYPASS_CARGO_CONFIG_FLAGS` | `.cargo/config.toml` `rustflags` | Reject | Must match policy-allowed flags |
| `BYPASS_ENV_RUSTFLAGS` | `$RUSTFLAGS` / `$CARGO_ENCODED_RUSTFLAGS` | Reject | Unexpected values at runtime |
| `BYPASS_ENV_RUSTC_WRAPPER` | `$RUSTC_WRAPPER` / `$RUSTC_WORKSPACE_WRAPPER` | Reject | Must be absent or `sccache` |
| `BYPASS_ENV_RUSTC_BOOTSTRAP` | `$RUSTC_BOOTSTRAP` set | Reject | Always a violation |

---

## 9. The strict-ai Policy

The policy is the single source of truth for what "strict Rust" means. It is
embedded as defaults in the titania-check binary and overridable only via
checked-in policy files.

### 9.1 Cargo.toml `[workspace.lints]`

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

unwrap_or_default = "deny"
exit = "deny"
default_numeric_fallback = "deny"
missing_errors_doc = "deny"
```

### 9.2 Clippy critical lints via `-F`

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

### 9.3 clippy.toml thresholds

```toml
too-many-lines-threshold = 40
too-many-arguments-threshold = 5
max-fn-params-bools = 1
```

### 9.4 deny.toml

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

### 9.5 Hermeticity rules

- `--frozen` everywhere (not `--locked`). `--frozen` = `--locked` + `--offline`.
- `rust-toolchain.toml` pinned (Moon-managed via `syncToolchainConfig`).
- `CARGO_HOME` set to a controlled, read-only directory.
- `RUSTUP_HOME` same.
- Reject parent-directory cargo configs.
- `.cargo/config` (extensionless) AND `.cargo/config.toml` both checked.
- `RUSTC_BOOTSTRAP` = violation.
- `RUSTC_WRAPPER` must be sccache or absent.

### 9.6 Generated code

`include!(concat!(env!("OUT_DIR"), ...))` is banned. Generated Rust must be
checked into source and gated like normal code.

### 9.7 policy.toml schema

Location: `.titania/profiles/strict-ai/policy.toml`

```toml
# Policy file for the strict-ai profile.
# Overrides binary defaults. Changes require CODEOWNER approval.

[lints]
# Override clippy lint levels (rare; defaults are in the binary)
# example: clippy::needless_return = "allow"

[thresholds]
# Override clippy.toml thresholds (rare)
# too_many_lines = 40

[architecture]
# Define which directories are "core" for architecture import rules
core_dirs = ["src/core", "src/domain", "crates/*-core/src"]
infra_crates = ["tokio", "axum", "sqlx", "reqwest"]

[supply_chain]
# Override deny.toml settings (rare)
```

### 9.8 exceptions.toml schema

Location: `.titania/profiles/strict-ai/exceptions.toml`

No owner, no reason, no expiry, no exception.

```toml
[[exceptions]]
rule_id = "FUNC_LOOPS_FOR"
path = "src/control/loop.rs"
owner = "flight-control"
reason = "Fixed 8-iteration control loop over sensor lanes"
expires_on = "2026-12-31"
review = "SAFETY-1234"
```

Every suppression — `#[allow]`, cargo-deny exception, env override — must be
in this file. The `PolicyScan` and `AstGrep` lanes consult this file before
emitting findings. Expired exceptions (`expires_on < today`) are themselves
rejected with `POLICY_EXCEPTION_EXPIRED`.

### 9.9 policy_digest algorithm

```
policy_digest = blake3(
    canonical_serialize(
        binary_defaults,
        policy_toml_contents,
        exceptions_toml_contents,
        deny_toml_contents,
        clippy_toml_contents
    )
)
```

Canonical serialization: TOML files normalized (sorted keys, no comments),
concatenated with length prefixes. blake3 hash, hex-encoded, 64 chars.

---

## 10. Domain Model

All types are defined here. No external document is needed.

### Core primitives

```rust
/// SHA-256 content digest, hex-encoded (64 ASCII chars).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Digest(pub String);  // invariant: len == 64, hex-only

impl Digest {
    pub fn from_bytes(data: &[u8]) -> Self {
        Self(blake3::hash(data).to_hex().to_string())
    }
}

/// Rule identifier. Uppercase, namespaced by prefix.
/// Examples: FUNC_LOOPS_FOR, CLIPPY_UNWRAP_USED, BYPASS_ALLOW_ATTR.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuleId(pub String);  // invariant: uppercase, ASCII, contains '_'

/// Normalized UTF-8 workspace-relative path.
/// Invariants: no backslashes, no `..`, no leading `/`, non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspacePath(pub String);

/// Byte range in a source file, for deterministic patching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start_byte: u32,
    pub end_byte: u32,  // invariant: end_byte >= start_byte
}
```

### Report

```rust
pub enum Report {
    Pass {
        receipt: QualityReceipt,
        per_lane: Box<[LaneOutcome]>,
    },
    Reject {
        // INVARIANT: at least one of code_findings or gate_failures is non-empty.
        // A Reject with both empty is a bug — should be Pass.
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

impl Report {
    pub fn reject_kind(&self) -> Option<RejectKind> {
        match self {
            Report::Reject { code_findings, gate_failures, .. } => {
                match (code_findings.is_empty(), gate_failures.is_empty()) {
                    (false, true) => Some(RejectKind::CodeOnly),
                    (true, false) => Some(RejectKind::GateOnly),
                    (false, false) => Some(RejectKind::Mixed),
                    (true, true) => None, // invariant violation
                }
            }
            _ => None,
        }
    }
}

pub enum RejectKind { CodeOnly, GateOnly, Mixed }
```

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
    // Future (when Moon integration lands): CacheHit { input_digest: Digest }
}

pub struct LaneEvidence {
    pub command: CommandEvidence,
    pub tool_version: String,
    pub exit_status: ProcessTermination,
    pub parsed_result_digest: Digest,
}

pub struct CommandEvidence {
    pub executable: String,
    pub argv: Box<[String]>,
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
    Dependency { crate_name: String, version: String },
    Manifest { file: WorkspacePath },
    Workspace,
    Tool { name: String, version: String },
}
```

Line/column: 1-based lines, 0-based columns (Unicode scalar values).

### RepairHint

```rust
pub enum RepairHint {
    Patch { file: String, range: TextRange, replacement: String },
    UseIteratorPipeline { suggestion: String },
    FlattenNesting { suggestion: String },
    UseCheckedArithmetic { op: String },
    RemoveAllowAttribute { attr: String },
    ReplaceDependency { from: String, to: String },
    RequiresHumanReview { note: String },
}
```

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
    Signaled { signal: i32 },  // Unix: signal number (9 = SIGKILL, covers OOM kill + user kill)
    TimedOut,
    MemoryLimitExceeded,
    SpawnFailed,
}
```

Windows: processes are killed via TerminateProcess which appears as
`Exited { code: 1 }`. There is no signal concept on Windows.

### QualityReceipt

```rust
pub struct QualityReceipt {
    pub schema_version: u16,  // v1 value: 1. Incremented on breaking schema changes.
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
```

**schema_version policy:** v1 uses `1`. The version increments when the JSON
shape changes in a way that breaks consumers. Additive changes (new optional
fields) do NOT increment. Removing fields or changing types DOES increment.

### Diagnostics

```rust
pub struct PolicyDiagnostic {
    pub message: String,
    pub file: Option<WorkspacePath>,
    pub severity: DiagnosticSeverity,
}

pub struct InputDiagnostic {
    pub message: String,
    pub tool: Option<String>,
    pub severity: DiagnosticSeverity,
}

pub enum DiagnosticSeverity {
    Error,
    Warning,
}
```

---

## 11. JSON Schemas

### 11.1 Report JSON (schema_version = 1)

Report Pass example:

```json
{
  "variant": "Pass",
  "receipt": {
    "schema_version": 1,
    "scope": "Edit",
    "source_digest": "a1b2c3d4...",
    "cargo_lock_digest": "e5f6g7h8...",
    "policy_digest": "i9j0k1l2...",
    "toolchain_digest": "m3n4o5p6...",
    "lanes": [
      { "lane": "Fmt", "evidence_digest": "q7r8s9t0...", "clean": true },
      { "lane": "Compile", "evidence_digest": "u1v2w3x4...", "clean": true }
    ]
  },
  "per_lane": [
    { "Clean": { "evidence": { "command": { "executable": "cargo", "argv": ["cargo", "fmt", "--check"] }, "tool_version": "rustfmt 1.84.0", "exit_status": { "Exited": { "code": 0 } }, "parsed_result_digest": "y5z6a7b8..." } } },
    { "Clean": { "evidence": { "command": { "executable": "cargo", "argv": ["cargo", "check", "--workspace", "--frozen"] }, "tool_version": "cargo 1.84.0", "exit_status": { "Exited": { "code": 0 } }, "parsed_result_digest": "c9d0e1f2..." } } }
  ]
}
```

Report Reject example:

```json
{
  "variant": "Reject",
  "code_findings": [
    {
      "lane": "AstGrep",
      "rule_id": "FUNC_LOOPS_FOR",
      "location": { "Span": { "file": "src/parser.rs", "line_start": 42, "col_start": 5, "line_end": 42, "col_end": 30 } },
      "message": "Imperative for loop in production source",
      "repair": { "UseIteratorPipeline": { "suggestion": "items.iter().map(|item| ...)" } },
      "effect": "Reject"
    },
    {
      "lane": "Clippy",
      "rule_id": "CLIPPY_UNWRAP_USED",
      "location": { "Span": { "file": "src/parser.rs", "line_start": 43, "col_start": 15, "line_end": 43, "col_end": 25 } },
      "message": "used `unwrap()` on an Option value",
      "repair": { "RequiresHumanReview": { "note": "Use ? operator, match, or if-let instead of unwrap" } },
      "effect": "Reject"
    }
  ],
  "gate_failures": [],
  "per_lane": [
    { "Findings": [ "..." ] },
    { "Failed": { "ToolFailure": { "tool": "cargo-test", "termination": { "Exited": { "code": 1 } } } } }
  ]
}
```

### 11.2 Lane output JSON (`.titania/out/<scope>/<lane>.json`)

Each lane writes a `LaneOutcome` as JSON to this path. The aggregator reads
all files matching `.titania/out/<scope>/*.json`.

```json
{
  "lane": "AstGrep",
  "outcome": {
    "Findings": [
      {
        "lane": "AstGrep",
        "rule_id": "FUNC_LOOPS_FOR",
        "location": { "Span": { "file": "src/parser.rs", "line_start": 42, "col_start": 5, "line_end": 42, "col_end": 30 } },
        "message": "Imperative for loop in production source",
        "repair": { "UseIteratorPipeline": { "suggestion": "items.iter().map(|item| ...)" } },
        "effect": "Reject"
      }
    ]
  }
}
```

**Aggregate behavior on missing lane output:** if a lane's output file is
missing when aggregate runs, the lane is recorded as
`LaneOutcome::Failed(LaneFailure::InfraFailure { tool, reason: "output file missing" })`.
This is a gate failure, not a silent skip.

**Atomic writes:** lane runners write to a temp file, then rename to the final
path. This prevents partial reads by the aggregator.

---

## 12. CLI Surface

```
titania-check [--scope edit|prepush|release] [--emit json] [--out <path>]
    Run scoped quality lanes via Moon. Emit report JSON + quality receipt on pass.
    This is the primary command — "titania-check" with no subcommand defaults to check.

titania-check run-lane <lane-name>
    Run a single lane and write findings to .titania/out/<scope>/<lane>.json.
    Invoked by Moon tasks.

titania-check aggregate --scope <scope> [--emit json] [--out <path>]
    Read all lane outputs for the scope, assemble Report, emit JSON.
    Invoked by Moon composite gate tasks.

titania-check doctor [--scope <scope>] [--emit json]
    Report required tools, installed status, and versions for the scope.

titania-check explain <rule-id>
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

### doctor output format (human)

```
titania-check doctor — scope: edit

Tool           Required   Installed   Version        Path
cargo          yes        yes         1.84.0         /home/user/.cargo/bin/cargo
rustfmt        yes        yes         1.84.0         /home/user/.cargo/bin/rustfmt
clippy         yes        yes         0.1.84         /home/user/.cargo/bin/clippy-driver
rg             yes        yes         14.1.0         /usr/bin/rg
ast-grep       embedded   —           —              —
dylint         yes        yes         0.1.0          /home/user/.cargo/bin/cargo-dylint
cargo-deny     no (edit)  yes         0.18.0         /home/user/.cargo/bin/cargo-deny
sccache        optional   yes         0.8.0          /home/user/.cargo/bin/sccache

Status: OK
```

### doctor output format (--emit json)

```json
{
  "scope": "edit",
  "tools": [
    { "name": "cargo", "required": true, "installed": true, "version": "1.84.0", "path": "/home/user/.cargo/bin/cargo" },
    { "name": "sccache", "required": false, "installed": true, "version": "0.8.0", "path": "/home/user/.cargo/bin/sccache" }
  ],
  "missing_required": [],
  "status": "OK"
}
```

If any required tool is missing, exit code is 3 (InputError) and `status` is
`"MissingRequiredTools"`.

### explain output format

```
FUNC_LOOPS_FOR
  Rejects imperative `for` loops in production source.

  Pattern: for $LOOP in $ITER { ... }
  Effect: Reject
  Repair: UseIteratorPipeline — suggests iterator pipeline alternative

  Example violation:
    for item in items { process(item); }

  Example repair:
    items.iter().for_each(|item| process(item));
```

On unknown rule ID, exit code is 3 (InputError) with message
`"unknown rule ID: <input>"`.

---

## 13. Moon Integration

### .moon/toolchains.yml

```yaml
rust:
  version: '<pinned-stable>'
  bins:
    - 'cargo-deny@latest'
    - 'cargo-binstall@latest'
    - 'cargo-dylint@latest'
  syncToolchainConfig: true
```

### .moon/tasks/all.yml (titania-check-managed)

```yaml
# Lane runners — each invokes titania-check run-lane
titania-fmt:
  command: 'titania-check run-lane fmt'
  toolchains: [rust]
  options: { runInCI: true }
  inputs: ['@globs(sources)', 'rustfmt.toml', '.titania/**']

titania-compile:
  command: 'titania-check run-lane compile'
  env:
    CARGO_TARGET_DIR: '${workspace.root}/.titania/cache/compile'
  toolchains: [rust]
  options: { runInCI: true }
  inputs: ['@globs(sources)', 'Cargo.toml', 'Cargo.lock', '.cargo/**', 'rust-toolchain.toml']

titania-clippy:
  command: 'titania-check run-lane clippy'
  env:
    CARGO_TARGET_DIR: '${workspace.root}/.titania/cache/clippy'
  toolchains: [rust]
  deps: [':titania-compile']
  options: { runInCI: true }
  inputs: ['@globs(sources)', 'clippy.toml', 'Cargo.toml', '.titania/**']

titania-ast-grep:
  command: 'titania-check run-lane ast-grep'
  options: { runInCI: true }
  inputs: ['@globs(sources)']

titania-dylint:
  command: 'titania-check run-lane dylint'
  toolchains: [rust]
  options: { runInCI: true }
  inputs: ['@globs(sources)']

titania-panic-scan:
  command: 'titania-check run-lane panic-scan'
  options: { runInCI: true }
  inputs: ['@globs(sources)']

titania-policy-scan:
  command: 'titania-check run-lane policy-scan'
  options: { runInCI: true }
  inputs: ['Cargo.toml', '**/Cargo.toml', '.cargo/**', '.titania/**']

titania-test:
  command: 'titania-check run-lane test'
  env:
    CARGO_TARGET_DIR: '${workspace.root}/.titania/cache/test'
  toolchains: [rust]
  deps: [':titania-compile']
  options: { runInCI: true }
  inputs: ['@globs(sources)', '@globs(tests)']

titania-deny:
  command: 'titania-check run-lane deny'
  options: { runInCI: true }
  inputs: ['Cargo.lock', 'deny.toml']

titania-build:
  command: 'titania-check run-lane build'
  env:
    CARGO_TARGET_DIR: '${workspace.root}/.titania/cache/release'
  toolchains: [rust]
  deps: [':titania-compile']
  options: { runInCI: true }
  inputs: ['@globs(sources)']
  outputs: ['target/release/titania-check']

# Composite gates
gate-edit:
  command: 'titania-check aggregate --scope edit'
  deps: [':titania-fmt', ':titania-compile', ':titania-clippy', ':titania-ast-grep', ':titania-dylint', ':titania-panic-scan', ':titania-policy-scan']
  options: { runInCI: true }

gate-prepush:
  command: 'titania-check aggregate --scope prepush'
  deps: [':gate-edit', ':titania-test', ':titania-deny']
  options: { runInCI: true }

gate-release:
  command: 'titania-check aggregate --scope release'
  deps: [':gate-prepush', ':titania-build']
  options: { runInCI: true }
```

### Cache layers

| Layer | Tool | Scope |
|---|---|---|
| 1 | sccache (rustc artifacts) | Global, all Rust projects |
| 2 | bazel-remote (Moon task outputs, AC+CAS) | Per-machine, all Moon projects |
| 3 | Cargo incremental (per-lane `CARGO_TARGET_DIR`) | Per-lane, per-repo |

Per-lane `CARGO_TARGET_DIR` breaks cargo lock contention. sccache recovers
duplicate-compile cost across lanes. Without sccache, expect ~3-4x compile
wall-time vs shared target dir.

---

## 14. Distribution

### Binary installation

```bash
# Primary: cargo-binstall (pre-built binary + dylint library)
cargo binstall titania-check

# Alternative: cargo install (builds from source)
cargo install titania-check
```

Both install the `titania-check` binary AND the co-located `libtitania_dylint`
library (`.so`/`.dylib`/`.dll`).

### Workspace adoption

```bash
cargo generate titania/template
```

The template provides:
- `.moon/workspace.yml`
- `.moon/toolchains.yml`
- `.moon/tasks/all.yml` (from §13)
- `.titania/profiles/strict-ai/policy.toml`
- `.titania/profiles/strict-ai/exceptions.toml`
- `.cargo/config.toml` (sccache wrapper hook)
- `clippy.toml`
- `rustfmt.toml`
- `deny.toml`
- `rust-toolchain.toml` (Moon-managed)
- `Cargo.toml` with `[workspace.lints]` from §9.1

### Prerequisites

- Moon v2+ (orchestration)
- Rust toolchain (Moon-managed)
- `rg` (for panic-scan)
- `cargo-dylint` (for dylint lane)
- sccache (strongly recommended)
- bazel-remote (optional — team cache)

`titania-check doctor --scope <scope>` verifies presence.

---

## 15. Definition of Done

Titania-check v1 is done when:

1. `titania-check --scope edit` runs fmt, compile, clippy, ast-grep, dylint,
   panic-scan, and policy-scan lanes via Moon.
2. `titania-check --scope prepush` adds test and cargo-deny lanes.
3. `titania-check --scope release` adds release build.
4. Each lane writes typed findings to `.titania/out/<scope>/<lane>.json`
   with schema versioning and atomic writes.
5. `titania-check aggregate --scope <scope>` reads lane outputs and produces
   a typed `Report` JSON per §11.1.
6. The report schema is stable (schema_version=1), versioned, machine-readable,
   and separates code findings from gate/tool failures.
7. The `strict-ai` policy forbids first-party unsafe, unwrap/expect, panic
   macros, unchecked indexing, unchecked arithmetic, unapproved lint
   suppressions, imperative loops, excessive nesting, architecture import
   violations, and core `Result<T, String>`.
8. All policy exceptions live in `.titania/profiles/strict-ai/exceptions.toml`
   with owner, reason, expiry, and review.
9. `titania-check doctor --scope <scope>` reports tools and versions per §12.
10. `cargo generate titania/template` produces a working workspace that passes
    `titania-check --scope prepush` out of the box.
11. Titania-check's own repository passes `titania-check --scope release`.
12. The dylint library loads correctly via `[workspace.metadata.dylint]`.
13. Clippy findings are normalized to typed Findings with `CLIPPY_*` rule IDs.
14. Cargo-deny findings are normalized to typed Findings with `DENY_*` rule IDs.

### Killer demo

An AI writes Rust with a `for` loop and `.unwrap()`:

```rust
for item in items {
    let value = item.unwrap();
}
```

`titania-check --scope edit` rejects with two typed findings:
- `FUNC_LOOPS_FOR` (from ast-grep) with `RepairHint::UseIteratorPipeline`
- `CLIPPY_UNWRAP_USED` (from clippy, normalized) with `RepairHint::RequiresHumanReview`

The AI repairs. The gate passes and emits a `QualityReceipt` with
schema_version=1 and source/policy/toolchain digests.

---

## 16. Deferred to v1.5+ (Roadmap)

These items are explicitly out of scope for v1. Listed to prevent scope creep.

### v1.5 — Kani panic-freedom + cargo-mutants

- `GateScope::Full` variant unlocked
- `cargo kani` lane — Kani harnesses on parsing/dispatch/arithmetic
- `cargo mutants` lane — mutation testing with checked-in baseline
- Panic-freedom proofs replace lint heuristics on critical paths
- `PROOF_KANI_*` and `MUTANT_SURVIVED` rule families

### v2.0 — Flux refinement types + architecture crate-graph enforcement

- `cargo flux` lane — Flux annotations on domain newtypes
- guppy-based crate graph enforcement (§6.1 of VISION)
- Type-level invariant proofs
- `PROOF_FLUX_*` rule family

### v2.5 — Verus functional correctness + `deep` scope

- `GateScope::Deep` variant unlocked
- `cargo verus` lane — Verus specs on pure domain logic
- Miri, sanitizers, cargo-fuzz
- `PROOF_VERUS_*` rule family

### v3.0 — Safety-critical profile + release evidence

- `strict-critical-rust` profile (future direction — not specified in v1)
- Ferrocene support
- cargo-auditable, cargo-cyclonedx, Syft, OSV-Scanner
- SBOM + audit trail

### Post-v3.0 — Team scale

- cargo-vet, cargo-geiger, cargo-machete, cargo-audit
- Shuttle/Loom concurrency verification
- Antithesis deterministic simulation
- Anti-circular meta-policy enforcement
- Persistent Receipt ledger (SQLite)
- Multiple policy profiles

---

## 17. Component Map

### Crates (first-party)

| Crate | Responsibility | Depends on |
|---|---|---|
| `titania-check` (bin) | CLI entrypoint, subcommand dispatch | all below |
| `titania-core` | Domain types: Report, Finding, Lane, Receipt, all primitives | (none) |
| `titania-policy` | strict-ai policy loading, validation, digest, exceptions | `titania-core` |
| `titania-lanes` | Lane runner implementations | `titania-core`, `titania-policy` |
| `titania-dylint` | dylint library (loaded by clippy driver) | (none — standalone cdylib) |
| `titania-output` | Report JSON serialization, doctor, explain | `titania-core` |
| `titania-aggregate` | Reads lane outputs, assembles Report, computes Receipt | `titania-core` |

**Note:** `titania-dylint` is a `cdylib` crate (compiles to `.so`/`.dylib`/`.dll`).
It does not depend on `titania-core` because it runs inside the clippy driver
process and communicates via dylint's own types.

### Template repository

| Path | Purpose |
|---|---|
| `titania/template` | `cargo generate` template for workspace adoption |

---

## 18. FAQ / Public Positioning

### What is Titania?

Titania is the highly opinionated Rust QA fairy for AI-assisted development.
It runs locally and in CI, shells out to proven Rust tools, and turns their
results into typed findings, repair hints, policy digests, and reproducible
evidence receipts.

The public promise is simple: AI can write Rust fast; Titania makes it prove it
did not hallucinate the basics.

### Is Titania an AI tool?

Yes, but not a chatbot.

Titania is AI infrastructure. It gives humans and AI coding agents the same
deterministic local feedback loop: run the Moon-powered QA gate, receive stable
machine-readable failures, repair against rule IDs, and prove the final state
with a receipt.

The goal is not to prompt the model harder. The goal is to make bad AI output
mechanically obvious before it leaves the laptop.

### Why Rust only?

See [`WHY_RUST_ONLY.md`](./WHY_RUST_ONLY.md) for the full product argument.

Because Titania is built around one opinion: if teams are going to push AI code
hard, Rust is the best place to extract useful speed without accepting the usual
quality collapse.

Titania is intentionally not polyglot. Polyglot QA sounds broad, but it usually
collapses into the lowest common denominator: lint some files, run some tests,
parse some logs, and hope the dynamic/runtime failures are caught later. Titania
chooses depth over breadth. It targets one language where the compiler, package
manager, linter ecosystem, type system, and verification tools can be composed
into a much sharper local gate.

Rust is uniquely useful for AI-assisted coding because the language turns many
AI mistakes into concrete, repairable failures:

- the compiler rejects ownership, borrowing, lifetime, trait, exhaustiveness,
  and type errors before code runs
- `Result` and `Option` make failure paths explicit instead of relying on
  unchecked exceptions, nulls, or ambient runtime behavior
- `Send`, `Sync`, lifetimes, and ownership rules expose concurrency and aliasing
  mistakes that agents frequently gloss over in looser languages
- enums, pattern matching, and exhaustive state modeling make workflow gaps
  visible to humans and machines
- newtypes, typestates, and domain-specific constructors let teams make illegal
  states unrepresentable instead of asking reviewers to remember every rule
- clippy gives agents a dense stream of idiomatic, machine-actionable feedback
- rustfmt removes formatting bikeshedding from both humans and agents
- cargo metadata gives a uniform workspace/package graph to inspect
- cargo-deny, cargo-audit, cargo-vet, cargo-geiger, cargo-machete, and related
  tools give supply-chain and dependency checks a coherent substrate
- cargo-nextest, Miri, Loom, sanitizers, fuzzing, and property tests give Rust a
  broad testing/analysis ladder beyond normal unit tests
- Kani, Flux, Verus, and related tools provide a credible path from lint gates
  toward actual proof obligations on the same language

That combination matters for AI. LLMs are good at producing plausible code;
they are much weaker at knowing whether the code's hidden assumptions are valid.
Rust gives Titania more surfaces where hidden assumptions become explicit:

- a panic becomes a rule ID, not a surprise runtime faceplant
- an unchecked index becomes a clippy finding, not a latent production bug
- a missing error variant becomes a domain-model review issue
- a sloppy stringly-typed ID becomes a newtype obligation
- a bypass attribute becomes policy evidence, not invisible reviewer debt
- an architectural import leak becomes a deterministic finding
- a dependency or license issue becomes a typed supply-chain failure

This is the shift-left thesis: AI supplies speed; Rust supplies friction in the
right places; Titania packages that friction into a local, deterministic QA
loop. The goal is not to make writing Rust effortless. The goal is to make AI
write better Rust by forcing it through a language and toolchain that refuse to
accept many common hallucinations.

Rust also has enough reach to justify focusing deeply. Teams can use it for
CLIs, backend services, web frontends through WASM/native UI frameworks,
embedded systems, data infrastructure, developer tools, low-level systems, and
performance-critical libraries. A Rust-only QA gate is therefore not a toy niche;
it can cover a large part of a serious software stack while preserving one
coherent quality model.

Titania is not saying every team must rewrite everything in Rust. It is saying
that when a team chooses Rust for AI-assisted development, Titania can be much
more rigorous than a generic language-agnostic CI wrapper because it can lean on
Rust-specific semantics, Cargo metadata, clippy diagnostics, Rust verification
tools, and type-driven design.

Titania may run inside many CI hosts, and it may coexist with non-Rust systems,
but the code it judges is Rust. That boundary keeps the product honest, sharp,
and mechanically enforceable.

### Why Moon CI/CD?

Because Titania is not trying to become a second-rate build system.

Moon is open source, runs locally, is written in Rust, and already provides the
hard CI/CD substrate: task graphs, dependency ordering, caching, affected-file
detection, workspace awareness, local/CI parity, and reporting hooks. Titania
uses Moon as the execution engine so Titania can focus on strict Rust QA policy,
typed findings, exception handling, receipts, and hallucination-resistant
automation.

Moon runs the DAG. Titania brings the wand, the rulebook, and the axe.

### Why not build a custom DAG engine?

Because that is not Titania's unique value in v1.

A custom DAG engine would force Titania to own scheduling, parallelism,
cancellation, cache invalidation, output restoration, affected-file detection,
cross-platform process handling, logs, artifacts, and CI annotations. Moon
already fights that war. Titania v1 is better if it is excellent on top of one
strong CI/CD substrate instead of mediocre across five homegrown abstractions.

### What if I do not like Moon?

Then Titania v1 is probably not for you.

Moon is the v1 orchestration substrate. Titania may expose an internal seam for
future CI engines, but the public v1 contract is Moon-powered on purpose.

### Why these coding rules?

Because Titania is opinionated on purpose.

The default policy targets codebases where reliability, reviewability,
AI-assisted development, and mechanical QA matter more than personal style
preferences. That means strict linting, fewer escape hatches, typed errors,
explicit failure modes, limited panic surfaces, source-shape rules, architecture
boundaries, and structured evidence.

### Can teams change the rules?

Yes, but changes must be explicit.

Titania ships one strict default policy. Teams may declare policy overrides and
exceptions, but Titania records those files, includes them in `policy_digest`,
and makes weakened policy visible in the receipt. If a team weakens the rules,
the evidence must prove that the rules were weakened.

### Why run locally?

Because waiting for remote CI to discover obvious failure is slow, noisy, and
expensive.

Titania treats the developer laptop as the first CI/CD layer. The same Moon
tasks should run locally and remotely, minimizing local/remote drift by making
Moon config, tool versions, policy files, and receipts explicit.

### Why not just GitHub Actions?

GitHub Actions is an execution environment. It is not a Rust QA policy, not a
typed finding model, not a local-first evidence system, and not a deterministic
repair loop for AI agents.

Titania can run inside GitHub Actions, but GitHub Actions is not the source of
truth. Moon and Titania define the quality contract; GitHub Actions merely hosts
it.

### Why not just clippy, cargo-deny, cargo-nextest, ast-grep, and friends?

Titania does not replace those tools. It weaponizes them together.

The value is one opinionated policy, one Moon DAG, one normalized report, one
exception model, one receipt, and one public contract for humans and AI agents
to repair against.

### Why shell out to libraries and tools instead of reimplementing everything?

Because the more proven tools Titania can compose safely, the better.

Titania should spend its complexity budget on typed domain modeling, process
evidence, policy digesting, finding normalization, and Moon integration — not on
rewriting cargo-deny, cargo-nextest, clippy, rustfmt, or a task scheduler.

### What does Titania catch?

Titania v1 targets common AI-generated and rushed-human Rust failures:

- unwrap/expect/panic happy-path lies
- unchecked indexing and unchecked arithmetic
- sloppy error modeling such as `Result<T, String>` in core code
- imperative source shapes where typed workflows are expected
- lint suppressions and policy bypasses
- architecture boundary violations
- formatting, compile, clippy, test, build, and supply-chain failures
- non-reproducible local-vs-CI behavior

### Is Titania security tooling?

Titania is QA and evidence tooling, not a security boundary.

It can run supply-chain checks, unsafe-code checks, policy scans, and strict
lint gates, but it does not sandbox malicious code. Cargo commands may execute
repository build scripts and tests. Titania improves quality evidence; it does
not make untrusted code safe to run.

### Is Titania only for AI-generated code?

No.

Titania is useful for human-written Rust too. AI just makes the need obvious:
hallucinated code exploits weak CI boundaries brutally. Titania gives both
humans and agents a stricter boundary with typed failures and receipts.

### What is "verification sprinkles"?

It is the playful wrapper around the serious mechanism.

The fairy dust is Moon tasks, clippy, cargo-deny, ast-grep, dylint,
panic-scan, policy-scan, typed JSON, policy digests, and evidence receipts.
Cute wrapper. Sharp teeth.

### What is explicitly not promised by v1?

v1 does not prove functional correctness, whole-program panic-freedom,
memory-safety beyond Rust's normal guarantees, malicious-code containment, or
safety certification. Those require later verification batches and, for
regulated domains, an external IV&V lifecycle.

---

## 19. References

- [`VISION.md`](./VISION.md) — the grand ambition
- Moon v2 skill — `.agents/skills/moon-v2/SKILL.md`
- Holzman Rust skill — `.agents/skills/holzman-rust/SKILL.md`
- Functional Rust skill — `.agents/skills/functional-rust/SKILL.md`
- ast-grep documentation — https://ast-grep.github.io/
- dylint documentation — https://github.com/trailofbits/dylint
