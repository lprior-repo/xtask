# Architecture Spec: Xtask — The Deterministic Rust Quality Enforcement Gate

> Status: ARCHITECTURE SPEC v2.0 (Restate removed; general Rust quality tool)
> Doctrine sources read in full: holzman-rust (SKILL + nasa-jpl-standards + latency-throughput-playbook + runtime-performance-architecture + zero-cost-abstractions + simd-patterns + mechanical-empathy-toolchain) and functional-rust (SKILL + scott-ddd-types + typing-refactor-checklist + complete-workflow).
> Next step: run `arch-spec-to-beads` to shred this into molecular tasks.

---

## 0. Product Sentence

```
Xtask is a deterministic CLI that wraps the full NASA/JPL Power-of-Ten +
functional-Rust toolchain and gates AI-authored Rust. It is the DECIDE
step of an AI authoring OODA loop: it returns a structured per-layer
verdict and a signed certificate, or it rejects. It contains zero
macros, zero DSL, zero LLM, zero runtime — it is 100% mechanical static
analysis that wraps real tools. When Xtask passes, you KNOW the code is
of very high quality, because a vast set of decidable properties has
been mechanically proven and the AI cannot route around the gate.
```

Xtask gates **arbitrary Rust**. It is not specific to any SDK, framework, or domain. The AI writes real Rust; Xtask enforces that it satisfies the complete Holzman + functional-rust discipline.

---

## 1. Non-Goals (Explicit Exclusions)

- **NO macros, NO DSL, NO grammar.** Xtask never parses a constrained language. It gates real Rust via real tools. (Non-negotiable — repeated.)
- **NO LLM inside the gate.** Xtask contains zero non-determinism. The AI is external and consumes Xtask's JSON.
- **NO runtime, NO execution.** Xtask never runs the gated code's logic. It only verifies.
- **NO code generation as source of truth.** Xtask emits typed repair *hints*, never the author's final code.
- **NO bypass flag.** No `--force`, no `--trust-me`, no inline `#[allow]` for the gated codebase. The only escape is a policy-PR.
- **NO domain coupling.** No Restate, no SDK-specific rules in v1. Pure general Rust quality.

---

## 2. Positioning — The AI OODA Loop

```
OBSERVE   AI reads context, existing code, Xtask's prior verdict
ORIENT    AI generates candidate Rust
DECIDE    Xtask gate — runs the full toolchain, emits verdict + certificate
ACT       Accepted code (valid certificate) ships
```

The AI is the **primary invoker**, running Xtask dozens of times per task in a tight repair loop. CI/Moon is secondary (canonical deploy gate). Local human runs are tertiary.

---

## 3. EARS Requirements

### Ubiquitous language
- **Layer** — one enforcement stage wrapping one or more external tools.
- **Verdict** — the typed result of a single Xtask invocation over an input unit.
- **Finding** — a single rule violation with span, rule id, and typed repair hint.
- **Certificate** — the Ed25519-signed artifact binding source + per-gate + policy + toolchain digests, emitted ONLY on aggregate Pass.
- **Policy** — the checked-in set of rule definitions + thresholds (clippy.toml, .xtask/semgrep/, deny.toml, rust-toolchain.toml, the xtask policy manifest). Policy changes require a PR through the same gate.

### Event-driven
- **When** the AI submits a crate or diff, **the system shall** run all layers in order, short-circuiting only where a layer's precondition is unmet.
- **When** any layer cannot produce a verdict (tool crash, timeout, missing binary), **the system shall** emit aggregate Reject, emit NO certificate, and report the layer as a `GateFailure`.
- **When** all layers emit `Pass`, **the system shall** emit aggregate Pass, compute and sign a certificate, and write it to the output path.
- **When** a policy file is malformed, **the system shall** emit a `PolicyError` and reject; this is NOT a code violation.

### State-driven
- **While** a certificate is valid and unmodified, **the deploy-gate shall** permit deployment; **otherwise it shall** reject.

### Unwanted
- **If** the deploy-gate receives a deployment request with no certificate or a certificate whose digests do not match the artifact, **the system shall NOT** deploy.
- **If** any layer emits a finding, **the system shall NOT** emit a certificate.

---

## 4. KIRK Contracts

### 4.1 Core invariant: Fail-Closed
> An aggregate Pass verdict and a valid certificate are emitted if and only if every layer produced a `Pass`. Any layer that cannot produce a verdict forces aggregate Reject with no certificate. Code never passes by default.

### 4.2 Determinism of Xtask itself
Xtask is **deterministic**: identical input (source bytes + Cargo.lock + toolchain pin + policy bytes) produces an identical verdict and identical certificate digests. The AI's repair loop depends on this.

### 4.3 The verdict type (the AI's contract) — disjoint typed enum

Misrouting across these categories is impossible at the type level:

```rust
pub enum Verdict {
    Pass { certificate: Certificate, per_layer: Box<[LayerOutcome]> },
    Reject { findings: Box<[Finding]>, per_layer: Box<[LayerOutcome]> },
}

pub enum LayerOutcome {
    Pass,
    Violations(Box<[Finding]>),       // code problems — AI edits code
    GateFailure(LayerFailure),        // tool crashed/missing/timeout — infra, do NOT edit code
    Skipped { reason: SkipReason },   // only when a prior layer failed (short-circuit)
}

pub enum TopLevelError {
    PolicyError(PolicyDiagnostic),    // policy malformed — edit policy, NOT code
    InputError(InputDiagnostic),      // not a crate, unreadable diff, path missing
}
```

`Finding` (CodeViolation), `LayerFailure` (GateFailure), and `PolicyDiagnostic` (PolicyError) are **three disjoint types with three disjoint fix paths**.

### 4.4 The finding type (typed repair hints)

```rust
pub struct Finding {
    pub layer: Layer,
    pub rule_id: RuleId,
    pub severity: Severity,           // Error | Warning
    pub span: Span,                   // file, line/col start+end
    pub message: String,
    pub repair: RepairHint,
}

pub enum RepairHint {
    ReplaceWith { old: String, new: String },
    UseIteratorPipeline { suggestion: String },   // for forbidden loops
    FlattenNesting { suggestion: String },         // for >2 nesting
    ReplaceWithCheckedArith { op: &'static str }, // + -> checked_add
    RemoveUnsafeWaiver,                            // unsafe needs explicit approval
    RequiresHumanReview { note: String },
}
```

---

## 5. The Complete Doctrine Xtask Enforces

This is the full Holzman (NASA/JPL Power of Ten + PLUS) plus functional-rust doctrine, read from the reference files. Every item maps to a checkable layer.

### 5.1 Power of Ten (Holzman Rule 1–10) — source: nasa-jpl-standards.md

| # | Rule | Xtask enforcement |
|---|---|---|
| 1 | Simple control flow: no recursion/panic-driven flow | clippy + semgrep: deny recursion in critical paths; panic-driven control flow blocked by panic deny |
| 2 | **Fixed loop bounds: every loop needs static upper bound or termination proof** | semgrep flags unbounded `loop{}`/`while`/`for`; functional-rust goes further — **no imperative loops at all** (see 5.3) |
| 3 | No post-init dynamic allocation in critical paths | budget policy (allocation-count gate on hot-path modules, opt-in); clippy `box_collection` etc. |
| 4 | Functions ≤ one page (~60 lines, ≤25 hot) | clippy `too_many_lines` (threshold 40, tunable) |
| 5 | Assertion density via types/constructors; production `assert!` is a panic path | the **production-assert-macro scan** (rg for `assert!`/`assert_eq!`/`assert_ne!`/`unreachable!` outside tests/benches/examples/build.rs) = Reject |
| 6 | Smallest scope | clippy `needless_late_init` etc. |
| 7 | Checked returns: never ignore Result/Option/handles | `unused_must_use` = deny; `let_underscore_must_use` = deny |
| 8 | Limited macros: must not hide allocation/panic/unsafe/loops | semgrep macro-audit rules |
| 9 | Restricted pointers: raw pointers/dyn Trait/FFI behind safe wrappers | `unsafe_code` = forbid; `as_conversions` = deny; FFI flagged |
| 10 | Zero warnings + strong static analysis | `-D warnings` everywhere; the full toolchain gate |

### 5.2 Panic-Free Standard (Holzman) — mechanically unbeatable

Production-reachable code is **rejected** if it contains:
- `unsafe` blocks/functions/traits/impls/raw-pointer deref/transmute (forbid)
- `unwrap`, `expect`, `panic`, `todo`, `unimplemented`, `unreachable!`
- indexing `items[i]` without bound proof (`indexing_slicing` deny)
- `string_slice`, `get_unwrap`
- `parse().unwrap()`, `Mutex::lock().unwrap()`, `send().unwrap()`
- production `assert!`/`assert_eq!`/`assert_ne!` (the rg scan)
- `dbg!`, ignored `Result`, `let _ =` on must-use
- unchecked arithmetic (`arithmetic_side_effects` deny)
- lossy `as` conversions (`as_conversions` deny)

### 5.3 Functional-Rust Doctrine — source: functional-rust SKILL + references

| Rule | Enforcement |
|---|---|
| **ZERO unwrap in any form** (incl. `unwrap_or`/`unwrap_or_else`/`unwrap_or_default`) | clippy `unwrap_used` deny + semgrep for the `unwrap_or*` family (clippy only catches `unwrap_used`, not `unwrap_or*`) |
| No swallowed errors | `unused_must_use` deny + semgrep catch-empty-block |
| **Linear control flow, ≤2 nesting levels** | semgrep nesting-depth rule (clippy `cognitive_complexity` is advisory; hard gate needs semgrep) |
| **NO imperative loops (`for`/`while`/`loop`)** — use Iterator/Stream/Rayon | semgrep rule matching loop keywords in source (clippy has no blanket no-loops lint) |
| One function one job (~60 lines) | clippy `too_many_lines` |
| Surface side effects (I/O only in Actions layer) | architectural; semgrep flags obvious hidden I/O |
| Make illegal states unrepresentable (enums/typestates) | `non_exhaustive_patterns`/`unreachable_patterns` deny; remove wildcard arms (semgrep) |
| Parse don't validate (boundary newtypes) | architectural guidance; not a hard gate |
| No `mut` by default | clippy `redundant_mut`/`unnecessary_mut_passed` |
| Zero-copy parsing (`&'a str`, `Cow`, `Bytes`) | architectural; clippy `clippy::all` catches some |
| `thiserror` for core errors, `anyhow` for shell; no `Result<T, String>` | semgrep `Result<_, String>` rule |
| No bool control flags (use enums) | clippy `fn_args_justly`/semgrep |

### 5.4 PLUS Performance Gates (Holzman) — opt-in budget lanes

These are correctness-of-claim gates, not correctness-of-compilation. They activate when a module is marked hot/critical in policy:
- Latency budget (p50/p95/p99) — requires benchmark evidence
- Throughput budget — requires benchmark evidence
- Allocation budget — allocation-count gate; `try_reserve` on untrusted growth
- Storage placement — measured stack/heap/arena choice
- Cache layout — `size_of` review, field order, AoS/SoA
- Static dispatch hot path — `dyn Trait` flagged in hot modules
- SIMD discipline — scalar oracle + fallback + target gate + benchmark (unsafe SIMD needs explicit waiver)
- Concurrency budget — bounded queues/tasks/retries; `await_holding_lock` deny

### 5.5 Supply-Chain & Dependency Discipline (Holzman)

- `cargo audit` — known advisories
- `cargo deny check` — advisories, licenses, banned crates, duplicate versions
- `cargo vet` — trusted dependency audit
- `cargo geiger` — `unsafe` in dependency tree above threshold = Reject
- `cargo machete` — unused dependencies = Reject
- `cargo hack check --workspace --feature-powerset` — every feature combo compiles = Reject on breakage

### 5.6 Mutation & Feature Correctness (Holzman)

- `cargo mutants` — mutation testing; surviving mutants = finding (the tests did not catch a mutation)
- `cargo hack --feature-powerset` — no feature combination breaks compilation

### 5.7 Pinned Toolchain (Holzman)

- Checked-in `rust-toolchain.toml` with pinned **dated** channel (nightly-YYYY-MM-DD or a pinned stable), `profile = "minimal"`, components `rustfmt clippy rust-src llvm-tools-preview`.
- Allowed source features by default: `portable_simd`, `try_blocks` only.
- `RUSTC_BOOTSTRAP` and arbitrary feature gates = policy violation.
- Toolchain digest bound into the certificate.

---

## 6. The Enforcement Layers (in execution order)

| Layer | Tool(s) | Budget | Rejects | Repair hint |
|---|---|---|---|---|
| 0 | `cargo fmt --check` | <2s | formatting drift | ReplaceWith |
| 1 | `cargo check` + rustc lints (`-D warnings`, `unsafe_code` forbid, `unused_must_use` deny, `non_exhaustive_patterns`/`unreachable_patterns` deny, `rust_2018_idioms` deny, `dead_code` deny) | <10s | compile errors, forbidden lints | compiler spans |
| 2 | `cargo clippy` (the full deny list from 5.2; source-only: `--lib --bins --examples`) | <15s | all panic-surface + footgun lints | clippy lint id + hint |
| 3 | `semgrep` (.xtask/semgrep/) | <30s | functional-rust structural rules: no imperative loops, >2 nesting, `unwrap_or*`, `Result<_,String>`, wildcard arms, hidden I/O, macro-audit, raw `as` survivors | typed hint (UseIteratorPipeline, FlattenNesting, etc.) |
| 4 | production-assert-macro scan (`rg` for `assert!`/`assert_eq!`/`assert_ne!`/`unreachable!` outside tests/benches/examples/build.rs) | <2s | panic-path macros in production | RequiresHumanReview |
| 5 | supply chain: `cargo audit` + `cargo deny check` + `cargo vet` + `cargo geiger` + `cargo machete` | <20s | advisories, banned, unsafe-dep, unused deps | supply findings |
| 6 | feature correctness: `cargo hack check --workspace --feature-powerset` | <60s | a feature combo breaks | feature findings |
| 7 | mutation testing: `cargo mutants` | <120s (heavy lane) | surviving mutants | mutation findings |

**Short-circuit rule:** layers that depend on compilation (3, 7) are `Skipped` if Layer 1 fails. Layers 0–6 are fast and always run. Layer 7 is heavy and may be policy-scoped (run in CI `--scope full`, optional locally) — BUT if scoped-in and unrunnable, it is `GateFailure` → aggregate Reject (fail-closed holds).

**Source vs test:** per Holzman + Moon skill, strict linting is **source-only**. Tests must compile, run, prove behavior with exact assertions, and stay deterministic — but test implementation style is NOT clippy-gated. Layer 4's rg scan explicitly excludes `**/tests/**`, `**/benches/**`, `**/examples/**`, `build.rs`.

---

## 7. The Certificate Model (artifact-bound trust)

```rust
pub struct Certificate {
    pub schema_version: u16,
    pub source_digest: Digest,           // blake3 of gated source tree
    pub cargo_lock_digest: Digest,
    pub per_layer: Box<[LayerDigest]>,   // each layer's pass evidence digest
    pub policy_digest: Digest,           // blake3 of all policy files
    pub toolchain_digest: Digest,        // rust-toolchain.toml + pinned tool versions
    pub hot_path_module_set_digest: Digest,
    pub timestamp_utc: DateTime<Utc>,
    pub signature: Option<Ed25519Signature>, // None = unsigned (local); Some = CI-signed
}
```

- **Ed25519.** CI holds the signing key as a secret. Local runs without the key emit an `unsigned` certificate (valid for repair-loop feedback, NOT deploy-acceptable).
- **Deploy-gate:** a Moon task that recomputes every digest from the actual artifact + policy + toolchain, compares against the certificate, and requires a CI signature. Mismatch/absent ⇒ deploy REJECTED. If the verifier itself cannot run ⇒ deploy REJECTED (fail-closed).

---

## 8. Escape Hatch (policy-PR only)

- **NO per-site bypass.** No inline `#[allow]` for the gated codebase.
- The ONLY way to ship a file that triggers a rule is to **edit the policy files** (clippy.toml, .xtask/semgrep/, deny.toml, the policy manifest, the hot-path module sets).
- Editing policy requires a **PR that itself passes the same Xtask gate**.
- This is the Swamp "policy-guided" principle made mechanical: adaptation is explicit (a diff), inspectable (in the PR), policy-guided (lives in files), artifact-bound (new policy_digest in the certificate).

---

## 9. Toolchain Requirements (hard-required, pinned, fail-closed)

Xtask shells out to: `cargo`/`rustc`/`rustfmt`/`clippy` (pinned via rust-toolchain.toml), `semgrep`, `cargo-audit`, `cargo-deny`, `cargo-vet`, `cargo-geiger`, `cargo-machete`, `cargo-hack`, `cargo-mutants`, plus `rg` for the assert-macro scan.

- **Quality tools (fmt/clippy/semgrep/audit/deny/vet/geiger/machete/hack)** are hard-required always.
- **Heavy tools (mutants)** are hard-required **when scoped-in** by policy (`--scope full`). A missing scoped-in tool ⇒ `GateFailure` + Reject + clear "install X" message. No silent degradation.
- **All versions pinned** and bound into `toolchain_digest`.
- Local must mirror CI's toolchain. The contributor installs the pinned set.

---

## 10. Moon CI/CD Integration

```yaml
# .moon/tasks/all.yml
gate:
  command: 'xtask gate --emit json --out target/xtask/verdict.json'
  toolchains: [rust]
  options: { runInCI: true }
  inputs: ['@globs(sources)', '.xtask/**', 'Cargo.lock', 'rust-toolchain.toml']
  outputs: ['target/xtask/verdict.json', 'target/xtask/certificate.json']

deploy-gate:
  command: 'xtask verify-cert target/xtask/certificate.json --require-signature'
  options: { runInCI: true }
```

- `moon ci` is the canonical gate; runs `:gate` on affected targets.
- Moon's source-lint zero-tolerance (`-W clippy::all`) reinforces Layer 2.
- Moon's remote cache (bazel-remote) + sccache accelerate re-runs; Xtask's verdict is a cacheable output.

---

## 11. Error Taxonomy (exhaustive)

### Verdict-level (disjoint)
- `Pass` — certificate emitted.
- `Reject{findings}` — code violations; AI edits code.
- `PolicyError` — policy malformed; edit policy.
- `InputError` — input contract violated.

### Layer failures (`GateFailure` — infra, do NOT edit code)
- `ToolMissing`, `ToolCrashed`, `ToolTimeout`, `ToolVersionMismatch`, `PanicInGate`.

### Rule families (`RuleId`)
- `HOLZMAN_PANIC_*` — unwrap/expect/panic/todo/indexing/assert-macro (5.2)
- `HOLZMAN_UNSAFE_*` — unsafe/raw-pointer/transmute/as-conversion (5.1 R9)
- `HOLZMAN_BOUNDS_*` — unbounded loop, missing termination (5.1 R2)
- `HOLZMAN_CHECKED_*` — ignored Result, arithmetic side effects (5.1 R7)
- `FUNC_NESTING_*` — >2 nesting depth (5.3)
- `FUNC_LOOPS_*` — imperative for/while/loop (5.3)
- `FUNC_TYPES_*` — Result<_,String>, wildcard arm, bool flag (5.3)
- `SUPPLY_*` — advisory/banned/unsafe-dep/unused-dep (5.5)
- `MUTANT_*` — surviving mutation (5.6)

### Certificate errors (deploy-gate)
- `CertAbsent`, `CertUnsigned`, `DigestMismatch`, `PolicyDigestMismatch`, `ToolchainDigestMismatch`.

---

## 12. Second-Order & Pre-Mortem

**Blast radius:**
- CI signing key leak ⇒ forged certs. Mitigation: key rotation + deploy-gate recomputes digests (forged signature against mismatched artifact still fails).
- Deploy-gate verifier bug ⇒ false-accept. Mitigation: verifier is Xtask-gated code; digest recompute is a pure function with property tests.
- Slow heavy lanes (mutants) ⇒ AI repair loop stalls. Mitigation: heavy lane policy-scoped; the fast lanes (0–6) give the AI immediate signal.
- Policy-PR used to weaken rules ⇒ the weakening is itself a visible, auditable PR with a new policy_digest.

**3 AM disaster:** bad code shipped. Most likely cause: a property Xtask **cannot** statically prove — e.g. a logical correctness bug that passes all lints/tests/mutations. Xtask guarantees code *quality discipline*, not *behavioral correctness* of the algorithm. The honest boundary: Xtask proves the code is disciplined and panic-free; it does not prove the code does the right thing. Tests + mutants narrow that gap; they do not close it. The certificate records which lanes ran so incident response knows the quality floor.

**The invariant that must NEVER break:** fail-closed. Any path that can emit Pass without all scoped layers verifiably passing kills the product.

---

## 13. The Honest Trust Boundary

Xtask makes it mechanically impossible to ship Rust that violates any **decidable** property in the Holzman + functional-rust doctrine — panic freedom, unsafe absence, exhaustiveness, checked returns, bounded loops, no imperative loops, supply-chain hygiene, mutation resistance, and feature-combo compilation. That is a vast, genuinely unbeatable set.

Xtask does **not** prove behavioral correctness (the algorithm is right) — no static tool does. Tests + cargo-mutants narrow this; they never close it. The certificate is the quality floor, not a correctness proof.

---

## 14. Component / Module Map (for decomposition)

Single Cargo workspace. Proposed crates (decomposer refines):

- `xtask-bin` — CLI entrypoint (clap). Subcommands: `gate`, `verify-cert`, `doctor`.
- `xtask-core` — domain types: `Verdict`, `Finding`, `Layer`, `RuleId`, `RepairHint`, `Span`, `Severity`, `Certificate`, `LayerOutcome`, `LayerFailure`.
- `xtask-policy` — policy loading, validation, rule catalog, module-set loading (hot-path sets), `policy_digest`.
- `xtask-layers` — the layer runners; each wraps a tool, parses output, maps to Findings:
  - `fmt`, `rustc`, `clippy`, `semgrep`, `assert_scan`, `supply` (audit/deny/vet/geiger/machete), `feature` (hack), `mutants`
- `xtask-certificate` — Ed25519 signing/verification, digest computation, serialization.
- `xtask-output` — verdict JSON schema, `doctor` diagnostics.

All first-party crates enforce the Holzman/functional-rust lint policy on themselves (Xtask eats its own dog food — it must pass its own gate).

---

## 15. CLI Surface

```
xtask gate [--input <crate|diff>] [--emit json] [--out <path>] [--scope fast|full]
    Run layers. fast = layers 0–6 (default for AI loop). full = +mutants (CI).
    Emit verdict JSON + exit code. Emit certificate on Pass.

xtask verify-cert <cert.json> [--require-signature]
    Recompute digests; verify signature. Deploy-gate primitive.

xtask doctor
    Enumerate required tools, versions, health. Fail-closed report.
```

- Exit codes: `0` Pass, `1` Reject, `2` PolicyError, `3` InputError, `>=4` GateFailure/internal.

---

## 16. Definition of Done (Xtask v1)

1. `xtask gate --scope fast` runs layers 0–6 and emits the disjoint verdict JSON + exit code.
2. `xtask gate --scope full` adds mutants (CI path).
3. A Pass emits a valid Ed25519 certificate binding all digests; CI signs it.
4. Any unrunnable scoped layer forces aggregate Reject with no certificate (fail-closed, verified by test).
5. The full Holzman panic-free + functional-rust no-loops/no-nesting doctrine is encoded and enforced.
6. The verdict's three failure categories (CodeViolation / GateFailure / PolicyError) are type-disjoint (compile-enforced).
7. The only escape is a policy-PR (no inline bypass).
8. Moon tasks `:gate` and `:deploy-gate` are wired; `moon ci` runs the canonical gate.
9. Xtask's own source passes its own gate (dogfooded).
10. Killer demo: AI writes Rust with a `for` loop + `.unwrap()` → `xtask gate` rejects with typed `UseIteratorPipeline` + `HOLZMAN_PANIC_UNWRAP` hints → AI fixes to iterator pipeline + `match` → gate passes → certificate emitted → deploy-gate accepts.

---

## 17. References (read in full)

- `holzman-rust/SKILL.md`
- `holzman-rust/references/nasa-jpl-standards.md` — Power of Ten + PLUS mapped to Rust
- `holzman-rust/references/latency-throughput-playbook.md` — workload/storage/allocation discipline
- `holzman-rust/references/runtime-performance-architecture.md` — prove-slow/execute-fast, dense IR, bounded runtime
- `holzman-rust/references/zero-cost-abstractions.md` — allocation/dispatch/layout cost ledger
- `holzman-rust/references/simd-patterns.md` — safe SIMD + unsafe-waiver rejection
- `holzman-rust/references/mechanical-empathy-toolchain.md` — second-ring evidence lanes
- `functional-rust/SKILL.md`
- `functional-rust/references/scott-ddd-types.md` — type-driven DDD, parse-don't-validate
- `functional-rust/references/typing-refactor-checklist.md` — no-loops/nesting/error discipline checks
- `functional-rust/references/complete-workflow.md` — Data→Calc→Actions worked example
- `moon-v2/SKILL.md` — canonical `moon ci` gate, source-lint zero-tolerance
