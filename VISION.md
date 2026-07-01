# Titania-Check: The High-Assurance Rust CI Toolchain

> **Vision v2.1** — JPL Power of Ten + Haskell + Gleam, expressed as Rust
> Companion: [`v1-spec.md`](./v1-spec.md) (the concrete, buildable v1 contract)
>
> **Tool name:** `titania-check` (binary) · `.titania/` (config) · `titania-*` (crates)
>
> Moon is non-negotiable: **MOON CI/CD is the absolute foundation for all of this work.**

---

## 0. Vision Statement

Titania-check is the high-assurance Rust CI toolchain for teams using Moon.
Moon CI/CD is the absolute foundation: every typed Rust lane is designed to run
inside Moon's task graph before any agent, hook, or CI provider claims success.
Titania replaces bash-in-YAML pipelines with typed Rust lanes, enforces strict
coding standards by default, and accumulates formal verification evidence in
batches — from "the code has the right shape" to "the code is mathematically
proven correct."

Titania-check is opinionated, not configurable. Its `strict-ai` policy is the
default and only profile. A `strict-critical-rust` profile for medical/aerospace
use is a **future direction** (v3.0+) — see §2.3. If you don't want strict Rust,
don't use titania-check.

For actual medical (IEC 62304 Class C) or aerospace (NASA-STD-8739.8)
certification, tooling is one piece of a lifecycle process. Titania-check
provides the technical foundation. The certification itself requires the full
IV&V lifecycle.

---

## 1. Moon CI/CD Is the Foundation

Rust CI in 2026 is bash-in-YAML. Every team reinvents the same pipeline:

```yaml
- run: cargo fmt --check
- run: cargo clippy -- -D warnings
- run: cargo test
- run: cargo audit
```

Six structural failures:

1. **Untyped contracts.** Exit codes and stdout. No structured findings.
2. **No verification beyond "the linter didn't complain."**
3. **No batched evidence strategy.** Green or red, nothing in between.
4. **No reproducibility.** A checkmark, not a structured record.
5. **No strict-opinion layer.** No canonical "strict Rust" policy.
6. **No architectural enforcement.** No crate/import/capability rules.

---

## 2. The Doctrine

### 2.1 JPL Power of Ten — Rust translation

```
No panic surface.           — unwrap/expect/panic/todo/unreachable banned
No unchecked absence.       — Option/Result handled explicitly
No first-party unsafe.      — unsafe_code = forbid
No architectural drift.     — crate graph + import graph + capability boundaries
No dependency drift.        — lockfile pinned, supply chain scanned
No implicit effects.        — I/O behind traits, no ambient global state in core
No hidden allocation.       — critical paths allocation-free after init (future)
No wildcard handling.       — no `_ => ...` unless externally non-exhaustive
No stringly typed errors.   — no Result<T, String>
No unreviewed features.     — feature powerset checked
No warnings.                — zero from compiler AND static analysis
No unowned exceptions.      — every suppression has owner + reason + expiry
```

### 2.2 Haskell / Gleam influence

```
Pure core, explicit effects, algebraic data types, exhaustive handling,
typed errors, small modules, no ambient global state, boring functions.
```

Architecture shape:
```
core/       pure domain, typed errors, no async runtime, no I/O
ports/      traits and capability interfaces
adapters/   filesystem, network, database, clock, random
app/        orchestration
bin/        CLI / process boundary
```

### 2.3 `strict-critical-rust` profile — FUTURE DIRECTION (v3.0+)

**Not specified in v1.** The following table is aspirational, not contractual.
Each rule requires a concrete detection mechanism before it can be enforced.
When specified, this profile will add JPL allocation constraints, Ferrocene
toolchain support, and mandatory formal verification on critical modules.

| Rule | `strict-ai` (v1) | `strict-critical-rust` (future) |
|---|---|---|
| Panic surface | banned | banned |
| Loops | banned (functional style) | banned unless bounded-loop attribute |
| Allocation | normal | no allocation after init (detection mechanism TBD) |
| Architecture enforcement | reject on drift | reject on drift |
| Toolchain | pinned stable | Ferrocene or qualified |
| Verification | optional | Kani required on critical modules |

The `#[titania_bounded_loop(max=N)]` attribute does not exist yet. It will
require a proc-macro crate. "Critical modules" is undefined until a marker
attribute or path convention is specified. These are v3.0 design problems.

---

## 3. The Grand Vision

Titania-check becomes the canonical quality pipeline for Rust projects using Moon.

- **Moon replaces ad-hoc orchestration.**
- **Typed lanes replace shell scripts.**
- **Strict standards replace configuration.**
- **Verification batches replace binary gates.**
- **Architecture enforcement replaces decay.**

---

## 4. Pipeline Stages and Verification Batches

### Stage overview

| Stage | v1? | Purpose | Wall time | Trigger |
|---|---|---|---|---|
| `edit` | ✅ | Fast quality — shape, style, structure, architecture imports | seconds | every save |
| `prepush` | ✅ | Full quality — tests, supply chain | minutes | before push |
| `release` | ✅ | Evidence — reproducible build | tens of minutes | on tag |
| `full` | v1.5 | Resistance — Kani, cargo-mutants, coverage, API drift | tens of minutes | on PR |
| `deep` | v2.5 | Assurance — Miri, fuzz, sanitizers, concurrency | hours | nightly / merge |

**v1 ships 3 stages.** `full` and `deep` are sequenced for post-v1 releases.
See v1-spec.md §16 for the deferred roadmap.

### Batch contents (final-state vision)

**`edit`** (v1): fmt, compile, clippy, ast-grep (structural + architecture + bypass),
dylint, panic-scan, policy-scan

**`prepush`** (v1): + test, cargo-deny

**`release`** (v1): + release build

**`full`** (v1.5): + Kani, cargo-mutants, cargo-llvm-cov, cargo-public-api, cargo-msrv

**`deep`** (v2.5): + Miri, sanitizers, cargo-fuzz, Verus, Loom/Shuttle

Each batch produces a `QualityReceipt`.

---

## 5. The Full Toolchain Map (Final-State Vision)

### Tier 0 — Toolchain and Execution Control
Ferrocene (regulated), pinned stable Rust, `--frozen` everywhere, tool pinning
(resolved path, version, SHA-256, argv, exit code, output hashes).

### Tier 1 — Formatting, Compilation, and Linting
rustfmt, cargo check, Clippy (`--lib --bins` only, NOT `--all-targets`).

### Tier 2 — ast-grep Structural Rules (PRIMARY house-rule engine)
Embedded via `ast-grep-core`. Rules: panic_surface, unsafe_surface, loop_surface,
effect_boundary, architecture_imports, typed_error_policy, match_exhaustiveness,
test_nondeterminism, lint_suppression, macro_policy.

### Tier 3 — JPL + Haskell + Gleam Rust Rules
Panic-surface discipline, typed absence, exhaustive handling, functional core
(no loops), allocation discipline (future), unsafe discipline, architecture doctrine.

### Tier 4 — Security SAST
CodeQL (primary), Semgrep (optional), SonarQube (dashboard only).

### Tier 5 — Supply-Chain Security
cargo-audit, cargo-deny (v1), cargo-vet, cargo-geiger, cargo-machete, cargo-udeps.

### Tier 6 — Dependency and Architecture Drift
cargo_metadata, guppy, cargo-hack, cargo-public-api, cargo-semver-checks, cargo-msrv.

### Tier 7 — Testing
cargo test / cargo-nextest, test nondeterminism scanner.

### Tier 8 — Coverage and Mutation Resistance
cargo-llvm-cov, cargo-mutants.

### Tier 9 — Fuzzing and Property Testing
cargo-fuzz, cargo-afl, Bolero.

### Tier 10 — UB, Unsafe, and Runtime Bug Detection
Miri, sanitizers (ASan, TSan, LSan), cargo-careful.

### Tier 11 — Concurrency Testing
Loom (exhaustive), Shuttle (randomized).

### Tier 12 — Formal Verification
Kani (v1.5), Flux (v2.0), Verus (v2.5), Prusti (experimental), MIRAI (watch).

### Tier 13 — SBOM, Binary Auditability, and Release Evidence
cargo-auditable, cargo-cyclonedx, Syft, Grype, OSV-Scanner, OpenSSF Scorecard.

---

## 6. Architecture Drift Enforcement

Three layers (crate graph lands in v2.0; import graph + capability boundaries in v1):

### 6.1 Crate graph (v2.0 — guppy)
```toml
[layer.core]
must_not_depend_on = ["tokio", "axum", "sqlx", "reqwest"]
```

### 6.2 Module import graph (v1 — ast-grep)
```
core must not import tokio, std::fs, std::time::SystemTime, rand::thread_rng
```

### 6.3 Capability boundaries (v1 — ast-grep + policy)
Ban direct effect APIs in core: `SystemTime::now`, `Instant::now`, `std::fs`,
`std::env`, `std::net`, `rand::thread_rng`, `tokio::spawn`.

---

## 7. Architecture Principles

- **Moon-native** — Moon handles scheduling, caching, affected-detection.
- **Single binary + dylint library** — co-located, loaded via `[workspace.metadata.dylint]`.
- **ast-grep embedded** — rules via `include_str!`, no external binary.
- **Strict-opinion** — policy is the product. Escape is a policy PR.
- **Typed lanes** — typed input, output, execution, evidence contracts.
- **Fail-closed** — missing tool = InputError. No silent warnings.
- **Tool pinning** — resolved path, version, SHA-256 before+after.

---

## 8. The Exception Schema

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

---

## 9. What Titania-Check Enforces Directly

```
tool pin verification, policy digest verification, source/Cargo.lock/policy
digests, exception ownership + expiry, architecture import graph, forbidden
import graph, no inline suppressions, TOML/env bypass detection.
```

---

## 10. The Competitive Landscape

| Tool | Typed contracts | Strict Rust policy | Verification batches | Architecture enforcement | Moon-native |
|---|---|---|---|---|---|
| GitHub Actions | no | no | no | no | no |
| Dagger | yes (Go/TS) | no | no | no | no |
| Bazel / Buck2 | yes (rules) | no | no | partial (aspects) | no |
| Moon (alone) | yes (tasks) | no | no | no | — |
| cargo-* tools | N/A | N/A | N/A | no | no |
| **titania-check** | **yes (Rust)** | **yes (strict-ai)** | **yes (5 stages)** | **yes (3 layers)** | **yes** |

- **Dagger**: complement, not competitor. General typed CI vs Rust-specific quality.
- **Bazel/Buck2**: build systems. No coding-standard enforcement or verification.
- **Moon (alone)**: orchestration substrate. Titania-check is the policy layer on top.

---

## 11. The Strategic Moat

1. **Verification stack** — Kani + Flux + Verus + Miri + Loom + cargo-fuzz. Multi-year to replicate.
2. **Policy curation** — every lint tested against real code. Judgment can't be copied.
3. **Architecture enforcement** — 3-layer drift detection. No other Rust CI does this.
4. **Evidence trail** — auditable Receipt, not a green checkmark.

---

## 12. Evolution Timeline

Sequence, not dates. Each version ships something usable.

### v1.0 — Typed CI with strict-ai (the foundation)
Moon-native, single binary, ast-grep + dylint, cargo built-ins, cargo-deny,
policy-scan. Architecture import scan (ast-grep). Strict-opinion enforcement.
3 scope tiers (edit/prepush/release). No formal verification.

### v1.5 — Kani panic-freedom + cargo-mutants
`GateScope::Full` unlocked. Kani harnesses on critical paths. Cargo-mutants
with baseline. Coverage (cargo-llvm-cov). API drift (cargo-public-api).

### v2.0 — Flux refinement types + crate-graph enforcement
Flux annotations on domain newtypes. Guppy-based crate-graph architecture rules.

### v2.5 — Verus functional correctness + `deep` scope
`GateScope::Deep` unlocked. Verus specs on pure domain logic. Miri, sanitizers,
cargo-fuzz. Loom/Shuttle concurrency tests.

### v3.0 — Safety-critical profile + release evidence
`strict-critical-rust` profile specified concretely. Ferrocene support.
cargo-auditable, cargo-cyclonedx, Syft, OSV-Scanner. SBOM + audit trail.

### v3.5+ — Team scale
Remote cache, affected-driven incremental gates, persistent Receipt ledger,
Antithesis, cargo-vet, CodeQL, multiple profiles.

---

## 13. The Strongest Recommended Stack (Final State)

```
Compiler:    Ferrocene (regulated) or pinned stable Rust
Core:        Cargo, rustfmt, Clippy, cargo_metadata
Structural:  ast-grep (embedded) + dylint (type-aware)
Architecture: guppy + cargo_metadata + ast-grep import rules
SAST:        CodeQL (+ optional Semgrep)
Supply chain: cargo-audit, cargo-deny, cargo-vet, cargo-geiger, cargo-machete,
              cargo-cyclonedx, cargo-auditable, OSV-Scanner
Testing:     cargo-nextest, cargo-llvm-cov, cargo-mutants
Drift:       cargo-hack, cargo-public-api, cargo-semver-checks, cargo-msrv
Deep:        Miri, sanitizers, cargo-careful, cargo-fuzz (+ Bolero),
              Kani, Verus, Loom + Shuttle
Release:     cargo-auditable, cargo-cyclonedx, Syft, Grype, OSV-Scanner,
              cargo-semver-checks, OpenSSF Scorecard
```

---

## 14. What Titania-Check Is Not

- Not a sandbox. Runs Cargo, which may execute repo code.
- Not a security boundary. No signing, no deploy-gate.
- Not a formal proof system in v1. Verification batches start v1.5.
- Not a replacement for cargo.
- Not for non-Moon users.
- Not configurable. Policy is the product.
- Not a general CI engine. Rust-specific.
- Not a certification. Technical evidence only; IV&V lifecycle is separate.

---

## 15. Success Criteria

1. Every PR carries structured findings — no more reading CI logs.
2. Every merge carries panic-freedom proofs (post-v1.5).
3. Every release carries a reproducible Receipt + SBOM.
4. The strict-ai policy is the reference standard for strict Rust.
5. The verification stack is the defensible niche — no competitor integrates
   Kani + Verus + Miri + Loom + cargo-fuzz.
6. Architecture drift is mechanically impossible.
7. Safety-critical Rust teams adopt `strict-critical-rust` (post-v3.0).
8. Moon ecosystem recognizes Titania as the Rust policy layer.

---

## 16. References

### Standards
- [NASA-STD-8739.8](https://standards.nasa.gov/standard/NASA/NASA-STD-87398)
- [JPL Power of Ten](https://spinroot.com/gerard/pdf/P10.pdf)
- IEC 62304, ISO 26262, IEC 61508

### Companion documents
- [`v1-spec.md`](./v1-spec.md) — the concrete, buildable v1 contract

### Key tools
- [Moon](https://moonrepo.dev/) · [ast-grep](https://ast-grep.github.io/) ·
  [dylint](https://github.com/trailofbits/dylint) · [Kani](https://github.com/model-checking/kani) ·
  [Verus](https://github.com/verus-lang/verus) · [cargo-deny](https://docs.rs/crate/cargo-deny/latest) ·
  [cargo-mutants](https://mutants.rs/) · [cargo-nextest](https://nexte.st/) ·
  [Miri](https://github.com/rust-lang/miri) · [Loom](https://github.com/tokio-rs/loom) ·
  [guppy](https://crates.io/crates/guppy) · [CodeQL](https://docs.github.com/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning-with-codeql)

### Skills
- Moon v2, Holzman Rust, Functional Rust — `.agents/skills/`
