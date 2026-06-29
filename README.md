# Titania

**The Rust QA fairy for AI-generated code.**

*Typed evidence, strict policy, fewer AI faceplants.*

---

> AI writes code fast. Titania makes it prove it didn't hallucinate the basics.

The goal is not to make AI "smarter" by prompting harder. The goal is to make
bad AI output **mechanically obvious**. Every panic surface, every unwrap on
a happy-path lie, every unchecked index, every stringly-typed error becomes a
typed finding with an exact file:line and a deterministic policy citation.

No prompt magic. No "the linter didn't complain." Just structured evidence
your team can review, gate on, and accumulate over time.

---

## What Titania Catches

The boring, high-leverage things LLMs get wrong in Rust:

| LLM tell-tale | How Titania catches it |
|---|---|
| `unwrap()` / `expect()` in production paths | `panic-surface` lane: ripgrep with parser prefilter, blocks production `assert!` / `unreachable!` / `panic!` / `todo!` / `unimplemented!` |
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

---

## The Core Promise

Titania is **opinionated, not configurable**. The `strict-ai` policy is the
default and only profile. There is no `titania.toml` to negotiate. You don't
write your own rules; you opt in to the opinion.

> If you don't want strict Rust, don't use titania-check.

This is the entire philosophy. Default to the strictest interpretation; let
projects opt out per-line with owner + reason + expiry, not by rewriting the
tool.

### What strict-ai enforces

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

---

## How It Runs

**Titania is a CLI tool that lives inside Moon.** Not a background agent,
not an IDE plugin, not a SaaS. You run it as a Moon task and the result
becomes a typed artifact in `.titania/out/`.

**Moon** here means [moonrepo](https://moonrepo.dev) — the polyglot
build-system / task orchestrator, not the celestial body. We're not invoking
moonbeams; we're using a deterministic build graph to make QA evidence
reproducible, cacheable, and reviewable.

```bash
# One-shot quality gate (matches PR/MR expectations)
titania-check run --scope prepush

# Edit-time feedback (~seconds, the inner loop)
titania-check run --scope edit

# Release-time receipt (full evidence, ~minutes)
titania-check run --scope release

# Diagnose a finding
titania-check explain vb-fmt-0012

# Verify your install
titania-check doctor
```

Under the hood, every scope runs a fixed set of typed **lanes**:
`fmt`, `compile`, `clippy`, `test`, `panic-scan`, `policy-scan`, `ast-grep`,
`dylint`, `cargo-deny`. Each lane emits `.titania/out/<scope>/<lane>.json`.
The aggregator assembles them into a `Report` and a `QualityReceipt` with
four digests (source, lock, policy, toolchain) — a reproducer for "the
exact code, the exact toolchain, the exact policy that produced green."

---

## Why Moon (not bash-in-YAML)

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

---

## Verification Batches

Titania is structured so that **batches of evidence** accumulate over
releases, not as binary gates. v1 ships structural/style evidence;
later versions add deeper layers without breaking earlier ones.

| Scope | Stages | Time | When |
|---|---|---|---|
| `edit` (v1) | shape, style, structure, architecture imports | seconds | every save |
| `prepush` (v1) | + tests, supply chain | minutes | before push |
| `release` (v1) | + reproducible build | tens of minutes | on tag |
| `full` (v1.5) | + Kani, cargo-mutants, coverage, API drift, MSRV | tens of minutes | on PR |
| `deep` (v2.5) | + Miri, sanitizers, cargo-fuzz, Verus, Loom | hours | nightly / merge |

**v1 ships `edit`, `prepush`, `release`.** `full` and `deep` are
sequenced for later releases; they require stable, scoped evidence
contracts before they can ship as gates.

---

## What Titania Is Not

- **Not a linter.** It's an aggregator over `clippy`, `ast-grep`, `dylint`,
  and ripgrep, with typed findings and policy enforcement. The lints live
  upstream; we own the opinion layer and the evidence shape.
- **Not configurable.** `strict-ai` is the policy. Opt out per-line with
  owner + reason + expiry, not by negotiating config.
- **Not a background agent.** It does not watch your IDE, your files, or
  your prompts. It runs on demand as a Moon task and emits a typed
  artifact you can review, gate on, and accumulate.
- **Not a CI replacement.** It runs **inside** Moon, which is the
  orchestrator. CI systems (GitHub Actions, GitLab CI, Buildkite) call
  Moon, which calls `titania-check run`.
- **Not a YAML pipeline.** It replaces bash-in-YAML with typed Rust
  lanes that emit structured JSON, run in a DAG, and cache by content
  hash.

---

## Install

```bash
# Coming with v1.0 release.
cargo install titania-check

# Or with cargo-binstall (faster, prebuilt binaries).
cargo binstall titania-check
```

For pre-v1 development, see [`v1-spec.md`](./v1-spec.md) for the buildable
contract and `cargo generate titania/template` for workspace adoption.

---

## Crate Layout

```
titania-check    CLI entrypoint, subcommand dispatch
titania-core     Domain types: Report, Finding, Lane, Receipt, primitives
titania-policy   strict-ai policy loading, validation, digest, exceptions
titania-lanes    Lane runner implementations
titania-dylint   dylint library (loaded by clippy driver)
titania-output   Report JSON serialization, doctor, explain
titania-aggregate Reads lane outputs, assembles Report, computes Receipt
```

`titania-dylint` is a `cdylib` (compiles to `.so`/`.dylib`/`.dll`); it
runs inside the clippy driver and adds type-aware context to the
`ast-grep` findings. The other crates are normal `rlib`s.

---

## Companion Docs

- [`VISION.md`](./VISION.md) — the grand ambition: JPL Power of Ten +
  Haskell + Gleam, expressed as Rust.
- [`v1-spec.md`](./v1-spec.md) — the concrete, buildable v1 contract.
- [`AGENTS.md`](./AGENTS.md) — agent workflow and skill routing.

---

## License

Dual MIT / Apache 2.0.
