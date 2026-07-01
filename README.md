# Titania

**A Moonrepo-powered Rust QA gate for AI-assisted code.**

> Moon is non-negotiable. **MOON CI/CD is the absolute foundation for all of this work.**
> AI writes code fast. Titania makes it prove it didn't hallucinate the basics.

Typed evidence. Strict policy. Fewer AI faceplants.

For the product thesis behind the Rust-only scope, see
[`WHY_RUST_ONLY.md`](./WHY_RUST_ONLY.md): **AI made code cheap. Rust makes it
cheaper to trust.**

---

Titania is a **Moonrepo-powered** Rust QA gate that runs locally and in CI.
Moon orchestrates every lane; Titania supplies the typed Rust checks Moon runs.
It shells out to proven tools (`cargo`, `clippy`, `ast-grep`, `dylint`,
`cargo-deny`), normalizes failures into typed findings, and emits
reproducible receipts — so humans and AI agents repair code against
deterministic feedback instead of vague log soup.

## What Titania Is

A **deterministic CLI and CI/CD gate first**. Agents and IDEs can invoke
it, but Titania itself is not a background copilot.

The goal is not to make AI "smarter" by prompting harder. The goal is to
make bad AI output **mechanically obvious** — every panic surface, every
`unwrap()` on a happy-path lie, every unchecked index, every stringly-typed
error becomes a typed finding with an exact file:line and a deterministic
policy citation.

No prompt magic. No "the linter didn't complain." Just structured evidence
your team can review, gate on, and accumulate over time.

## What Titania Is Not

- **Not a linter.** It's an aggregator over `clippy`, `ast-grep`, `dylint`,
  and `ripgrep`, with typed findings and policy enforcement. The lints
  live upstream; Titania owns the opinion layer and the evidence shape.
- **Not configurable.** `strict-ai` is the policy. Opt out per-line with
  owner + reason + expiry, not by negotiating config.
- **Not a background agent.** It does not watch your IDE, your files, or
  your prompts. It runs on demand and emits a typed artifact you can
  review, gate on, and accumulate. A future `titania watch` is sugar, not
  v1 core.
- **Not a CI replacement.** It runs **inside** Moon, which is the
  orchestrator. CI systems (GitHub Actions, GitLab CI, Buildkite) call
  Moon, which calls `titania`.
- **Not a YAML pipeline.** It replaces bash-in-YAML with typed Rust
  lanes that emit structured JSON, run in a DAG, and cache by content
  hash.

## Quick Start

```bash
# Initialize a workspace (writes .titania/ config + Moon task wiring)
titania init

# Verify the install
titania doctor

# Edit-time feedback (~seconds, the inner loop)
titania ci --scope edit

# Pre-push gate (~minutes, the PR expectation)
titania ci --scope prepush

# Full evidence sweep (~tens of minutes, on PR)
titania ci --scope full

# Diagnose a finding
titania explain vb-fmt-0012
```

Git hooks and CI call the same binary with the same scope. The
`prepush` scope is what your CI runs. `full` is what runs on PRs.

## What Titania Catches

The boring, high-leverage things LLMs get wrong in Rust:

| LLM tell-tale | How Titania catches it |
|---|---|
| `unwrap()` / `expect()` in production paths | `panic-scan` lane: ripgrep with parser prefilter, blocks production `assert!` / `unreachable!` / `panic!` / `todo!` / `unimplemented!` |
| Unchecked indexing (`x[i]`, `&s[0..n]`) | `clippy::indexing_slicing` + `clippy::string_slice` denied at source level |
| `Result<T, String>` (stringly-typed errors) | Domain policy rejects; only `thiserror`-typed errors allowed |
| Lossy `as` casts and arithmetic side effects | `clippy::as_conversions` + `clippy::arithmetic_side_effects` denied |
| Hidden I/O in pure functions | Layered architecture enforcement: core crate has zero async deps, I/O behind traits |
| First-party `unsafe` | `forbid(unsafe_code)` workspace-wide; `cargo geiger` blocks CI on detection |
| Unaudited dependencies | `cargo deny` (licenses, bans, advisories, sources, dupes) + `cargo vet` |
| Silent feature drift | `cargo hack check --feature-powerset` builds every combination |
| Architectural drift | `ast-grep` structural rules + `dylint` type-aware context for crate/import boundaries |
| Policy bypasses | `policy-scan` rejects `.cargo/config.toml` overrides, `[lints]` weakening, env violations |

Every finding is a typed `Report → Finding → { file, line, rule, severity }`
record, not a stdout line. Aggregable, diffable, gateable.

## The Doctrine: `strict-ai`

Titania is **opinionated, not configurable**. The `strict-ai` policy is
the default and only profile. There is no `titania.toml` to negotiate.
You don't write your own rules; you opt in to the opinion.

> If you don't want strict Rust, don't use Titania.

- No `unwrap` / `expect` / `panic` / `todo` / `unimplemented` in production
- No first-party `unsafe` (workspace `forbid(unsafe_code)`)
- No unchecked indexing, slicing, casts, arithmetic
- No `Result<T, String>` — typed errors only
- No ambient global state, no I/O in core, no async in core
- No architectural drift — crate/import/capability boundaries enforced
- No dependency drift — lockfile pinned, supply chain scanned
- No unowned exceptions — every suppression needs owner + reason + expiry
- No warnings — zero from compiler AND static analysis

`strict-critical-rust` (medical / aerospace profile) is a **future direction
for v3.0+**, not specified yet.

## How It Runs (Moon, Not YAML)

Titania lives inside **[Moonrepo](https://moonrepo.dev)** — the polyglot
build-system and task orchestrator, not the celestial body. We are not
invoking moonbeams; we are using a deterministic build graph to make QA
evidence reproducible, cacheable, and reviewable.

**Moon** runs the DAG. **Titania** owns the Rust policy, typed findings,
and evidence receipts.

Rust CI in 2026 is still mostly:

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

Titania replaces each:

| bash-in-YAML | Titania lane | Output |
|---|---|---|
| `cargo fmt --check` | `fmt` | `FmtFinding[]` per file |
| `cargo clippy` | `clippy` (with strict policy) | `ClippyFinding[]` per file:line |
| `cargo test` | `test` | `TestReport` per binary |
| `cargo audit` | `cargo-deny` | `AdvisoryFinding[]` |
| (manual review) | `panic-scan` | `PanicFinding[]` per file:line |
| (none) | `policy-scan` | `PolicyViolation[]` per file:line |
| (none) | `ast-grep` | `StructuralFinding[]` per file:line |
| (none) | `dylint` | `TypeAwareFinding[]` (type-resolved) |
| (none) | `aggregate` | `Report` + `QualityReceipt` (4 digests) |

The lanes are **Moon tasks**, declared in `.moon/tasks/all.yml`. They run
in a fixed DAG. They cache by content hash. They emit typed artifacts.
The aggregator walks the DAG, collects findings, hashes them, and writes
the receipt. A `moon ci` rerun is byte-identical to a previous green
because the input hash is in the receipt.

## Verification Batches

Titania is structured so that **batches of evidence** accumulate over
releases, not as binary gates. v1 ships structural/style evidence;
later versions add deeper layers without breaking earlier ones.

| Scope | Stages | Time | When |
|---|---|---|---|
| `edit` (v1) | shape, style, structure, architecture imports | seconds | every save |
| `prepush` (v1) | + tests, supply chain | minutes | before push / in CI |
| `release` (v1) | + reproducible build | tens of minutes | on tag |
| `full` (v1.5) | + Kani, cargo-mutants, coverage, API drift, MSRV | tens of minutes | on PR |
| `deep` (v2.5) | + Miri, sanitizers, cargo-fuzz, Verus, Loom | hours | nightly / merge |

**v1 ships `edit`, `prepush`, `release`.** `full` and `deep` are
sequenced for later releases; they require stable, scoped evidence
contracts before they can ship as gates.

## FAQ

### Is Titania a CLI, CI tool, or background agent?

Titania is a **CLI-first QA gate** that runs locally and in CI.

Developers and AI agents run it directly before commits. CI runs the
same gate remotely. Titania does not need to watch your editor or act
as a background copilot to be useful. The core value is **deterministic
feedback**: same policy, same Moon task graph, same typed findings, same
evidence receipts.

Background watching may come later (`titania watch`), but v1 is explicit,
reproducible, and scriptable.

### Why Moonrepo and not "just Cargo"?

Cargo runs builds. Moonrepo runs a **typed task graph** with content
hashing, remote caching, and DAG-aware parallel execution. Titania
needs:

- Reproducible receipts (the input hash must be in the output).
- Affected-only execution (only re-run the lanes the change touched).
- A single execution surface for local + CI (same code path, same result).

Cargo aliases can't do any of that. Moon can.

### Why not just `cargo clippy -- -D warnings`?

You can. Titania is what you get when you want **typed, evidence-bearing,
policy-cited** failures instead of exit codes, and you want the same
gate to run locally, in pre-commit, in pre-push, in CI, and in your
AI agent's tool harness.

### Is Titania for individual developers or teams?

Both. Individual devs use `titania ci --scope edit` in the inner loop.
Teams use `--scope prepush` in CI and `--scope full` on PRs. The same
binary, the same policy, the same receipts.

## Install

```bash
# Coming with v1.0 release.
cargo install titania

# Or with cargo-binstall (faster, prebuilt binaries).
cargo binstall titania
```

For pre-v1 development, see [`v1-spec.md`](./v1-spec.md) for the buildable
contract and `cargo generate titania/template` for workspace adoption.

## Crate Layout

```
titania          CLI binary (init, doctor, ci, explain)
titania-core     Domain types: Report, Finding, Lane, Receipt, primitives
titania-policy   strict-ai policy loading, validation, digest, exceptions
titania-lanes    Lane runner implementations
titania-dylint   dylint library (loaded by clippy driver, type-aware context)
titania-output   Report JSON serialization, doctor, explain
titania-aggregate Reads lane outputs, assembles Report, computes Receipt
```

`titania-dylint` is a `cdylib` (compiles to `.so`/`.dylib`/`.dll`); it
runs inside the clippy driver and adds type-aware context to the
`ast-grep` findings. The other crates are normal `rlib`s.

## Companion Docs

- [`VISION.md`](./VISION.md) — the grand ambition: JPL Power of Ten +
  Haskell + Gleam, expressed as Rust.
- [`v1-spec.md`](./v1-spec.md) — the concrete, buildable v1 contract.
- [`AGENTS.md`](./AGENTS.md) — agent workflow and skill routing.

## License

Dual MIT / Apache 2.0.
