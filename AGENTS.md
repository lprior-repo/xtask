# Agent Instructions

High-assurance Rust work for **titania-check**. Beads for tracking. Skills for verification discipline.

Moon is non-negotiable: **MOON CI/CD is the absolute foundation for all of this work.**
Every high-assurance lane, Rust gate, and Beads delivery step is expected to
respect Moon as the canonical CI/CD orchestrator.

## Beads (bd)

Use `bd` for ALL task tracking. Never TodoWrite/markdown TODO lists. Run `bd prime` for full context after compaction.

```bash
bd ready                  # Find work
bd show <id>              # View issue
bd update <id> --claim    # Claim atomically
bd close <id>             # Complete
bd dolt push              # Sync to remote
```

**Architecture:** issues live in a local Dolt DB (`.beads/dolt/`); cross-machine sync uses `bd dolt push/pull` over `refs/dolt/data` on the git remote — separate from `refs/heads/*` where code lives. `.beads/issues.jsonl` is a passive export, not the wire protocol. See [SYNC_CONCEPTS.md](https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md).

**Session close:** close beads → run quality gates → `git status` → conservative profile by default (report handoff, do NOT commit/push/sync unless explicitly authorized).

## Non-Interactive Shell Commands

`cp`, `mv`, `rm` may be aliased with `-i` and hang the agent. Use `-f`/`-rf`:

```bash
cp -f src dst       # NOT: cp src dst
mv -f src dst       # NOT: mv src dst
rm -f file          # NOT: rm file
rm -rf dir          # NOT: rm -r dir
```

Other prompts to bypass: `scp -o BatchMode=yes`, `ssh -o BatchMode=yes`, `apt-get -y`, `brew` with `HOMEBREW_NO_AUTO_UPDATE=1`.

## Rust Verification Stack — Skill Routing

When work touches Rust code, the **whole stack fires by default** unless the bead explicitly opts out of an obligation. Skills own lanes — agents orchestrate them. No alternate Rust, async, test, proof, or performance agent may be used unless the user grants a per-task override.

### Source & Gate Lanes (every Rust change)

| Skill | Lane | Owns |
|-------|------|------|
| `functional-rust` | Architecture & source purity | Data/Calc/Actions layering, zero unwrap/panic, iterator pipelines, parse-don't-validate, illegal-states-unrepresentable, file headers `#![deny(clippy::unwrap_used)] #![forbid(unsafe_code)]` |
| `holzman-rust` | Lint, CI gate, perf | NASA/JPL Power-of-Ten, strict `cargo fmt/clippy/check/test`, allocation/latency/throughput budgets, second-ring evidence (asm/IR/SBOM/auditable), `cargo geiger` zero-unsafe policy |
| `moon-v2` | Moon build & CI triage | `.moon/`, `moon ci` (canonical — NOT `moon run :ci`), `lint-src` with `-W clippy::all` (zero-tolerance source lint), `fmt`/`check`/`test`, source-length gates, 3-layer cache (sccache → bazel-remote → Cargo incremental) |
| `async-rust-reviewer` | Async Rust review | spawn at edge only, domain crate has ZERO async deps, streams over loops, bounded concurrency, `Send+Sync` hygiene, cancellation as protocol (request → stop intake → drain → finalize → report), two-phase effects, capability-gated I/O, max 3 .await/fn + 60 LOC, `tracing` + `tokio-console` + OTLP from day one |
| `miri` | UB detection | `cargo miri`, `MIRIFLAGS`, Stacked/Tree Borrows, Strict Provenance, `MaybeUninit`, alignment, use-after-free, leaks, data races — **nightly-only, second-ring for unsafe paths, evidence not whole-crate soundness** |

### Proof & Model Lanes (scope-bound)

| Skill | Lane | Owns |
|-------|------|------|
| `verus` | Deductive Rust proof | `spec`/`proof`/`exec` mode contracts, loop invariants, quantifier triggers, ghost/exec separation, verifier diagnostics, trusted-boundary audit. **Default for Rust-local pure invariants. No `#[verifier::external_body]` laundering.** Mandatory **STRONG / WEAK_MIRROR / WEAK_EXTERN** production-binding — every spec must `#[path = "..."]` to production AND `assume_specification[ production::fn ]` bridge. **VACUUM (shadow types with no `#[path]`) is lethal.** |
| `flux-rs` | Refinement types | `#[refined_by]/#[variant]/#[sig]`, bounds/legal-state/length/index refinements, `&strg` post-states, extern specs, panic preconditions (nightly-only). **Lightweight alternative to Verus for refinement-shaped obligations. `#[trusted]`/`#[ignore]`/`#[extern_spec]` are proof debt — keep thin and reported.** |
| `kani` | Bounded model check | `#[kani::proof]` harnesses, `kani::any/assume/cover`, `#[unwind]`, `#[should_panic]`, panic-freedom, arithmetic/index/state-transition, unsafe harnesses. **Bounded claims only — `cover!` is non-vacuity evidence, never proof. Run cgroup-capped (`-j 1`, `MemoryMax=24G`, `MemorySwapMax=0`).** |
| `loom` | Concurrency permutation | `loom::model`, `Builder`, `cfg(loom)` sync indirection (NEVER import std sync directly), thread schedules, lock-free primitives, sync protocols, memory ordering. **Small bounded models only; never I/O-bound async. Run `--release` with `LOOM_MAX_PREEMPTIONS=2-3`. Yield in spin loops under `cfg(loom)`.** |
| `tla-plus` | Temporal design model | `.tla`/`.cfg`/PlusCal, `Init/Next/vars`, safety invariants, liveness/fairness, deadlock, workflow/lease/retry/claim-handoff/lifecycle. **TLC baseline; Apalache only when obligation names it. Bridges to Rust via Verus/Kani/Flux/Loom/proptest/fuzz.** Note: `proof-planner` lane for temporal workflows uses loom + proptest shadow — TLA+ remains the design-of-record for protocol modeling. |
| `rust-fuzzer` | Hostile-input fuzzing | `cargo-fuzz`/libFuzzer (primary), AFL++ (long campaigns), honggfuzz (alt feedback), LibAFL (custom), fuzzcheck/arbitrary (structured), sanitizer/coverage lanes, crash triage, OSS-Fuzz. **Stage-split harnesses (lexer/parser/IR/interpreter/JIT). Oracles required (round-trip, differential, metamorphic). Bounded execution + persistent reset. Risk-selected companion for parsers/decoders/untrusted input.** |

### Selection priority (no overlapping lanes)

- Pure core invariant (Rust-local) → **Verus** (mandatory production-binding).
- Lightweight refinement (bounds/legal state/index) → **Flux RS**.
- Bounded execution check (panic, arithmetic, state, unsafe harness) → **Kani**.
- UB / unsafe / FFI layout / `MaybeUninit` → **Miri** (second-ring after human + clippy + Kani).
- Thread interleaving / memory ordering → **Loom**.
- Temporal workflow / protocol / fairness / deadlock → **TLA+** (design) + **Loom + proptest** (Rust shadow).
- Hostile-input / parser / decoder → **rust-fuzzer** (+ `proptest` shadow).
- Tiny theorem kernel beyond Verus → **Lean/Aeneas/Hax** (obligation-specific).
- Async code path / runtime / waker / spawn → **async-rust-reviewer** (also routes tokio/loom).

### Mandatory verification gate (Rust)

`holzman-rust` strict gate fires every Rust change. `moon-v2` `moon ci` is the canonical Moon gate when tasks exist. No completion without exit 0:

```bash
cargo fmt --check
cargo clippy -- --all-targets -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
cargo test --workspace --all-features
rg -n '(^|[^A-Za-z0-9_])(assert!|assert_eq!|assert_ne!|unreachable!)' --glob '*.rs' --glob '!**/tests/**' --glob '!**/benches/**'
cargo audit && cargo deny check && cargo vet && cargo geiger && cargo machete
```

### Verification gate discipline

- Never invent CLI output, benchmark numbers, verifier diagnostics, or file paths.
- Strict clippy is **source-target only** — test implementation style is irrelevant; test design and exact assertions are mandatory.
- Compile tests/examples/benches with `cargo check --workspace --all-targets --all-features`.
- Bead delivery classifies failures as `BLOCK_LOCAL` / `BLOCK_REGRESSION` / `BLOCK_GLOBAL` / `REQUIRED_OBLIGATION_FAIL` / `WAIVED`. Local, regression, required-obligation, and global-readiness failures block until repaired.
- Production `assert!`/`assert_eq!`/`unreachable!` are panic paths — forbidden except in tests, benches, build scripts, or process-start invariant failure with diagnostics.
- `cargo geiger` reporting unsafe in touched production code is a fail unless user explicitly approved an unsafe waiver **before** the code was written.
- Performance claims need benchmark/profiler evidence (commands + numbers), not template names.
- `cargo miri` / `cargo kani` / `cargo flux` / `verus` / `tlc` / `cargo fuzz` runs are only valid when the exact tool, scope, harness/spec/model, command, and pass/fail output are recorded. Missing tools → `BLOCKER`, not pass.

## Bead Delivery Pipeline (Go-Skill Lifecycle)

For every Rust bead the following lanes fire in order. Use `go-skill` to orchestrate, or invoke specialists directly.

| Phase | Skill(s) | Output |
|-------|----------|--------|
| Scout | `explore` | Map files, APIs, crates, risks, existing verification artifacts |
| Contract | `rust-contract` | Ubiquitous language, value objects, typestates, workflows, hazard analysis, **proof seeds** (not obligations), traceability matrix |
| Plan | `proof-planner` → `proof-plan-reviewer` → `proof-writer` | Verification ledger, refined plans, written Verus/Flux/Kani/Loom/proptest/fuzz artifacts |
| Review | `proof-reviewer` | Adversarial review + **Verus production-binding audit (STRONG/WEAK/VACUUM)** |
| Bridge | `proof-to-implementation` | Map proof claims → Rust refs, behavior tests, harness refs, exact commands |
| Test | `test-planner` → `test-writer` → `test-reviewer` | Behavior tests via Testing Trophy (~60% integration / ~30% unit / ~5% e2e / ~5% static), BDD, proptest, fuzz, mutation ≥90% kill, ≥90% line coverage, no `is_ok()`-only assertions |
| Implement | `functional-rust` + `holzman-rust` (+ `async-rust-reviewer` / `moon-v2` as scoped) | Source + test code, passing gates |
| Behavior | `bdd-enforcer` | Given/When/Then scenarios proving behavior end-to-end |
| Gate | `black-hat-reviewer` | Contract parity, Farley constraints, Holzman Rust, strict DDD, bitter-truth simplicity |
| Audit | `truth-serum` | Dual-persona audit of AI-generated work — verify claims with command evidence |
| Package | `evidence-packaging` | Truth-serum-audited assurance bundle — requirements mapped to raw evidence |
| Land | `landing-skill` | Quality gates passed, sync pushed, clean handoff |

For multi-bead concurrent dispatch with per-bead gates preserved, use `femdation`. For architectural drift / oversized files / DDD cohesion repair mid-pipeline, use `architectural-drift` or `scott-ddd-refactor`.

## Beads Skill Pointer

For durable workflow guidance see `.agents/skills/beads/SKILL.md` (project) or `~/.agents/skills/beads/SKILL.md` (global).

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:970c3bf2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   bd dolt push
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd update <id> --claim  # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.
<!-- END BEADS CODEX SETUP -->
