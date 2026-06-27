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

### 2.1 How AI Agents Leverage Xtask (end-to-end)

Xtask is designed as the AI agent's verification layer. The agent never ships code that hasn't passed the gate. The repair loop:

```
1. AI writes Rust (targeting the curated crate stack, §5.9)
2. AI runs: xtask gate --scope fast --emit json
3. Xtask runs layers 0–6, returns structured verdict
4. AI reads the disjoint verdict JSON:
     - Pass → done, certificate emitted
     - Reject → reads typed findings + repair hints, fixes code, goto 2
     - GateFailure → recognizes infra issue (do NOT edit code), reports blocker
     - PolicyError → recognizes policy issue (edit policy, not code)
5. For performance-critical modules:
     - AI marks module #[xtask::hot] with workload/budget declaration
     - AI writes the Criterion/iai-callgrind benchmark
     - AI runs: xtask gate --scope full --emit json
     - Xtask runs the benchmark, compares against declared threshold
     - Reject with PERF_* finding if latency/throughput/allocation over budget
     - AI profiles (flamegraph/heaptrack), optimizes, re-runs
6. Certificate emitted only on full Pass → deploy-gate accepts → code ships
```

**What the AI gets from each verdict finding (the repair contract):**

| Finding type | What the AI does |
|---|---|
| `HOLZMAN_PANIC_UNWRAP` | Replace `.unwrap()` with `match`/`map_or_else` (ReplaceWith hint) |
| `HOLZMAN_PANIC_INDEXING` | Replace `items[i]` with `items.get(i)` (ReplaceWith hint) |
| `FUNC_LOOPS_IMPERATIVE` | Replace `for`/`while`/`loop` with iterator pipeline (UseIteratorPipeline hint) |
| `FUNC_NESTING_DEPTH` | Decompose nested logic into named functions (FlattenNesting hint) |
| `HOLZMAN_ARITHMETIC` | Replace raw `+` with `checked_add` (ReplaceWithCheckedArith hint) |
| `HOLZMAN_UNSAFE` | Remove unsafe or request explicit waiver (RemoveUnsafeWaiver hint) |
| `SUPPLY_BANNED_CRATE` | Replace banned crate with approved alternative (ReplaceWith hint) |
| `MUTANT_SURVIVING` | Add a test that catches the mutation (RequiresHumanReview hint) |
| `PERF_NO_BENCHMARK` | Write a named Criterion/iai-callgrind benchmark for the hot module |
| `PERF_REGRESSION` | Profile (flamegraph/heaptrack), optimize bottleneck, re-run |
| `PERF_ALLOCATION_OVER` | Reduce allocations (borrow, preallocate, arena, caller-owned buffer) |

**Why this works for AI agents:**
- The verdict is **deterministic** — same code always produces the same verdict. The AI's repair loop converges; no flapping.
- The failure categories are **type-disjoint** — the AI never wastes a cycle trying to fix an infra failure by editing code.
- The repair hints are **typed and machine-actionable** — `ReplaceWith{old, new}` is auto-applicable; `RequiresHumanReview` tells the AI to stop and escalate.
- The certificate is **the trust artifact** — the AI cannot fabricate it; only a full Pass + CI signing produces it. Deploy-gate rejects without it.
- The AI **cannot route around the gate** — no `--force`, no inline bypass, policy-PR is the only escape and that PR passes the same gate.

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

These are correctness-of-claim gates, not correctness-of-compilation. They activate when a module is marked hot/critical in policy. **No performance claim is accepted without benchmark evidence.** The full Holzman PLUS extensions:

| Extension | Requirement | Xtask enforcement |
|---|---|---|
| Workload definition | State target hardware, input distribution, hot path, threshold before optimizing | semgrep: hot-module must declare a `#[xtask::hot]` workload doc |
| Latency budget | p50/p95/p99 or max latency when user-visible/real-time/networking | requires named benchmark passing threshold; `cargo bench` evidence |
| Throughput budget | ops/sec, bytes/sec, req/sec under realistic batching+concurrency | requires named benchmark passing threshold |
| Allocation budget | mission-critical: zero post-init alloc; perf-only: count/bound allocs | `try_reserve` on untrusted growth; clippy `box_collection`/semgrep `format!`-in-hot-path |
| Storage placement | stack/heap/arena/pool chosen by measured size, lifetime, locality | architectural review; clippy flags `Vec` where `SmallVec`/`ArrayVec` wins |
| Cache layout | `size_of` review, field order, padding, AoS vs SoA, false sharing | semgrep: hot struct flagged if `#[repr(C)]` absent and size > cache line |
| Static dispatch | generics/enums/inline in hot paths; justify `dyn Trait` | clippy disallowed `dyn Trait` in `#[xtask::hot]` modules |
| Branch behavior | split hot/cold; minimize unpredictable branches in tight loops | clippy `cognitive_complexity`; `#[cold]` on rare error paths |
| Numeric semantics | `checked_*` for external input; `saturating_*` for counters; `wrapping_*` for hashes/ring-buffers; plain `+-*` only with local range proof | `arithmetic_side_effects` deny; semgrep flags raw `+`/`-`/`*` in hot modules |
| SIMD discipline | scalar oracle + scalar fallback + target-feature gate + alignment/remainder + benchmark | unsafe SIMD forbidden; safe `std::simd`/auto-vectorization only; needs explicit waiver for `std::arch` |
| Concurrency budget | bounded queues/tasks/retries/locks; document cancellation + lock ordering | `await_holding_lock` deny; semgrep unbounded `spawn`/`channel` |
| Code size | monomorphization/inlining/feature-flag bloat when changed | `cargo bloat` + `cargo llvm-lines` on changed hot crates |
| Regression guard | baseline + result + command + workload + pass/fail threshold recorded | benchmark must include before/after evidence in the verdict |

**Mechanical Empathy Standard (the meta-rule):** fast Rust makes the machine do less work — fewer bytes moved, fewer cache misses, fewer heap allocations, fewer unpredictable branches, fewer locks/atomics/syscalls, fewer virtual calls. Do not accept performance claims based on style. Accept only measured bottleneck removal. Optimization hierarchy: algorithm → memory traffic → data layout → allocation → branch predictability → synchronization → compiler visibility → target-specific builds/SIMD.

**Arithmetic Standard:** every integer operation must name its overflow behavior — `checked_*` (invalid input/invariants), `saturating_*` (counters/metrics), `wrapping_*` (hashes/checksums/ring-buffers), plain arithmetic only when type/range proof is local and obvious.

**OOM discipline:** hot paths and untrusted-input paths that grow memory must declare max size, use checked arithmetic for capacity, call `try_reserve` when allocation failure must be graceful, and return typed resource errors. `Vec::new()` + unbounded push on untrusted data = Reject.

**Benchmarking & Profiling Tool Stack** — these are the evidence tools Xtask wraps when a module is declared hot. A performance claim with NO benchmark is `BLOCKER`, not a pass.

| Tool | Role | Xtask gate use |
|---|---|---|
| **Criterion** (`criterion = "0.8"`) | Statistical local benchmarks — p50/p95/p99, regression detection, outlier classification | Named `cargo bench --bench <name>` must exist + pass threshold for `#[xtask::hot]` modules |
| **iai-callgrind** (`iai-callgrind = "0.16"`) | Deterministic instruction/cache regression — no timing noise, counts raw instructions | CI-stable regression gate where timing is too noisy; exact instruction-count comparison |
| **hyperfine** | CLI command benchmark comparison — statistical, warmup-aware | Benchmarking Xtask's own binary and CLI-path performance claims |
| **perf stat** | CPU hardware counters — cycles, instructions, cache-misses, branches, branch-misses | Profiler evidence required for cache/dispatch/branch claims |
| **cargo flamegraph / samply** | Hot-path discovery — sampled stack profiles | Required when the AI claims "the bottleneck is X" — Xtask demands the flamegraph showing X |
| **heaptrack / DHAT / bytehound** | Allocation profiling — count/bytes per site, peak memory | Required for allocation-budget claims; `allocations_apply_input = 0` assertions |
| **cachegrind** (valgrind) | Cache simulation — cache-miss modeling without hardware variance | Portable cache-behavior evidence where perf counters vary |
| **cargo bloat** | Binary size — which crates/types dominate the binary | Code-size gate; flags monomorphization bloat on changed hot crates |
| **cargo llvm-lines** | IR line count per generic — monomorphization/code-size bloat | Flags generic functions that explode under feature-powerset |
| **tokio-console / console-subscriber** | Async task/resource diagnostics — task stalls, contention | Required for async hot-path claims; flags tasks holding locks across await |

**Xtask's benchmark gate contract (for `#[xtask::hot]` modules):**
1. A named benchmark target (Criterion or iai-callgrind) MUST exist in the crate. Missing benchmark = `PERF_NO_BENCHMARK` finding.
2. The benchmark MUST pass the declared latency/throughput threshold from policy. Regression = `PERF_REGRESSION` finding with before/after numbers.
3. Allocation-sensitive modules MUST have heaptrack/DHAT evidence showing the allocation count is within budget. Over-budget = `PERF_ALLOCATION_OVER` finding.
4. The verdict records the benchmark command, the measured numbers, the threshold, and the pass/fail. This is the regression guard — recorded in the certificate.

**Forbidden for performance claims:**
- Generic `cargo bench` as discovery only — it is NOT a named benchmark. Xtask requires a real target name.
- Template command names — Xtask substitutes the actual repo benchmark name or reports `BLOCKER`.
- "It looks idiomatic, therefore fast" — no style-based claims accepted.

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

### 5.8 Strict Clippy Configuration (STUPIDLY strict — all groups maxed)

Xtask enforces the maximum clippy strictness on gated source. Start with ALL lint groups denied, then allow-list only the few that are genuinely inapplicable. Tests are exempt from style gates (compile + behavior only).

**The enforced `[workspace.lints.clippy]` on gated code:**

```toml
# --- ALL lint groups at maximum deny ---
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
cargo = { level = "deny", priority = -1 }
restriction = { level = "warn", priority = -1 }   # warn all, deny the important ones below

# --- Hard denies: panic surface (Holzman panic-free standard) ---
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
panic_in_result_fn = "deny"
todo = "deny"
unimplemented = "deny"
unreachable = "deny"
dbg_macro = "deny"
missing_panics_doc = "deny"        # if you document a panic, you shouldn't have one
exit = "deny"                       # no std::process::exit

# --- Hard denies: footgun access ---
indexing_slicing = "deny"
string_slice = "deny"
get_unwrap = "deny"
arithmetic_side_effects = "deny"
as_conversions = "deny"
let_underscore_must_use = "deny"
await_holding_lock = "deny"

# --- Hard denies: functional-rust (no unwrap in ANY form) ---
unwrap_or_default = "deny"
unwrap_or_else = "deny"
unwrap_or = "deny"

# --- Hard denies: code quality ---
too_many_lines = "deny"             # threshold 40
too_many_arguments = "deny"         # threshold 5
manual_unwrap_or_default = "deny"
map_unwrap_or = "deny"
str_to_string = "deny"
string_to_string = "deny"
suspicious_operation_groupings = "deny"
tests_outside_test_module = "deny"
unused_async = "deny"
unused_self = "deny"
wildcard_enum_match_arm = "warn"    # functional-rust: no wildcard arms in domain match
missing_errors_doc = "deny"
missing_const_for_fn = "warn"

# --- Warns (promotion to deny via policy) ---
shadow_unrelated = "warn"           # no variable shadowing
print_stdout = "warn"               # use tracing, not println
print_stderr = "warn"
default_numeric_fallback = "deny"   # no implicit i32 default
float_arithmetic = "warn"           # float in hot paths needs care
rc_buffer = "warn"
rc_mutex = "warn"
mod_module_files = "deny"
self_named_module_files = "deny"
verbose_file_reads = "warn"
use_debug = "warn"
```

**What clippy CANNOT express — those rules live in Layer 3 (semgrep), not custom lint code:**
- No imperative loops (`for`/`while`/`loop`) — clippy has no blanket no-loops lint
- Nesting depth > 2 — clippy `cognitive_complexity` is advisory, not a hard gate
- `Result<T, String>` error type — clippy doesn't catch stringly-typed errors
- Hidden I/O in helper functions — architectural, needs pattern matching
- `format!`/`to_string()` in `#[xtask::hot]` modules — needs hot-module awareness

Xtask does NOT build custom lint passes or compiler extensions. It wraps clippy at the above strictness and uses semgrep for the remainder. **Wrap tools, don't build them.**

### 5.9 Allowed Library & Crate Policy (Holzman curated stack)

Xtask enforces a curated crate allowlist via `cargo deny` config + clippy `disallowed_methods`/`disallowed_types` + semgrep. Only audited, Holzman-approved crates are permitted. Adding a non-approved crate requires a policy-PR.

**Approved crates by purpose:**

| Purpose | Approved crates | Notes |
|---|---|---|
| Async I/O | `tokio` | No CPU-heavy loops on async workers |
| HTTP/API | `axum`, `tower`, `tower-http`, `hyper` | Enforce timeouts, limits, tracing |
| CPU parallelism | `rayon` | Only for large independent work with scaling evidence |
| Concurrency | `crossbeam-channel`, `parking_lot`, `flume` | Bounded queues only |
| Buffers | `bytes`, `arrayvec`, `smallvec`, `heapless` | Choose by measured size/lifetime/cache |
| Arenas | `bumpalo` | Only when many objects share one lifetime |
| Maps | `hashbrown`, `ahash`, `rustc-hash` | Fast hashers for internal/non-adversarial keys only |
| Immutable state | `rpds`, `arc-swap` | Structural-sharing snapshots, lock-free reads |
| Concurrent state | `dashmap` | High-throughput; measured only |
| Ergonomic pipelines | `itertools`, `tap` | Sync pipelines, linear pipe() flow |
| Formats (binary) | `postcard` | Default compact Serde-compatible |
| Formats (zero-copy) | `rkyv` | Audited zero-copy only |
| JSON | `serde_json` | Default; `sonic-rs`/`simd-json` only when proven bottleneck |
| Parsing | `winnow`, `nom`, `lexical-core` | Reduce handwritten parser risk |
| Errors | `thiserror` (core), `anyhow` (shell) | No `Result<T, String>` in core |
| Checksums | `crc32fast` | Fast non-crypto checksums |
| Hashing (crypto) | `blake3` | Content-addressing digests |

**Banned crates / methods (Reject on sight):**
- New `bincode` usage (maintenance risk per Holzman)
- `chrono::Local` direct calls (non-determinism)
- Raw `rand::random()` / `thread_rng()` direct calls outside a controlled primitive
- `std::sync::Mutex` in async handler scope (use `parking_lot` or sharding)
- `std::cell::RefCell` in async/workflow scope
- Fast non-cryptographic hashers for adversarial/user-controlled keys
- `async_trait` in hot paths without measurement
- `mimalloc`/`tikv-jemallocator` only after heap profiling proves allocator pressure remains

**Xtask enforcement:** `cargo deny` config with the allowlist; clippy `disallowed_methods`/`disallowed_types` for method/type bans; semgrep for `chrono::Local`/`rand::random`/`std::sync::Mutex` in scope-sensitive positions.

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
