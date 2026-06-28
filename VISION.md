# Xtask: The High-Assurance Rust CI Toolchain

> **Vision v2.0** — JPL Power of Ten + Haskell + Gleam, expressed as Rust
> Companion: [`v1-spec.md`](./v1-spec.md) (the concrete, buildable v1 contract)

---

## 0. Vision Statement

Xtask is the high-assurance Rust CI toolchain for teams using Moon. It wraps
best-of-breed tools, pins them, normalizes their output, enforces architectural
rules, and produces evidence. It replaces bash-in-YAML pipelines with typed
Rust lanes, enforces strict coding standards by default, and accumulates formal
verification evidence in batches — from "the code has the right shape" to "the
code is mathematically proven correct."

Xtask is opinionated, not configurable. Its `strict-ai` policy is the default
profile. For medical, aerospace, or safety-critical work, the `strict-critical-rust`
profile adds JPL Power of Ten discipline, allocation constraints, architecture
enforcement, and formal verification. If you don't want strict Rust, don't use
xtask.

For actual medical (IEC 62304 Class C) or aerospace (NASA-STD-8739.8)
certification, tooling is one piece of a lifecycle process. Xtask provides the
technical foundation — the pipeline, the evidence, the policy enforcement. The
certification itself requires the full IV&V lifecycle, of which Xtask is the
automation layer.

---

## 1. The Problem

Rust CI in 2026 is bash-in-YAML. Every team reinvents the same pipeline:

```yaml
- run: cargo fmt --check
- run: cargo clippy -- -D warnings
- run: cargo test
- run: cargo audit
```

This has six structural failures:

1. **Untyped contracts.** Steps communicate via exit codes and stdout. No
   structured findings, no schema, no machine-readable evidence.
2. **No verification beyond "the linter didn't complain."** Clippy catches
   patterns; it does not prove panic-freedom, functional correctness, or type
   invariants. Real Rust verification tools exist but no CI toolchain
   integrates them.
3. **No batched evidence strategy.** No concept of "this PR proves shape; this
   merge proves panic-freedom; this release proves correctness." Evidence is
   binary: green or red.
4. **No reproducibility.** A green CI run produces a checkmark, not a structured
   record of *what was proven, by which tool, against which inputs.*
5. **No strict-opinion layer.** No canonical "this is what strict Rust looks
   like" policy that teams can adopt whole.
6. **No architectural enforcement.** No crate-level dependency rules, no module
   import constraints, no capability boundaries. Code rots architecturally with
   no signal until it's too late.

Every Rust team solves these independently. Xtask exists to solve them once.

---

## 2. The Doctrine

### 2.1 JPL Power of Ten — Rust translation

Gerard Holzmann's JPL Power of Ten rules are mechanically checkable constraints
for safety-critical C. Translated to Rust:

```
No panic surface.           — unwrap/expect/panic/todo/unreachable banned in production
No unchecked absence.       — Option/Result handled explicitly, no ignored fallible results
No first-party unsafe.      — unsafe_code = forbid
No architectural drift.     — crate graph + import graph + capability boundaries enforced
No dependency drift.        — lockfile pinned, supply chain scanned, versions bounded
No implicit effects.        — I/O behind traits, no ambient global state in core
No hidden allocation.       — critical paths allocation-free after init
No wildcard handling.       — no `_ => ...` match arms unless externally non-exhaustive
No stringly typed errors.   — no Result<T, String>, no Box<dyn Error> in core
No unreviewed features.     — feature powerset checked, combinations bounded
No warnings.                — zero warnings from compiler AND static analysis
No unowned exceptions.      — every suppression has owner + reason + expiry
```

The paper explicitly says: static-analysis warnings should go to zero, even when
rewriting is easier than arguing with the analyzer.

### 2.2 Haskell / Gleam influence

```
Pure core.                  — domain logic is pure, no I/O, no side effects
Explicit effects.           — effects live behind traits at the edges
Algebraic data types.       — enums carry data, states are unrepresentable illegally
Exhaustive handling.        — every match is total, no wildcards
Typed errors.               — domain-specific error enums, never String
Small modules.              — high cohesion, narrow scope
No ambient global state.    — everything passed explicitly
Boring functions.           — mechanical composability over cleverness
```

This is functional-core / imperative-shell, Rust edition:

```
core/       pure domain, typed errors, no async runtime, no I/O
ports/      traits and capability interfaces
adapters/   filesystem, network, database, clock, random
app/        orchestration
bin/        CLI / process boundary
```

### 2.3 The "strict-critical-rust" profile

For medical, aerospace, or safety-critical work, xtask provides an additional
profile beyond `strict-ai`:

| Rule | `strict-ai` | `strict-critical-rust` |
|---|---|---|
| Panic surface | banned | banned |
| First-party unsafe | forbid | forbid |
| Loops | banned (functional style) | banned unless `#[xtask_bounded_loop(max=N)]` |
| Allocation | normal | no allocation after init in critical modules |
| Recursion | banned (syntactic) | banned (with mutual/trait detection) |
| Architecture enforcement | advisory | reject on drift |
| Feature matrix | declared | powerset, bounded depth |
| Toolchain | pinned stable | Ferrocene or qualified |
| Verification | optional | Kani required on critical modules |
| Evidence | Receipt | Receipt + SBOM + audit trail |

---

## 3. The Grand Vision

Xtask becomes the canonical quality pipeline for Rust projects using Moon.

**Typed lanes replace shell scripts.** Every CI step is a Rust subcommand with
typed inputs and typed outputs (structured findings JSON, schema-versioned).

**Strict standards replace configuration.** The policy is the product. Teams
adopt it whole or don't use xtask.

**Verification batches replace binary gates.** Each scope tier is a commitment
about evidence — a structured claim about what has been proven.

**Architecture enforcement replaces decay.** The crate graph, module import
graph, and capability boundaries are checked mechanically. Code cannot drift
without a signal.

**Moon replaces ad-hoc orchestration.** Moon's task graph handles parallelism,
caching, affected-file detection, and CI integration.

---

## 4. Pipeline Stages and Verification Batches

### Stage overview

| Stage | Purpose | Wall time | Trigger |
|---|---|---|---|
| `edit` | Fast quality — shape, style, structure | seconds | every save |
| `prepush` | Full quality — tests, supply chain, features | minutes | before push |
| `full` | Resistance — coverage, mutation, API drift | tens of minutes | on PR |
| `deep` | Assurance — Miri, fuzz, Kani, sanitizers, concurrency | hours | nightly / merge |
| `release` | Evidence — SBOM, auditable, semver, reproducible build | full pipeline | on tag |

### Batch 1: Shape (`edit`)

**Claim:** "The code conforms to strict structural and style rules."

```
cargo fmt --all -- --check
cargo check --workspace --frozen
cargo clippy --lib --bins --frozen -- -D warnings -F <critical>
ast-grep scan
dylint scan
panic/assert/unsafe/allocation scan
architecture import scan
policy consistency scan
```

### Batch 2: Shippable (`prepush`)

**Claim:** "The code is ready for review — tests pass, dependencies are clean."

```
edit +
cargo nextest run --workspace --frozen
cargo hack check --workspace --feature-powerset --frozen
cargo audit --no-fetch
cargo deny check
cargo vet
cargo geiger
cargo machete
CodeQL (if configured)
```

### Batch 3: Resistant (`full`)

**Claim:** "Tests are strong enough to catch mutations; coverage is adequate."

```
prepush +
cargo llvm-cov
cargo mutants --workspace --cargo-arg=--frozen
mutation baseline validation
cargo public-api diff
cargo msrv verify
```

### Batch 4: Assurance (`deep`)

**Claim:** "The code is free from undefined behavior and proven on critical paths."

```
full +
cargo +nightly miri test
sanitizers (ASan, TSan, LSan)
cargo careful
cargo fuzz run <target> -- -max_total_time=300
cargo kani
Loom/Shuttle concurrency tests
```

### Batch 5: Evidence (`release`)

**Claim:** "The release artifact is reproducible and auditable."

```
deep +
cargo auditable build --release --frozen
cargo cyclonedx --format json
Syft SBOM
OSV-Scanner / Grype
cargo semver-checks
release evidence manifest
```

Each batch produces a `QualityReceipt` — a structured record of what was proven,
by which tool, against which source/policy/toolchain digests.

---

## 5. The Full Toolchain Map

### Tier 0 — Toolchain and Execution Control

| Tool | Role | Required? |
|---|---|---|
| **Ferrocene** | Qualified Rust toolchain for safety-critical (IEC 62304 Class C, ISO 26262, IEC 61508) | For medical/aerospace |
| **rustc / Cargo** | Standard Rust toolchain, pinned via `rust-toolchain.toml` | Yes |
| `cargo fetch --locked` | Prefetch dependencies for offline/frozen execution | Yes |
| `--frozen` everywhere | `--locked` + `--offline` = deterministic, no drift | Yes |
| **Tool pinning** | xtask captures resolved path, version, SHA-256 before+after, argv, exit code, output hashes/tails | Yes |

**Hermeticity rules:**
- `CARGO_HOME` controlled, read-only, digest-bound
- `RUSTUP_HOME` same
- Reject parent-directory cargo configs
- `.cargo/config` AND `.cargo/config.toml` both checked
- `RUSTC_BOOTSTRAP` = violation
- `RUSTC_WRAPPER` must be sccache or absent
- `RUSTFLAGS` / `CARGO_ENCODED_RUSTFLAGS` scanned, frozen

### Tier 1 — Formatting, Compilation, and Linting

| Tool | Command | Purpose |
|---|---|---|
| rustfmt | `cargo fmt --all -- --check` | Format drift = reject |
| cargo check | `cargo check --workspace --frozen` | Fast compile gate |
| Clippy | `cargo clippy --lib --bins --frozen` | Source-only, NOT `--all-targets` |

**Critical Clippy lints via `-F`:**
```
clippy::unwrap_used, expect_used, panic, panic_in_result_fn,
todo, unimplemented, indexing_slicing, string_slice, get_unwrap,
arithmetic_side_effects, dbg_macro, as_conversions,
let_underscore_must_use, await_holding_lock
```

Tests compile via `cargo test` — behavior-gated, NOT style-gated.

### Tier 2 — ast-grep Structural Rule Engine

ast-grep is the **primary house-rule engine**. Rules embedded in xtask via
`include_str!`, run via `ast-grep-core` Rust library.

**ast-grep owns:**
```
panic_surface         — unwrap/expect/panic/todo/unimplemented/unreachable
unsafe_surface        — first-party unsafe blocks/declarations
loop_surface          — for/while/loop in production
allocation_surface    — Vec::new, Box::new, format!, etc. in critical paths
effect_boundary       — std::fs, std::net, std::env, rand in core
architecture_imports  — core must not import tokio/axum/sqlx/reqwest
typed_error_policy    — Result<T, String>, Box<dyn Error> bans
match_exhaustiveness  — wildcard arm policy
test_nondeterminism   — thread_rng, SystemTime::now, Instant::now in tests
cfg_policy            — cfg/cfg_attr bans
lint_suppression      — #[allow]/#[expect] detection
macro_policy          — forbidden macros
```

### Tier 3 — JPL + Haskell + Gleam Rust Rules

These are the named profile rules, enforced via ast-grep + dylint + xtask-native
checks:

**Panic-surface discipline:**
```
Banned in production:    Allowed only in:
  .unwrap()                tests
  .unwrap_err()            benches
  .expect(...)             examples
  panic!(...)              checked-in policy exceptions
  todo!(...)               debug-only invariant profile
  unimplemented!(...)
  unreachable!(...)
  assert!(...)
  assert_eq!(...)
  assert_ne!(...)
```

**Typed absence:**
```
Ban:                          Prefer:
  Result<T, String>             enum ParseError { ... }
  Result<T, Box<dyn Error>>     enum ConfigError { ... }
  Option<T> from fallible       Result<T, DomainError>
    core without domain reason
```

**Exhaustive handling:**
```rust
// Rejected:
_ => ...

// Allowed (external non-exhaustive with named fallback):
other => handle_unknown_external_variant(other)
```

**Functional core (no loops):**
```rust
// Rejected:
for item in items { ... }
while cond { ... }
loop { ... }

// Preferred:
items.iter().map(...)
items.iter().filter_map(...)
items.iter().try_fold(...)
items.iter().try_for_each(...)
items.into_iter().collect::<Result<Vec<_>, _>>()?

// Bounded loop exception (JPL Power of Ten requires fixed bounds):
#[xtask_bounded_loop(max = 16, reason = "Fixed 8-iteration sensor loop")]
for lane in sensor_lanes { ... }
```

**Allocation discipline:**
```
normal profile:       allocation allowed
critical profile:     no allocation after init
embedded/no_std:      no std, no heap, no dynamic dispatch

Banned in critical paths:
  Vec::new(), Vec::with_capacity(), push, Box::new()
  String::new(), format!, collect::<Vec<_>>()
  HashMap::new, BTreeMap::new, Arc::new, Rc::new
```

**Unsafe discipline:**
```
first-party unsafe = reject
dependency unsafe  = visible (cargo-geiger) and reviewed (cargo-vet)
unsafe exception   = owner + reason + expiry + proof obligation
```

**Architecture doctrine:**
```
Reject:
  domain imports infrastructure
  core imports tokio, axum, sqlx, serde_json directly
  use of time/random/network/filesystem outside effect boundary

Required shape:
  core/     pure domain, typed errors, no async runtime, no I/O
  ports/    traits and capability interfaces
  adapters/ filesystem, network, database, clock, random
  app/      orchestration
  bin/      CLI / process boundary
```

### Tier 4 — Security SAST

| Tool | Role | When |
|---|---|---|
| **CodeQL** | Hosted SAST — dataflow, injection, unsafe patterns, cross-function reasoning | `prepush` (GA for Rust in 2025) |
| **Semgrep** | Optional SAST — security policy packs, taint patterns, multi-language repos | `prepush` (optional) |
| **SonarQube** | Dashboard layer — maintainability scoring, enterprise reporting | Not source of truth |
| **Snyk Code** | Commercial SCA + SAST | Optional |
| **Checkmarx / Qodana** | Only if org standardizes on them | Optional |

ast-grep handles Rust house style. CodeQL handles security dataflow. They
complement, not duplicate.

### Tier 5 — Supply-Chain Security

| Tool | Role | Required? |
|---|---|---|
| **cargo-audit** | Cargo.lock vs RustSec advisories | Yes |
| **cargo-deny** | Advisories + licenses + bans + sources + dupes | Yes |
| **cargo-vet** | Third-party dep audits, trusted-entity model | For safety-critical |
| **cargo-geiger** | Unsafe visibility in dep tree | Yes |
| **cargo-machete** | Unused dep detection (with baseline) | Yes |
| **cargo-udeps** | More accurate unused deps (nightly) | Deep lane |
| **cargo-crev** | Distributed code-review/trust web | Optional |
| **cargo-outdated** | Outdated dependency detection | Optional |

**Conflict resolution:** any advisory from cargo-audit OR cargo-deny rejects.
Duplicates normalized by advisory ID.

### Tier 6 — Dependency and Architecture Drift

| Tool | Role |
|---|---|
| **cargo_metadata** | Machine-readable workspace/package structure (used inside xtask) |
| **guppy** | Rust interface over Cargo dependency graphs — crate-level architecture rules, cycle detection, restricted paths |
| **cargo tree** | Evidence command — `--workspace --locked` and `-e features` |
| **cargo-hack** | Feature-powerset combination checks (bounded by policy) |
| **cargo-public-api** | Public API snapshot + diff between releases/commits |
| **cargo-semver-checks** | SemVer violation detection via rustdoc analysis |
| **cargo-msrv** | Minimum supported Rust version verification |

**Feature matrix policy:**
```toml
[feature_matrix]
mode = "powerset"       # or "bounded-depth" or "declared"
max_combinations = 64
skip = ["unstable", "nightly"]
```

### Tier 7 — Testing

| Tool | Role |
|---|---|
| **cargo test** | Baseline — `--workspace --frozen -- --test-threads=1` |
| **cargo-nextest** | Modern runner — per-test process isolation, CI speed, flaky isolation |
| **Test nondeterminism scanner** | ast-grep/xtask rules rejecting known nondeterminism sources |

**Banned in tests (nondeterminism):**
```
thread_rng(), rand::random(), SystemTime::now(), Instant::now(),
Utc::now(), Local::now(), StdRng::from_entropy()
```

**Allowed:**
```
StdRng::seed_from_u64(...), FakeClock, InjectedClock, DeterministicRng
```

### Tier 8 — Coverage and Mutation Resistance

| Tool | Role |
|---|---|
| **cargo-llvm-cov** | Line, region, and branch coverage via LLVM instrumentation |
| **cargo-tarpaulin** | Alternative coverage (optional, platform-dependent) |
| **cargo-mutants** | Mutation testing — finds where tests don't catch bugs |

**Mutation testing policy:**
```
cargo mutants --workspace --cargo-arg=--frozen
Requires: checked-in baseline, owner, reason, expiry, no inline excludes
```

Coverage tells you code was executed. Mutation tells you whether tests depended
on the result. For AI-authored Rust, mutation testing is one of the best tools
for making code less shallow.

### Tier 9 — Fuzzing and Property Testing

| Tool | Role |
|---|---|
| **cargo-fuzz** | libFuzzer-based fuzzing — required for parsers, codecs, protocols |
| **cargo-afl** | AFL++-style fuzzing — when team uses AFL ecosystem |
| **Bolero** | Unified fuzzing + property testing front-end — integrates with Kani |

**Required for:**
```
fuzz target per parser
fuzz target per codec
seed corpus checked in
crash corpus minimized
```

### Tier 10 — Undefined Behavior, Unsafe, and Runtime Bug Detection

| Tool | Role | When |
|---|---|---|
| **Miri** | UB detection via interpreter — catches unsafe violations on exercised paths | Required for unsafe-permitted profiles |
| **Sanitizers** | ASan (OOB, use-after-free, double-free), TSan (data races), LSan (leaks) | Required for deep native testing |
| **cargo-careful** | Extra-careful execution with nightly debug assertions | Optional deep lane |

```
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test -Zbuild-std --target x86_64-unknown-linux-gnu
RUSTFLAGS="-Zsanitizer=thread"  cargo +nightly test -Zbuild-std --target x86_64-unknown-linux-gnu
```

### Tier 11 — Concurrency Testing

| Tool | Role | When |
|---|---|---|
| **Loom** | Exhaustive concurrent execution permutation — atomics, locks, custom sync | Required for low-level concurrent code |
| **Shuttle** | Randomized concurrency testing — async services, task orchestration, channels | Required for async/concurrent systems |

**Policy:** all custom concurrency primitives require Loom or Shuttle tests.

### Tier 12 — Formal Verification

Used selectively on critical modules, not everywhere.

| Tool | Role | Best for |
|---|---|---|
| **Kani** | Bit-precise model checker — panic-freedom, arithmetic invariants, bounded algorithms | First formal tool for most teams |
| **Verus** | Spec/proof verification using solvers — functional correctness of low-level systems | Hard invariants, state machines, security kernels |
| **Prusti** | Viper-based verifier — contract correctness for supported Rust subsets | Experimental |
| **MIRAI** | Abstract interpreter for Rust MIR | Watch, do not center pipeline on it |

### Tier 13 — SBOM, Binary Auditability, and Release Evidence

| Tool | Role |
|---|---|
| **cargo-auditable** | Embed dependency info into compiled executables for post-release audit |
| **cargo-cyclonedx** | CycloneDX SBOM generation (JSON) |
| **Syft** | SBOM from filesystems and container images |
| **Grype** | Vulnerability scanning paired with Syft |
| **OSV-Scanner** | Scans Rust binaries with cargo-auditable metadata |
| **OpenSSF Scorecard** | Repository-level supply-chain hygiene checks |
| **Dependabot** | Background dependency maintenance (not a quality gate) |

---

## 6. Architecture Drift Enforcement

Three layers of architectural enforcement, because Cargo cannot see inside
crates and ast-grep alone cannot see across crates.

### 6.1 Crate graph (cargo_metadata + guppy)

```toml
[layer.core]
may_depend_on = ["types", "math"]
must_not_depend_on = ["tokio", "axum", "sqlx", "reqwest"]

[layer.adapters]
may_depend_on = ["core", "ports"]

[layer.app]
may_depend_on = ["core", "ports", "adapters"]
```

Reject crate-level violations before source scanning.

### 6.2 Module import graph (ast-grep)

```
core must not import tokio
core must not import std::fs
core must not import std::time::SystemTime
core must not import rand::thread_rng
domain must not import infrastructure
```

Catches architectural drift that happens inside a crate.

### 6.3 Capability boundaries

Enforce "effects only at edges" via trait-based capabilities:

```rust
trait Clock { fn now(&self) -> Instant; }
trait RandomSource { fn fill(&mut self, bytes: &mut [u8]) -> Result<(), RandomError>; }
trait FileStore { fn read(&self, key: Key) -> Result<Bytes, StoreError>; }
```

Then ban direct effect APIs in core:
```
SystemTime::now, Instant::now, std::fs, std::env, std::net,
rand::thread_rng, tokio::spawn, reqwest, sqlx
```

This is functional-core / imperative-shell, mechanically enforced.

---

## 7. Architecture Principles

### Moon-native
Moon is the orchestration substrate. xtask does not build its own scheduler,
cache, or affected-file detector.

### Single binary + dylint library
xtask ships as one binary plus a co-located dylint dynamic library for
type-aware lint scans.

### ast-grep embedded
Structural rules run via `ast-grep-core` embedded as a Rust dependency. Rules
ship embedded via `include_str!`.

### Strict-opinion, not configurable
Policy is the product. Escape is a policy PR with CODEOWNER approval, not a
CLI flag.

### Typed lanes with typed contracts
Every lane has typed input, output, execution, and evidence contracts.

### Fail-closed
Missing tool = `InputError`. Ambiguous policy = `PolicyError`. No silent
warnings. No "best effort."

### Tool pinning and evidence
Every external executable is pinned: resolved path, version, SHA-256 before
and after run, argv, exit code, output hashes and tails.

---

## 8. The Exception Schema

No owner, no reason, no expiry, no exception.

```toml
[[exceptions]]
rule_id = "FUNC_LOOPS_BOUNDED"
path = "src/control/loop.rs"
owner = "flight-control"
reason = "Fixed 8-iteration control loop over sensor lanes"
expires_on = "2026-12-31"
review = "SAFETY-1234"
```

Every suppression — `#[allow]`, cargo-audit `--ignore`, cargo-deny exception,
cargo-vet exemption, cargo-mutants `--exclude` — must be in a checked-in policy
file with this schema.

---

## 9. What Xtask Enforces Directly

Xtask-native checks (not delegated to external tools):

```
tool pin verification           — resolved path + SHA-256 before/after
policy digest verification      — sha256 of active policy
source digest                   — content-addressed source tree
Cargo.lock digest               — content-addressed lockfile
workspace metadata digest       — cargo_metadata hash
feature profile digest          — feature matrix config hash
mutation baseline digest        — baseline file hash
tool output digest              — per-lane output hash
exception ownership             — every exception has an owner
exception expiry                — expired exceptions reject
architecture layer graph        — crate-level dependency rules
allowed dependency graph        — what crates may depend on what
forbidden import graph          — module-level import constraints
no inline suppressions          — all suppressions in policy files
```

---

## 10. The Competitive Landscape

| Tool | Typed lanes | Strict Rust policy | Verification batches | Architecture enforcement | Moon-native |
|---|---|---|---|---|---|
| GitHub Actions | no | no | no | no | no |
| Dagger | yes | no | no | no | no |
| Bazel / Buck2 | yes | no | no | partial | no |
| Moon alone | yes | no | no | no | — |
| cargo-* point tools | N/A | N/A | N/A | no | no |
| **xtask** | **yes** | **yes** | **yes (5 stages)** | **yes (3 layers)** | **yes** |

### Dagger: complement, not competitor
Dagger is general typed CI. xtask is Rust-specific quality. They compose.

### Bazel / Buck2: different game
Build systems with typed rules. No coding-standard enforcement, no verification
integration. Different category.

### Moon alone: substrate, not policy
Moon provides orchestration. xtask provides the opinionated Rust policy layer.

---

## 11. The Strategic Moat

### 1. The verification stack
Kani + Flux + Verus + Miri + Loom/Shuttle + cargo-fuzz + sanitizers — each is a
multi-week integration. Once composed, they form a stack strictly stronger than
any individual tool. Replicating is a multi-year effort.

### 2. The policy curation
Every lint, threshold, `-F` flag, and rule is chosen for a reason and tested
against real code. The judgment behind `strict-ai` and `strict-critical-rust`
cannot be copied without doing the work.

### 3. The architecture enforcement
Three-layer drift detection (crate graph + import graph + capability boundaries)
is bespoke. No other Rust CI tool does this.

### 4. The evidence trail
Every Receipt records what was proven, by which tool, against which digests. This
is auditable evidence, not a green checkmark. For safety-critical work, this is
the deliverable.

---

## 12. Evolution Timeline

Not dates. Sequence. Each version ships something usable on its own.

### v1.0 — Typed CI with strict-ai (the foundation)
Moon-native, single binary, ast-grep + dylint, cargo built-ins, cargo-deny.
Strict-opinion enforcement. Architecture import scan (ast-grep). No formal
verification yet. This alone is better Rust CI than 99% of repos have.

### v1.5 — Kani panic-freedom + cargo-mutants
Add Kani to `full` scope. Add cargo-mutants with baseline. Every PR proves
critical paths cannot panic and tests catch mutations.

### v2.0 — Flux refinement types + architecture enforcement
Add Flux to `full` scope. Add guppy-based crate graph enforcement. Domain
newtypes carry refinement invariants. Architecture drift rejects.

### v2.5 — Verus functional correctness + `deep` scope
Add Verus and `deep` scope tier. Add Miri, sanitizers, cargo-fuzz. Pure domain
logic carries formal specs. The Receipt records which functions are verified.

### v3.0 — Safety-critical profile + release evidence
Add `strict-critical-rust` profile. Add Ferrocene support. Add cargo-auditable,
cargo-cyclonedx, Syft, OSV-Scanner. Full SBOM + audit trail. IEC 62304 / ISO
26262 evidence packaging.

### v3.5+ — Team scale and beyond
- Remote cache (bazel-remote shared across machines)
- Affected-file-driven incremental gates
- Persistent Receipt ledger (SQLite)
- Shuttle concurrency verification
- Loom exhaustive concurrency verification
- Antithesis deterministic simulation
- cargo-vet integration for full audit chain
- CodeQL integration for SAST
- Multiple policy profiles

---

## 13. The Strongest Recommended Stack

If we had to pick the actual final-state stack, not just list options:

```
Compiler/toolchain:
  Ferrocene (regulated) or pinned stable Rust

Core:
  Cargo, rustfmt, Clippy, cargo_metadata

Structural doctrine:
  ast-grep (embedded) + dylint (type-aware)

Architecture:
  guppy + cargo_metadata + ast-grep import rules + capability boundary scan

SAST:
  CodeQL (+ optional Semgrep for security packs)

Supply chain:
  cargo-audit, cargo-deny, cargo-vet, cargo-geiger, cargo-machete
  cargo-cyclonedx, cargo-auditable, OSV-Scanner

Testing:
  cargo-nextest, cargo-llvm-cov, cargo-mutants

Dependency/API drift:
  cargo-hack, cargo-public-api, cargo-semver-checks, cargo-msrv

Deep assurance:
  Miri, sanitizers (ASan/TSan/LSan), cargo-careful
  cargo-fuzz (+ Bolero for Kani integration)
  Kani (panic-freedom + bounded proofs)
  Verus (functional correctness)
  Loom + Shuttle (concurrency verification)

Release evidence:
  cargo-auditable, cargo-cyclonedx, Syft, Grype, OSV-Scanner
  cargo-semver-checks, OpenSSF Scorecard
```

---

## 14. What Xtask Is Not

- **Not a sandbox.** Runs Cargo and developer tools, which may execute repo code.
- **Not a security boundary.** No signing, no deploy-gate, no artifact trust.
- **Not a formal proof system in v1.** Verification batches land starting v1.5.
- **Not a replacement for cargo.** Orchestrates cargo; does not replace it.
- **Not for non-Moon users.** Moon is required.
- **Not configurable.** Policy is the product. Escape is a policy PR.
- **Not a general CI engine.** Rust-specific.
- **Not a certification.** Xtask provides technical evidence. IEC 62304 / NASA-
  STD-8739.8 certification requires the full IV&V lifecycle, of which Xtask is
  the automation layer.

---

## 15. Success Criteria

Xtask has succeeded when:

1. **Every PR carries structured findings** — no more reading CI logs.
2. **Every merge carries panic-freedom proofs** (post-v1.5) — Kani harnesses.
3. **Every release carries a reproducible Receipt + SBOM** — source/lock/policy/
   toolchain digests + CycloneDX + auditable binary.
4. **The strict-ai policy is the reference standard** — "this is what strict
   Rust looks like."
5. **The verification stack is the defensible niche** — no other Rust CI tool
   integrates Kani + Verus + Miri + Loom + cargo-fuzz. xtask owns that category.
6. **Architecture drift is mechanically impossible** — crate graph + import
   graph + capability boundaries enforced on every PR.
7. **Safety-critical Rust teams use `strict-critical-rust`** — medical (IEC
   62304), aerospace (NASA-STD-8739.8), automotive (ISO 26262) teams adopt xtask
   as their evidence pipeline.
8. **Moon ecosystem recognizes xtask as the Rust policy layer** — listed in
   Moon's ecosystem docs.

---

## 16. References

### Standards
- [NASA-STD-8739.8](https://standards.nasa.gov/standard/NASA/NASA-STD-87398) — Software Assurance and Software Safety Standard
- [JPL Power of Ten](https://spinroot.com/gerard/pdf/P10.pdf) — Holzmann's rules
- IEC 62304 — Medical device software lifecycle (FDA-recognized)
- ISO 26262 / IEC 61508 — Automotive / industrial functional safety

### Companion documents
- [`v1-spec.md`](./v1-spec.md) — the concrete, buildable v1 contract
- [`architecture-spec.md`](./architecture-spec.md) — v6.0 historical spec (superseded)

### Key tools
- [Ferrocene](https://ferrocene.dev/) — qualified Rust toolchain
- [Moon](https://moonrepo.dev/) — build system and CI orchestration
- [ast-grep](https://ast-grep.github.io/) — structural search/lint
- [Kani](https://github.com/model-checking/kani) — Rust model checker
- [Verus](https://github.com/verus-lang/verus) — verified Rust
- [cargo-deny](https://docs.rs/crate/cargo-deny/latest) — supply chain checks
- [cargo-vet](https://mozilla.github.io/cargo-vet/) — dependency audit
- [cargo-mutants](https://mutants.rs/) — mutation testing
- [cargo-nextest](https://nexte.st/) — modern test runner
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — coverage
- [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) — fuzzing
- [Miri](https://github.com/rust-lang/miri) — UB detection
- [Loom](https://github.com/tokio-rs/loom) — concurrency testing
- [Shuttle](https://github.com/awslabs/shuttle) — randomized concurrency testing
- [guppy](https://crates.io/crates/guppy) — dependency graph analysis
- [cargo-auditable](https://rustsec.org/) — binary auditability
- [cargo-cyclonedx](https://github.com/cyclonedx/cyclonedx-rust-cargo) — SBOM
- [CodeQL](https://docs.github.com/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning-with-codeql) — SAST

### Skills
- Moon v2 — `.agents/skills/moon-v2/SKILL.md`
- Holzman Rust — `.agents/skills/holzman-rust/SKILL.md`
- Functional Rust — `.agents/skills/functional-rust/SKILL.md`
