# Xtask: The Verified Rust CI Toolchain

> **Vision v1.0** — the grand ambition
> Companion: [`v1-spec.md`](./v1-spec.md) (the concrete, buildable v1 contract)

---

## 0. Vision Statement

Xtask is the verified Rust CI toolchain for teams using Moon. It replaces
bash-in-YAML pipelines with typed Rust lanes, enforces strict coding standards
by default, and accumulates formal verification evidence in batches — from
"the code has the right shape" to "the code is mathematically proven correct."

Xtask is opinionated, not configurable. Its `strict-ai` policy is the default
and the only profile. If you don't want strict Rust, don't use xtask.

---

## 1. The Problem

Rust CI in 2026 is bash-in-YAML. Every team reinvents the same pipeline:

```yaml
- run: cargo fmt --check
- run: cargo clippy -- -D warnings
- run: cargo test
- run: cargo audit
```

This has five structural failures:

1. **Untyped contracts.** Steps communicate via exit codes and stdout. No
   structured findings, no schema, no machine-readable evidence. Reading CI
   logs is an act of archaeology.

2. **No verification beyond "the linter didn't complain."** Clippy catches
   patterns; it does not prove panic-freedom, does not prove functional
   correctness, does not prove type invariants. Real Rust verification tools
   (Kani, Flux, Verus) exist but no CI toolchain integrates them.

3. **No batched evidence strategy.** Every pipeline runs everything or
   nothing. There is no concept of "this PR proves shape; this merge proves
   panic-freedom; this release proves correctness." Evidence is binary:
   green or red.

4. **No reproducibility.** A green CI run produces a checkmark. It does not
   produce a structured record of *what was proven, by which tool, against
   which inputs, under which policy.* Two runs of the same code on different
   machines may produce different results with no way to explain why.

5. **No strict-opinion layer.** Teams can configure clippy however they want,
   which means most teams configure it weakly. There is no canonical "this is
   what strict Rust looks like" policy that teams can adopt whole.

Every Rust team solves these problems independently. The solutions are
incomplete, inconsistent, and unmaintainable. Xtask exists to solve them once.

---

## 2. The Grand Vision

Xtask becomes the canonical quality pipeline for Rust projects using Moon.

**Typed lanes replace shell scripts.** Every CI step is a Rust subcommand
with typed inputs (files, env, dependencies) and typed outputs (structured
findings JSON, schema-versioned). No more parsing stdout with regex.

**Strict standards replace configuration.** The `strict-ai` policy is the
only profile. It forbids unsafe, unwrap/expect, panic macros, unchecked
indexing, unchecked arithmetic, imperative loops, excessive nesting, and
unapproved lint suppressions. Teams adopt the policy or they don't use xtask.

**Verification batches replace binary gates.** Each scope tier is a
commitment about evidence:

| Batch | Trigger | What it proves | Tools |
|---|---|---|---|
| `edit` | every save | code has the right shape | fmt, clippy, ast-grep, dylint, panic-scan |
| `prepush` | before push | code is shippable | + tests, cargo-deny |
| `full` | on PR | code cannot panic | + Kani (panic-freedom proofs) |
| `verify` | on merge to main | code is functionally correct | + Flux (refinement types), Verus (specs) |
| `release` | on tag | release artifact is reproducible | + release build with Receipt |

Each batch produces a `QualityReceipt` — a structured record of what was
proven, by which tool, against which source/policy/toolchain digests. The
Receipt is reproducible, content-addressed, and machine-readable.

**Moon replaces ad-hoc orchestration.** Moon's task graph handles parallelism,
caching (sccache + bazel-remote + Cargo), affected-file detection, and CI
integration. xtask defines the policy and the lanes; Moon runs them.

---

## 3. Verification Batches

The scope-tier-as-batching-strategy is xtask's core insight. Each batch is a
*stage of evidence accumulation* — a structured claim about what has been
proven at each point in the development lifecycle.

### Batch 1: Shape (edit scope)

**Claim:** "The code conforms to strict-ai structural and style rules."

**Evidence:** formatting passes, compilation succeeds, clippy passes with
`-F` critical lints, ast-grep structural rules pass, dylint type-aware rules
pass, no production panic/assert macros.

**Cost:** seconds to tens of seconds. Runs on every save.

### Batch 2: Shippable (prepush scope)

**Claim:** "The code is ready for review — tests pass, dependencies are clean."

**Evidence:** all of Shape + test suite passes (single-threaded, deterministic
config), cargo-deny reports no advisories/license/ban/source violations.

**Cost:** minutes. Runs before push.

### Batch 3: Cannot Panic (full scope, post-v1)

**Claim:** "For all inputs within proven bounds, the code cannot panic."

**Evidence:** all of Shippable + Kani bounded model-checking harnesses prove
panic-freedom on critical paths (parsing, dispatch, arithmetic, indexing).

**Cost:** tens of minutes. Runs on PR.

### Batch 4: Correct by Construction (verify scope, post-v1)

**Claim:** "The code satisfies its formal specification."

**Evidence:** all of Cannot Panic + Flux refinement types prove type-level
invariants (non-empty, sorted, length-indexed) + Verus specs prove functional
correctness of pure domain logic.

**Cost:** minutes to hours. Runs on merge to main.

### Batch 5: Reproducible Release (release scope)

**Claim:** "The release artifact is bit-reproducible from the Receipt."

**Evidence:** all of Correct + release build succeeds + Receipt records
source/lock/policy/toolchain digests sufficient to reproduce the artifact.

**Cost:** full pipeline. Runs on tag.

---

## 4. Architecture Principles

### Moon-native

Moon is the orchestration substrate. xtask does not build its own scheduler,
cache, or affected-file detector. Each lane is a Moon task; each composite
gate is a Moon task with `deps:` on its lanes. Distribution assumes Moon is
installed.

### Single binary + dylint library

xtask ships as one binary (CLI, lane runners, aggregator, doctor, explain,
policy, digests, serialization) plus a co-located dylint dynamic library
for type-aware lint scans. Distribution via `cargo-binstall` installs both.
The dylint library cannot be statically linked (it loads into the clippy
driver process).

### ast-grep embedded

Structural rules (no loops, nesting depth, no `Result<T, String>`, bypass
attribute presence) run via `ast-grep-core` embedded as a Rust dependency.
Rules ship embedded in the binary via `include_str!`. No external ast-grep
binary required.

### Strict-opinion, not configurable

The `strict-ai` policy is the only profile. It is embedded as defaults in
the binary and overridable only via checked-in policy files (which themselves
require CODEOWNER approval to change). No per-site bypass. No lenient mode.

### Typed lanes with typed contracts

Every lane has:
- Typed input contract (files, env, dependencies)
- Typed output contract (`.xtask/out/<lane>.json`, schema-versioned)
- Typed execution contract (timeout, resource limits)
- Typed evidence contract (lane receipt with command, version, digest)

### Fail-closed

Missing tool = `InputError`. Ambiguous policy = `PolicyError`. Tool failure =
`LaneFailure`. No silent warnings. No "best effort." Either the gate proves
its claim or it rejects.

---

## 5. The Competitive Landscape

| Tool | Typed lanes | Strict Rust policy | Verification batches | Moon-native |
|---|---|---|---|---|
| GitHub Actions | no (strings) | no | no | no |
| Dagger | yes (Go/TS/Python) | no | no | no |
| Bazel / Buck2 | yes (rules) | no | no | no |
| Moon alone | yes (tasks) | no | no | — |
| cargo-* point tools | N/A | N/A | N/A | no |
| **xtask** | **yes (Rust)** | **yes (strict-ai)** | **yes (5 tiers)** | **yes** |

### Dagger: complement, not competitor

Dagger is a general typed CI engine. xtask is a Rust-specific quality layer.
They compose: a team could run xtask inside a Dagger pipeline for non-Rust
work. The narrow overlap ("typed CI for Rust") is where they appear to
compete, but xtask's strict-ai policy and verification batches have no Dagger
equivalent.

### Bazel / Buck2: different game

Bazel and Buck2 are build systems with typed rules. They solve build
reproducibility and hermeticity. They do not enforce coding standards, do not
run clippy with strict lints, do not integrate Kani/Flux/Verus. Different
category.

### Moon alone: substrate, not policy

Moon provides the orchestration layer xtask builds on. Without xtask, Moon
users still write their own cargo/clippy/deny task definitions. xtask is the
opinionated Rust policy layer that turns Moon into a verified Rust quality
gate.

---

## 6. The Strategic Moat

Two things make xtask hard to replicate:

### 1. The verification batch stack

Each batch (Kani, Flux, Verus) is a multi-week integration project requiring
deep expertise in the verifier, the solver, and the Rust compiler internals.
Once built, the batches compose — Kani panic-freedom + Flux refinement + Verus
correctness form a stack that is strictly stronger than any individual tool.

A competitor would need to integrate all three verifiers, plus dylint, plus
ast-grep, plus cargo-deny, plus the strict-ai policy, plus the Moon task
graph. That is a multi-year effort to match what xtask accumulates version
by version.

### 2. The strict-ai policy

The policy is curated. Every lint, every threshold, every `-F` flag, every
clippy restriction is chosen for a reason and tested against real code. A
competitor can copy the Cargo.toml `[workspace.lints]` section but cannot
copy the judgment behind it without doing the work.

Together: the verification stack is hard to build, and the policy is hard to
curate. Both compound over time.

---

## 7. Evolution Timeline

Not dates. Sequence. Each version ships something usable on its own.

### v1.0 — Typed CI with strict-ai (the foundation)

Moon-native, single binary, ast-grep + dylint, cargo built-ins, cargo-deny.
Strict-opinion enforcement. No formal verification yet. This alone is a
better Rust CI than 99% of repos have.

### v1.5 — Kani panic-freedom

Add Kani to the `full` scope. Every merge proves that critical paths (parsing,
dispatch, arithmetic) cannot panic within proven bounds. Replaces lint
heuristics with proofs on the panic-discipline surface.

### v2.0 — Flux refinement types

Add Flux to the `full` scope. Domain newtypes carry refinement invariants
(`WorkspacePath{v: !v.is_empty()}`, `TextRange{v: v.start <= v.end}`).
Runtime validation replaced by type-level proof.

### v2.5 — Verus functional correctness

Add Verus and the `verify` scope tier. Pure domain logic carries `spec fn`
and `proof fn` artifacts. The Receipt records which functions are formally
verified. Merges to main require verification.

### v3.0+ — Team scale and beyond

- Remote cache (bazel-remote shared across machines)
- Affected-file-driven incremental gates
- Persistent Receipt ledger (`xtask-ledger` SQLite crate)
- Shuttle concurrency verification (when lane parallelization lands)
- Antithesis deterministic simulation (when stateful components land)
- Multiple policy profiles (if demand exists — strict-ai may remain the only one)

---

## 8. What Xtask Is Not

- **Not a sandbox.** Xtask runs Cargo and other developer tools, which may
  execute repository code. Only run on repositories you trust to build.
- **Not a security boundary.** No signing, no attestation, no deploy-gate,
  no artifact trust. CI runs `xtask gate --scope <scope>` and checks exit code.
- **Not a formal proof system in v1.** Verification batches land starting
  v1.5. v1 enforces structure and style, not correctness.
- **Not a replacement for cargo.** Xtask orchestrates cargo; it does not
  replace cargo workflows.
- **Not for non-Moon users.** Moon is a required dependency. If you don't
  use Moon, xtask is not for you.
- **Not configurable.** The strict-ai policy is the only profile. Escape is
  a policy PR with CODEOWNER approval, not a CLI flag.
- **Not a general CI engine.** xtask is Rust-specific. Other languages are
  out of scope.

---

## 9. Success Criteria

Xtask has succeeded when:

1. **Every PR in an adopting repo carries structured findings** — no more
   reading CI logs to understand what failed.
2. **Every merge to main carries panic-freedom proofs** (post-v1.5) — Kani
   harnesses prove critical paths cannot panic.
3. **Every release carries a reproducible Receipt** — source/lock/policy/
   toolchain digests sufficient to reproduce the evidence on any machine.
4. **The strict-ai policy is the reference standard** — teams point to xtask's
   `[workspace.lints]` as "this is what strict Rust looks like."
5. **The verification batch stack is the defensible niche** — no other Rust
   CI tool integrates Kani + Flux + Verus. xtask owns that category.
6. **Moon ecosystem recognizes xtask as the Rust policy layer** — listed in
   Moon's ecosystem docs, referenced as the canonical Rust quality integration.

---

## 10. References

- [`v1-spec.md`](./v1-spec.md) — the concrete, buildable v1 contract
- [`architecture-spec.md`](./architecture-spec.md) — v6.0 historical spec
  (superseded by v1-spec.md for implementation purposes)
- Moon v2 skill — `.agents/skills/moon-v2/SKILL.md`
- Holzman Rust skill — `.agents/skills/holzman-rust/SKILL.md`
- Functional Rust skill — `.agents/skills/functional-rust/SKILL.md`
