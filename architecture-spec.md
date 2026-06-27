# Architecture Spec: Xtask — The Deterministic Rust Quality Enforcement Gate

> Status: ARCHITECTURE SPEC v3.0 — black-hat hardened
> v3 addresses a comprehensive adversarial review: signing-key isolation, artifact-bound certificates, freshness/revocation, anti-circular policy, bypass-surface countermeasures, corrected tool claims, sandbox profile, honest non-overclaiming, and a corrected domain model.
> Next step: run `arch-spec-to-beads` to shred this into molecular tasks.

---

## 0. Product Sentence (honest, non-overclaiming)

```
Xtask is a deterministic CLI that wraps a pinned stack of Rust quality tools
(fmt, rustc, clippy, semgrep, cargo-audit/deny/vet/geiger/machete, cargo-hack,
cargo-mutants) and gates code against a specific, pinned policy derived from
the NASA/JPL Power-of-Ten and functional-Rust doctrine. It is the DECIDE step
of an AI authoring OODA loop: it returns a structured per-layer report and a
signed attestation, or it rejects.

PASSING XTASK MEANS THE CODE PASSED A SPECIFIC PINNED POLICY. It does not mean
"you KNOW the code is high quality" in an absolute sense — tools have limits
(Miri observes particular executions, Kani can run out of resources, semgrep
cannot see macro-expanded code). What Xtask guarantees is that the code could
not ship without mechanically passing every scoped lane of that pinned policy,
and that the attestation is artifact-bound and fresh. No macros, no DSL, no LLM,
no custom lint code inside the gate — 100% mechanical tool wrapping.
```

---

## 1. Threat Model

**The AI code author is potentially MALICIOUS, not merely buggy or careless.**

The design assumes the author will attempt to bypass the gate via: `#[allow]`/`#[expect]`, `.cargo/config.toml`, build scripts, proc macros, `include!`/`#[path]`, `cfg`-gated code, PATH poisoning, tool-native suppressions (semgrep ignores, cargo-audit `--ignore`, cargo-vet exemptions, cargo-mutants `--exclude`), generated code, dependency tricks, empty proof obligations, and policy weakening.

**The signing key is a crown-jewel secret.** It must NEVER exist in any environment that executes untrusted Cargo commands (build scripts, proc macros, tests, fuzz, mutants all run attacker-controlled code). See §7 (two-environment architecture).

**Xtask's own source is trusted** (it is gated by itself and reviewed). The GATED code (what the AI writes) is untrusted.

---

## 2. Non-Goals (corrected — no contradictions)

- **NO Xtask-specific authoring macros or DSL.** Ordinary Rust macros (`thiserror`, `serde`/`clap` derives, `assert!` in tests) are allowed through policy. What is banned is an Xtask-defined grammar that the AI writes code inside.
- **NO LLM inside the gate.** The AI is external; it consumes the report JSON.
- **NO claim of omniscience.** Xtask enforces a pinned policy; it does not prove behavioral correctness or catch all UB. The product sentence is honest about this.
- **NO bypass flag.** No `--force`. The only escape is a policy-PR that is checked against the PREVIOUS policy plus a meta-policy (§13).
- **NO code generation as source of truth.** Xtask emits typed repair hints, never the author's final code.

---

## 3. Positioning — The AI OODA Loop + Agent Leverage

```
OBSERVE   AI reads context, existing code, Xtask's prior report
ORIENT    AI generates candidate Rust (targeting the curated crate stack, §6.9)
DECIDE    Xtask gate — runs the toolchain in a sandbox, emits report + attestation
ACT       Accepted code (valid, fresh, signed attestation + matching artifact) ships
```

The AI is the primary invoker. The repair loop:

```
1. AI writes Rust
2. AI runs: xtask gate --scope edit --emit json     (fast loop: fmt/check/clippy/semgrep/assert-scan)
3. AI reads the disjoint report:
     Pass          → fast lanes passed; run --scope full for CI-readiness
     CodeReject    → reads typed findings + repair hints, fixes code, goto 2
     GateReject    → recognizes infra issue (do NOT edit code), reports blocker
     PolicyError   → recognizes policy issue (edit policy, not code)
     InputError    → fix the input contract
4. AI runs: xtask gate --scope full --emit json      (CI path: +supply/hack/mutants)
5. Attestation emitted only on full Pass → signer environment signs → deploy-gate accepts
```

**Scope tiers (the "fast" loop is actually fast):**

| Scope | Lanes | Target latency | Use case |
|---|---|---|---|
| `edit` | fmt, check, clippy, semgrep, assert-scan | <30s | AI repair loop (dozens of iterations) |
| `prepush` | edit + supply-chain + feature-powerset | <90s | before push |
| `full` | prepush + mutants | <300s, policy-scaled | CI / deploy-gate |

Time budgets are **SLOs scaled by crate size**, not correctness gates. A large healthy crate that takes 60s for clippy is not "rejected" — the budget adapts. Mutants/feature-powerset budgets are per-policy.

---

## 4. EARS Requirements (corrected)

### Event-driven
- **When** the AI submits a crate or diff, **the system shall** run the scoped lanes, short-circuiting ONLY compilation-dependent lanes when Layer 1 (check) fails. Semgrep (Layer 3) runs on source REGARDLESS of compilation status — it does not need the crate to typecheck, and skipping it on compile failure reduces repair signal.
- **When** any scoped lane cannot produce a report (tool crash, timeout, missing binary), **the system shall** emit a `GateReject` with no attestation.
- **When** all scoped lanes emit clean, **the system shall** emit `Pass`, compute the evidence digest, and write canonical evidence artifacts to the output path. Signing happens in a SEPARATE environment (§7).

### State-driven
- **While** an attestation is valid, fresh (within `not_after`), and its evidence digest matches the current artifact+policy+advisory-db, **the deploy-gate shall** permit deployment.
- **If** the advisory DB has changed since the attestation was issued (advisory_db_digest mismatch), **the deploy-gate shall** REJECT regardless of signature validity — a dependency may have gained an advisory.

### Unwanted
- **If** the signing key is present in any environment that executes a Cargo command on untrusted code, **the system design is BROKEN.** This is an invariant (§7).
- **If** a tool-native suppression (`#[allow]`, `#[expect]`, semgrep ignore, cargo-audit `--ignore`, etc.) is found in gated source, **the system shall** flag it unless explicitly ledgered (§12).

---

## 5. Domain Model (corrected — single disjoint root enum)

### 5.1 The report type

```rust
/// The single disjoint root. Every invocation produces exactly one.
pub enum Report {
    Pass { evidence: EvidenceDigest, per_lane: Box<[LaneOutcome]> },
    CodeReject { findings: Box<[Finding]>, per_lane: Box<[LaneOutcome]> },
    GateReject { failures: Box<[LaneFailure]>, per_lane: Box<[LaneOutcome]> },
    PolicyError(PolicyDiagnostic),
    InputError(InputDiagnostic),
}
```

`CodeReject` = the Rust is wrong (AI edits code). `GateReject` = a tool crashed/missing/timeout (AI does NOT edit code — infra). `PolicyError` = policy malformed (edit policy). These are type-disjoint with disjoint fix paths.

### 5.2 Lane outcomes (corrected skip semantics)

```rust
pub enum LaneOutcome {
    Clean { evidence: LaneEvidence },
    Findings(Box<[Finding]>),
    Failed(LaneFailure),
    Skipped(SkipReason),
}

pub enum SkipReason {
    PriorCompilationFailure,   // Layer 1 failed; compilation-dependent lanes skip
    NotSelectedByScope,        // the --scope didn't include this lane
    NotApplicable,             // e.g. no unsafe code → Miri not applicable
    PolicyDisabled,            // policy explicitly disabled this lane
}
```

### 5.3 The finding type

```rust
pub struct Finding {
    pub lane: Lane,
    pub rule_id: RuleId,
    pub location: Location,        // NOT always a span (see below)
    pub message: String,
    pub repair: RepairHint,
}

/// Location is a sum type — many findings are not source spans.
pub enum Location {
    Span { file: PathBuf, line_start: u32, col_start: u32, line_end: u32, col_end: u32 },
    Dependency { crate_name: String, version: String, source: DepSource },
    Manifest { file: PathBuf },
    Workspace,
    Tool { name: String, version: String },
    Artifact { digest: Digest },
}
```

### 5.4 Repair hints (serializable, JSON-friendly — no Rust lifetimes)

```rust
#[derive(Serialize, Deserialize)]
pub enum RepairHint {
    Patch { file: String, range: TextRange, replacement: String },
    UseIteratorPipeline { suggestion: String },
    FlattenNesting { suggestion: String },
    UseCheckedArithmetic { op: String },   // "checked_add", "saturating_sub", etc.
    RemoveAllowAttribute { attr: String },
    ReplaceDependency { from: String, to: String },
    AddBenchmark { benchmark_name: String },
    RequiresHumanReview { note: String },
}
```

### 5.5 Lane evidence (every Clean outcome carries proof)

```rust
pub struct LaneEvidence {
    pub command: String,           // exact command run
    pub tool_path_hash: Digest,    // hash of the resolved binary path
    pub tool_version: String,
    pub env_digest: Digest,        // relevant env vars (frozen, §9)
    pub stdout_digest: Digest,     // digest of captured stdout (canonicalized)
    pub stderr_digest: Digest,
    pub duration_ms: u64,
    pub exit_status: i32,
    pub parsed_result_digest: Digest,  // digest of the parsed findings/result
}
```

---

## 6. The Doctrine (Holzman + functional-rust — tool-correctness fixed)

### 6.1 Panic-free standard (mechanically unbeatable for the constructs it covers)

`unsafe_code` = forbid. `unwrap`/`expect`/`panic`/`todo`/`unimplemented`/`unreachable!`/`dbg!` denied. Production `assert!`/`assert_eq!`/`assert_ne!` (the `rg` scan, excluding tests/benches/examples/build.rs BUT build.rs gets its OWN stricter lane — §8).

**Honest caveat:** `panic_in_result_fn` cannot prove a function doesn't panic (called functions may). `unused_must_use` only catches `#[must_use]` types, not all results (add `unused_results` as warn). Indexing ban catches `items[i]` but not all panic paths.

### 6.2 Strict clippy (STUPIDLY strict — all groups maxed, tool-correctness fixed)

Start with ALL groups denied, allow-list only inapplicable lints. Tests exempt from style (compile + behavior only).

```toml
# ALL groups at maximum
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
cargo = { level = "deny", priority = -1 }
restriction = { level = "warn", priority = -1 }

# CRITICAL lints use -F (forbid), NOT -D — #[allow] cannot lower a forbid
# Xtask passes these as -F flags on the command line, not workspace lints:
#   -F clippy::unwrap_used -F clippy::expect_used -F clippy::panic
#   -F clippy::indexing_slicing -F clippy::string_slice -F clippy::get_unwrap
#   -F clippy::arithmetic_side_effects
# This is critical: -D lints CAN be overridden by #[allow] in source.
# -F (forbid) cannot be lowered except via lint caps (which Xtask also scans for).

# Functional-rust style denies (these are STYLE, not panic-safety):
unwrap_or_default = "deny"     # unwrap_or does NOT panic — this is house style
unwrap_or_else = "deny"
unwrap_or = "deny"
too_many_lines = "deny"         # threshold 40
too_many_arguments = "deny"     # threshold 5
exit = "deny"
str_to_string = "deny"
string_to_string = "deny"
default_numeric_fallback = "deny"
missing_errors_doc = "deny"
missing_panics_doc = "deny"
missing_const_for_fn = "warn"
shadow_unrelated = "warn"
print_stdout = "warn"
print_stderr = "warn"
wildcard_enum_match_arm = "warn"  # functional-rust house style
fn_params_excessive_bools = "warn"  # CORRECTED: not fn_args_justly (doesn't exist)

# NOTE: non_exhaustive_omitted_patterns is the CORRECT lint name (not non_exhaustive_patterns).
# #[non_exhaustive] FORCES wildcard arms for downstream consumers — the no-wildcard rule
# needs an exception with explicit rationale for external non_exhaustive enums.
```

**`#[allow]`/`#[expect]` scan (anti-bypass):** Xtask scans gated source for `#[allow(...)]`, `#[expect(...)]`, `cfg_attr(..., allow(...))`, and `#![allow(...)]`/`#![expect(...)]` attributes. ANY un-ledgered suppression attribute = `BYPASS_ALLOW_ATTRIBUTE` finding → CodeReject. Only the policy file can permit specific allows, and those are recorded in the trusted-base ledger.

**`--cap-lints` statement:** Cargo uses `--cap-lints allow` for dependencies. The lint regime applies to FIRST-PARTY source only. This is stated explicitly — transitive crate lint failures are out of scope (cargo-deny/geiger handle supply-chain).

**Repair-hint caveat:** clippy's own suggestions can conflict with policy (e.g. `get_unwrap` suggests indexing, but indexing is also banned). Xtask generates CUSTOM repair hints, not raw clippy suggestions.

### 6.3 Functional-rust doctrine (honest about decidability)

| Rule | Enforcement | Honest caveat |
|---|---|---|
| No imperative loops (`for`/`while`/`loop`) | semgrep | This is HOUSE STYLE, not inherently more verifiable/faster. Iterator complexity can replace loop complexity. |
| ≤2 nesting depth | semgrep | House style; not a universal quality fact. |
| No `unwrap_or*` family | clippy deny | STYLE rule — `unwrap_or` does NOT panic. |
| No `Result<T, String>` | semgrep | Catches the pattern; cannot prove error taxonomy quality. |
| No wildcard arms in domain match | semgrep (warn) | `#[non_exhaustive]` external enums FORCE wildcards — needs exceptions. |
| No bool control flags | `fn_params_excessive_bools` (warn) | Checks declarations, not every bool. |
| Parse don't validate | NOT a hard gate | Architectural guidance; admitted in spec. |
| Zero-copy parsing | NOT enforced | Clippy catches "some" cases; not a gate. |
| No hidden I/O | NOT decidable by semgrep | A call behind a trait/dependency/callback can do I/O. Needs architecture manifest, not pattern matching. |
| Make illegal states unrepresentable | NOT mechanically enforced by wildcard ban | Design guidance unless backed by type-state checks. |
| No recursion | semgrep (syntactic only) | Catches direct recursion; NOT mutual recursion, trait dispatch, fn pointers, macro expansion, or generated code. |

### 6.4 PLUS performance gates (opt-in, honest about runtime)

These are correctness-of-CLAIM gates that require RUNTIME evidence (benchmarks, profiling). This CONTRADICTS any "no runtime" claim — Xtask explicitly runs benchmarks for `#[xtask::hot]` modules. The "no runtime" non-goal refers to executing the gated code's business logic, not to benchmark/profiling tools.

| Extension | Requirement | Honest caveat |
|---|---|---|
| Latency budget | Named Criterion benchmark passing p50/p95/p99 threshold | NONDETERMINISTIC — requires hardware spec, load, warmup, sample size, noise threshold, baseline. Without these, CI is "a slot machine wearing a lab coat." |
| Throughput budget | Named benchmark passing ops/sec threshold | Same nondeterminism caveat. |
| Allocation budget | heaptrack/DHAT evidence showing count within budget | Runtime instrumentation, not static. |
| Storage placement | measured stack/heap/arena choice | Architectural review, not a lint. |
| SIMD discipline | scalar oracle + fallback + target gate + benchmark | unsafe SIMD needs explicit waiver (§6.6). |

**Benchmarking tools Xtask wraps:** Criterion (`criterion = "0.8"`), iai-callgrind (`iai-callgrind = "0.16"` — deterministic instruction counts), hyperfine, perf stat, cargo flamegraph/samply, heaptrack/DHAT/bytehound, cachegrind, cargo bloat, cargo llvm-lines, tokio-console. **No benchmark = `PERF_NO_BENCHMARK` blocker.** Generic `cargo bench` is discovery only — a named target is required.

### 6.5 Supply-chain (honest about tool limits)

| Tool | What it does | What it does NOT do |
|---|---|---|
| `cargo audit` | Checks Cargo.lock against KNOWN RustSec advisories | Does NOT catch unknown/undisclosed vulnerabilities. |
| `cargo deny check` | Advisories + licenses + bans + sources + duplicate versions | Overlaps cargo-audit on advisories (intentional defense-in-depth). |
| `cargo vet` | Checks third-party deps have audits from trusted entities; reports gaps | NOT a correctness proof. Bootstrap cost is real (first adoption needs human audits or exemptions). |
| `cargo geiger` | Counts `unsafe` in dependency tree | Counts, does not prove soundness. |
| `cargo machete` | Detects unused dependencies | EXPLICITLY IMPRECISE per its README — will false-positive. Requires baseline/triage, not blanket reject. |
| `cargo hack --feature-powerset` | Every feature combo compiles | Does NOT prove all TARGET-specific code compiles (feature ≠ target). |

Supply-chain checks distinguish runtime, build, dev, and proc-macro dependencies — dev/build/proc-macro deps execute in CI even if they don't ship.

### 6.6 Unsafe policy (decided: zero first-party unsafe, waivers through ledger)

v1 is **zero first-party `unsafe`** (`unsafe_code = forbid`). There is NO SIMD waiver, NO FFI safe-wrapper exception, NO raw-pointer accounting in v1. If a genuine FFI/SIMD need arises, it requires a policy-PR that adds the crate to the trusted-base ledger with owner + reason + compensating evidence, and `unsafe_code` is lifted to `deny` (not `forbid`) for that specific crate via a scoped lint config. The default remains forbid.

### 6.7 Pinned toolchain (corrected — version-pinned, not floating)

- `rust-toolchain.toml` with a **version-pinned** channel. "Stable" floats — use an explicit `1.x.y` pin, OR a date-pinned `nightly-YYYY-MM-DD`.
- `portable_simd` and `try_blocks` are **nightly-only unstable features** — they CANNOT be allowed on a stable-pinned toolchain. If the project pins stable, these are not available. If nightly is required, pin the date and allow only these features via `-Zallow-features`.
- Components: `rustfmt`, `clippy`, `rust-src`, `llvm-tools-preview`.
- `RUSTC_BOOTSTRAP` = policy violation.
- `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER` are scanned and constrained (§12).

### 6.8 Strict clippy summary

See §6.2. Key corrections from v2: critical lints use `-F` (forbid) not `-D` (deny) so `#[allow]` cannot override them; `non_exhaustive_omitted_patterns` is the correct lint name; `fn_params_excessive_bools` replaces nonexistent `fn_args_justly`; `#[allow]`/`#[expect]` scan added as anti-bypass.

### 6.9 Allowed library & crate policy

Curated allowlist enforced via `cargo deny` + clippy `disallowed_methods`/`disallowed_types` + semgrep. Adding a non-approved crate requires a policy-PR.

| Purpose | Approved | Banned |
|---|---|---|
| Async I/O | `tokio` | — |
| HTTP | `axum`, `tower`, `tower-http`, `hyper` | — |
| CPU parallelism | `rayon` (scaling evidence required) | — |
| Concurrency | `crossbeam-channel`, `parking_lot`, `flume` (bounded) | `std::sync::Mutex` in async scope |
| Buffers | `bytes`, `arrayvec`, `smallvec`, `heapless` | — |
| Maps | `hashbrown`, `ahash`, `rustc-hash` (internal keys only) | fast non-crypto hasher for adversarial keys |
| Formats | `postcard`, `serde_json` | new `bincode` usage |
| Errors | `thiserror` (core), `anyhow` (shell) | `Result<T, String>` in core |
| Parsing | `winnow`, `nom`, `lexical-core` | — |
| Hashing | `blake3`, `crc32fast` | `chrono::Local`, raw `rand::random()` |

---

## 7. Two-Environment Architecture (signing-key isolation — CRITICAL)

**THE SIGNING KEY MUST NEVER EXIST IN ANY ENVIRONMENT THAT EXECUTES A CARGO COMMAND ON UNTRUSTED CODE.**

```
┌─────────────────────────────────────────────────────────────┐
│ VERIFIER ENVIRONMENT (untrusted code executes here)          │
│                                                              │
│  Sandbox: network-off, readonly source, frozen PATH,        │
│  no secrets, CPU/mem limits, fixed env/locale/timezone       │
│                                                              │
│  Runs: cargo check/clippy/test/mutants/hack, semgrep,        │
│        rg, benchmarks, profilers                             │
│                                                              │
│  HAS NO SIGNING KEY                                          │
│                                                              │
│  OUTPUTS: canonical evidence artifacts + evidence digest     │
└──────────────────────┬──────────────────────────────────────┘
                       │ canonical evidence only (no live checkout)
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ SIGNER ENVIRONMENT (hardened, no code execution)             │
│                                                              │
│  Receives ONLY: evidence digest, source digest, artifact     │
│  digest, policy digest, advisory-db digest                   │
│                                                              │
│  HAS THE SIGNING KEY (KMS/HSM or keyless CI identity)        │
│                                                              │
│  OUTPUTS: signed Attestation                                 │
└─────────────────────────────────────────────────────────────┘
```

The signer NEVER receives a live checkout. It receives digests and canonical evidence. It cannot execute code. An attacker who compromises the verifier environment cannot sign — the key isn't there.

---

## 8. The Enforcement Lanes (corrected skip rules + build.rs lane)

| Lane | Tool(s) | Scope | Depends on compile? | Skip rule |
|---|---|---|---|---|
| 0 | `cargo fmt --check` | edit | No | always runs |
| 1 | `cargo check` + rustc lints (`-D warnings`, `-F unsafe_code`, deny `unused_must_use`/`unused_results`/exhaustiveness) | edit | — | always runs |
| 2 | `cargo clippy` (all groups maxed, §6.2; source-only `--lib --bins`; NOT `--examples`) | edit | Yes | skip if L1 fails |
| 3 | `semgrep` (.xtask/semgrep/) | edit | **No** | **always runs** (doesn't need compilation) |
| 4 | assert-macro scan (`rg`) | edit | No | always runs |
| 4b | **build.rs lane** — stricter scan of build scripts (they EXECUTE during builds) | edit | No | always runs |
| 5 | supply chain: `cargo audit` + `cargo deny` + `cargo vet` + `cargo geiger` + `cargo machete` (with triage baseline) | prepush | No | always runs in prepush+ |
| 6 | feature correctness: `cargo hack --feature-powerset` | prepush | **Yes** | skip if L1 fails |
| 7 | mutation testing: `cargo mutants` (with baseline + equivalent-mutant triage) | full | Yes | skip if L1 fails |
| 8 | tests: `cargo test` / `cargo nextest run` (plain deterministic tests are a FIRST-CLASS lane) | edit | Yes | skip if L1 fails |
| 9 | benchmark gate (for `#[xtask::hot]` modules only) | full | Yes | skip if L1 fails; N/A if no hot modules |

**Corrected from v2:**
- Lane 3 (semgrep) runs on source REGARDLESS of compilation — it does not need the crate to typecheck. Skipping it on compile failure reduces repair signal.
- Lane 6 (feature-powerset) depends on compilation — skip if L1 fails, not "always run fast."
- Lane 2 is `--lib --bins` only (NOT `--examples` — examples are not clippy-gated for style, but DO compile).
- **build.rs gets its own lane (4b)** — build scripts EXECUTE during builds and are attacker-controllable. They are NOT excluded from scanning.
- `cargo test` is an explicit first-class lane (was missing in v2).

---

## 9. Sandbox Profile (the verifier environment)

Every Cargo/semgrep/rg/benchmark command in the verifier runs under:

| Control | Value | Why |
|---|---|---|
| Network | OFF (no outbound) | Determinism; no dependency fetching, no advisory DB live-fetch |
| Source tree | READ-ONLY | Build scripts cannot mutate source; compare source digest before/after |
| Writable | `target/`, `OUT_DIR`, temp only | Build artifacts only |
| Secrets | ABSENT | No signing key, no tokens, no env secrets |
| PATH | FROZEN — resolved absolute tool paths from a trusted toolchain dir | No PATH poisoning / tool shadowing |
| Env vars | Fixed, frozen, digest-bound | `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_NET_OFFLINE=true`, `--locked`, `--frozen` |
| Locale | Fixed (C/POSIX) | Deterministic output sorting |
| Timezone | Fixed (UTC) | Deterministic timestamps in tool output |
| CPU/Memory | Cgroup-capped (Linux) / container limits | Kani/CBMC can OOM; mutants can run away |
| `.cargo/config.toml` | Included in policy digest, constrained | It can set build flags, wrappers, aliases — it's a policy surface |

**Network-off means:** dependencies must be vendored or `--frozen`/`--locked`. Advisory DBs must be pinned snapshots (their digests in the certificate), not live-fetched. cargo-vet uses `--locked`/`--frozen` modes.

---

## 10. Certificate Model (artifact-bound, fresh, split deterministic/signed)

### 10.1 EvidenceDigest (deterministic — identical input produces identical digest)

```rust
pub struct EvidenceDigest {
    pub schema_version: u16,
    pub source_digest: Digest,              // blake3 of gated source tree (canonical paths)
    pub cargo_lock_digest: Digest,
    pub dependency_source_digest: Digest,    // vendored dep tree digest (Cargo.lock is NOT the source tree)
    pub artifact_digest: Option<Digest>,     // blake3 of the built binary/artifact (for deploy-bound certs)
    pub build_profile: Option<String>,       // "release" / "dev"
    pub target_triple: Option<String>,
    pub feature_set: Option<Box<str>>,
    pub per_lane: Box<[LaneDigest]>,         // each lane's evidence digest
    pub policy_digest: Digest,               // blake3 of ALL policy files (§11)
    pub toolchain_digest: Digest,            // hash of RESOLVED BINARIES or hermetic container/OCI digest (not version strings)
    pub advisory_db_digest: Digest,          // pinned RustSec/deny DB snapshot digest
    pub cargo_vet_db_digest: Digest,         // pinned vet imports snapshot digest
    pub env_digest: Digest,                  // frozen env vars
    pub hot_module_set_digest: Digest,
}
```

`toolchain_digest` hashes the **resolved binaries** (or uses a hermetic Nix/OCI image digest), not version strings — version strings are not supply-chain integrity.

### 10.2 Attestation (non-deterministic, SIGNED — detached over canonical pre-sign payload)

```rust
pub struct Attestation {
    pub evidence_digest_hash: Digest,    // blake3 of canonical-serialized EvidenceDigest
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,         // FRESHNESS — old certs expire
    pub policy_epoch: u64,                // increments on policy change; deploy checks current epoch
    pub key_id: String,                   // which key signed (for rotation/revocation)
    pub scope: Scope,                     // edit | prepush | full — deploy requires full
    pub signature: Vec<u8>,               // Ed25519 detached signature over evidence_digest_hash + fields above
}
```

The signature is DETACHED over a canonical pre-sign payload (the fields above, canonically serialized). The signature does not sign itself.

**Freshness/revocation:** `not_after` expiry. `advisory_db_digest` in the evidence — if the DB changed since issuance, deploy REJECTS (a dependency may have gained an advisory). `policy_epoch` — if policy evolved, old certs from a prior epoch are stale. `key_id` enables key rotation/revocation.

### 10.3 Scope conflation fix

An attestation encodes `scope`. Deploy-gate REQUIRES `scope = full`. A `scope = edit` attestation (from the fast loop) is NOT deploy-acceptable regardless of signature.

### 10.4 Canonical serialization

JSON map ordering, path separators, absolute paths, ANSI output, locale, and tool-output order can change digests. Xtask uses: **canonical JSON** (sorted keys, no whitespace), **sorted findings** (by lane, rule_id, location), **normalized paths** (workspace-relative, forward-slash), **scrubbed volatile fields** (durations, timestamps stripped from evidence digests), **fixed locale** (C/POSIX), **fixed timezone** (UTC).

---

## 11. Policy Files (what's in policy_digest)

ALL of these are hashed into `policy_digest`. Changing ANY requires a policy-PR:

```
clippy.toml
rustfmt.toml
.xtask/semgrep/           (all rules)
deny.toml
rust-toolchain.toml
cargo-vet supply-chain/   (vet policy + imports)
.xtask/policy.toml        (xtask policy manifest: hot-module sets, scope config, budgets)
.cargo/config.toml        (build flags/wrappers — constrained)
Cargo.toml [lints]        (workspace lint config)
```

---

## 12. Bypass-Surface Countermeasures (comprehensive)

The AI author is adversarial. Every known bypass vector and its countermeasure:

| Bypass vector | Countermeasure |
|---|---|
| `#[allow(...)]` / `#[expect(...)]` | rg/semgrep scan; un-ledgered = CodeReject. Critical lints use `-F` (forbid) so `#[allow]` cannot lower them. |
| `cfg_attr(..., allow(...))` | same scan |
| `#![allow(...)]` / `#![expect(...)]` (crate-level) | same scan |
| `.cargo/config.toml` (build flags, wrappers) | included in policy_digest; constrained; scanned for `runner`/`rustc-wrapper` overrides |
| `RUSTFLAGS` / `CARGO_ENCODED_RUSTFLAGS` | scanned; unexpected flags rejected; frozen in env_digest |
| `RUSTC_WRAPPER` / `RUSTC_WORKSPACE_WRAPPER` | scanned; rejected unless explicitly policy-approved |
| PATH poisoning / tool shadowing | frozen absolute tool paths from trusted toolchain dir |
| build scripts (`build.rs`) | own lane (4b); stricter scan; readonly source enforced; source digest compared before/after |
| proc macros (execute during compilation) | acknowledged: proc-macro crates execute attacker code at compile time. Mitigated by: signing key NOT in verifier env (§7); proc-macro deps in supply-chain scan |
| `include!` / `include_str!` / `include_bytes!` / `#[path]` | canonical module/input discovery; symlink handling; rule forbidding source-tree escapes; included files in source_digest |
| generated `OUT_DIR` code | `include!(concat!(env!("OUT_DIR"), ...))` scanned and ledgered; generated code is not source-of-truth |
| `cfg`-gated code (platform-specific) | semgrep parses all cfg branches conservatively; feature-powerset checks feature cfgs; target-specific code flagged (target powerset ≠ feature powerset) |
| `#[cfg(test)]` modules in `src/*.rs` | not falsely treated as production by path-only exclusion; semantic cfg-aware scanning |
| semgrep ignore comments | scanned; un-ledgered = CodeReject |
| cargo-audit `--ignore` / cargo-deny exceptions | scanned in config; un-ledgered = CodeReject |
| cargo-vet exemptions | scanned in supply-chain/; un-ledgered = CodeReject |
| cargo-mutants `--exclude` | scanned; un-ledgered = CodeReject |
| dependency tricks (renamed crates, git deps, path deps) | cargo-deny bans + dependency_source_digest |
| macro-expanded code (semgrep/rg can't see it) | KNOWN LIMITATION: semgrep/rg over source CANNOT see loops/panics/allocations/unsafe introduced by proc macros or derives. Stated honestly. Partial mitigation: clippy DOES see macro-expanded code. |
| empty proof obligations | (proofs deferred to future phase) — when added, coverage rules will require obligation existence per policy |

---

## 13. Escape Hatch (anti-circular policy)

**NO per-site bypass.** The only escape is editing policy files (§11). BUT:

**Anti-circularity:** a policy-PR is checked against the **PREVIOUS main-branch policy** (not the weakened policy being introduced). A meta-policy requires:
- **CODEOWNER approval** on any policy file change.
- **Explicit diff classification** (tightening / loosening / neutral). Loosening changes are flagged for human review.
- The policy-PR itself must pass the gate under the PREVIOUS policy.

This prevents: AI weakens policy → weakened policy judges itself → passes → ships.

---

## 14. Deploy-Gate (corrected)

The deploy-gate is a Moon task that runs before deploy. It checks ALL of:

1. **Attestation present** and `scope = full`.
2. **Signature valid** under `key_id` (and key not revoked).
3. **Fresh:** `not_before ≤ now ≤ not_after`.
4. **Evidence matches:** recompute `EvidenceDigest` from the actual artifact + source + policy + toolchain + advisory-db + env. Compare against `evidence_digest_hash`. Mismatch → REJECT.
5. **Advisory DB current:** `advisory_db_digest` matches the current pinned DB snapshot. If the DB changed (new advisory), → REJECT (re-run the gate).
6. **Policy epoch current:** attestation's `policy_epoch` matches current. Stale epoch → REJECT.
7. **Artifact-bound:** if deploying a binary, `artifact_digest` must match the actual binary (reproducible rebuild or signed artifact).
8. **If the deploy-gate itself cannot run** → REJECT (fail-closed).

Deploy-gate is an EXPLICIT dependency of the deploy target — it does not get skipped by Moon's affected-target logic.

---

## 15. Moon CI/CD Integration (corrected inputs)

```yaml
# .moon/tasks/all.yml
gate-edit:
  command: 'xtask gate --scope edit --emit json --out target/xtask/report.json'
  toolchains: [rust]
  options: { runInCI: true }
  inputs:
    - '@globs(sources)'
    - '.xtask/**'
    - 'Cargo.toml'
    - 'Cargo.lock'
    - '**/Cargo.toml'           # workspace manifests
    - '.cargo/**'               # build config — policy surface
    - 'rustfmt.toml'
    - 'clippy.toml'
    - 'deny.toml'
    - 'supply-chain/**'         # cargo-vet
    - 'rust-toolchain.toml'
  outputs: ['target/xtask/report.json', 'target/xtask/evidence.json']

gate-full:
  command: 'xtask gate --scope full --emit json --out target/xtask/report.json'
  # same inputs + fuzz corpora, test seeds, mutant baselines
  inputs:
    - '@globs(sources)'
    - '.xtask/**'
    - 'Cargo.toml'
    - 'Cargo.lock'
    - '**/Cargo.toml'
    - '.cargo/**'
    - 'rustfmt.toml'
    - 'clippy.toml'
    - 'deny.toml'
    - 'supply-chain/**'
    - 'rust-toolchain.toml'
    - '.xtask/mutants-baseline.json'
    - '.xtask/advisory-db-snapshot/'
    - '.xtask/fuzz-corpus/'

deploy-gate:
  command: 'xtask verify-attestation target/xtask/attestation.json --require-signature --require-scope full'
  options: { runInCI: true }
```

**Moon cache caveat:** Moon caching replays outputs when declared inputs match. Missing inputs become a trust bug — the input list above is comprehensive. Deploy-gate recomputes against the CURRENT deployment request, not a cached cert.

---

## 16. Error Taxonomy (corrected)

### Report root (disjoint)
`Pass | CodeReject | GateReject | PolicyError | InputError`

### Lane failures (`GateReject` — infra)
`ToolMissing | ToolCrashed | ToolTimeout | ToolVersionMismatch | ToolPathPoisoned | PanicInGate`

### Rule families (`RuleId`)
- `HOLZMAN_PANIC_*`, `HOLZMAN_UNSAFE_*`, `HOLZMAN_CHECKED_*`
- `FUNC_LOOPS_*`, `FUNC_NESTING_*`, `FUNC_STYLE_*` (style, not panic-safety)
- `SUPPLY_*` (advisory/banned/unsafe-dep/unused-dep/vet-gap)
- `MUTANT_*` (with triage: real-mutant vs equivalent-mutant vs irrelevant)
- `PERF_*` (no-benchmark / regression / allocation-over)
- `BYPASS_*` (allow-attribute / cfg-attr-allow / semgrep-ignore / cargo-ignore / tool-wrapper / source-escape / out-dir-include)
- `POLICY_*` (malformed policy / circular-policy-change)

### Severity
If all findings reject, there is no "Warning" severity — findings either reject or they don't. `Severity` is removed from the model. A finding either causes `CodeReject` or is informational (not a finding). No decorative severity.

### Repair-path honesty
The three buckets (code/gate/policy) are the REPORT-level split. Individual findings CAN have mixed repair paths (a cargo-audit finding may need a dep update OR a policy exception OR a code change). The `RepairHint` enum carries the specific action; the report-level bucket is the routing decision, not a claim that every finding has exactly one fix path.

---

## 17. Second-Order & Pre-Mortem (corrected)

**Key-leak mitigation (CORRECTED from v2):** "digest recompute" does NOT mitigate a leaked key — an attacker can sign a malicious artifact whose digests match that malicious artifact. Real mitigation: key revocation, KMS/HSM or keyless CI identity (Sigstore), transparency logging (Rekor), branch-protected signing policy, and separation from untrusted execution (§7).

**3 AM disaster (most likely):** A logical correctness bug that passes all lints/tests/mutations but is semantically wrong. Xtask enforces DISCIPLINE against a pinned policy — it does not prove the algorithm is correct. The attestation records which lanes ran, so incident response knows the quality floor.

**Macro-expanded code blind spot:** semgrep/rg cannot see code generated by proc macros or derives. Clippy CAN see it. This is a known residual — a proc macro could generate an `unwrap()` that semgrep misses but clippy catches (if the lint applies post-expansion).

**Build-script mutation:** a build script could modify the source tree during compilation. Mitigated by readonly source (§9) + source-digest comparison before/after.

---

## 18. The Honest Trust Boundary (corrected — no overclaiming)

Xtask makes it mechanically impossible to ship first-party Rust that violates any **decidable property in the pinned policy** — panic-surface constructs, unsafe, indexing, arithmetic side-effects, supply-chain advisories, feature-combo compilation, and mutation resistance (with triage).

**What Xtask does NOT guarantee:**
- Behavioral correctness (the algorithm is right) — no static tool proves this.
- All UB freedom — Miri observes particular executions only; Kani can run out of resources.
- Macro-expanded code quality — semgrep/rg are blind to proc-macro output (clippy partially covers).
- Unknown vulnerabilities — cargo-audit checks KNOWN advisories only.
- Concurrency soundness — Loom requires deterministic tests using Loom sync types; it cannot prove arbitrary concurrent Rust.
- That `unwrap_or` is a panic risk — it isn't; banning it is house style.

The attestation is a **quality floor for a pinned policy**, not a correctness proof or omniscience claim.

---

## 19. Component / Module Map

Single Cargo workspace (decomposer refines):
- `xtask-bin` — CLI (clap). Subcommands: `gate`, `verify-attestation`, `doctor`.
- `xtask-core` — domain types: `Report`, `Finding`, `Lane`, `RuleId`, `RepairHint`, `Location`, `LaneOutcome`, `SkipReason`, `LaneFailure`, `LaneEvidence`.
- `xtask-policy` — policy loading, validation, policy_digest, hot-module sets, meta-policy (CODEOWNER, diff classification).
- `xtask-lanes` — lane runners: `fmt`, `rustc`, `clippy`, `semgrep`, `assert_scan`, `build_rs_scan`, `supply`, `feature`, `mutants`, `test`, `benchmark`.
- `xtask-sandbox` — sandbox enforcement: network-off, readonly source, frozen PATH, fixed env, cgroup caps.
- `xtask-evidence` — canonical serialization, `EvidenceDigest` computation, `LaneEvidence`.
- `xtask-signer` — `Attestation` signing/verification, Ed25519, canonical pre-sign payload, key_id, revocation.
- `xtask-bypass` — bypass-surface scans (§12): allow-attribute scan, cfg-attr scan, tool-wrapper scan, source-escape scan.
- `xtask-output` — report JSON schema (versioned), `doctor` diagnostics.

All first-party crates pass their own gate (dogfooded).

---

## 20. CLI Surface

```
xtask gate [--input <crate|diff>] [--scope edit|prepush|full] [--emit json] [--out <path>]
    Run scoped lanes. Emit report JSON + exit code. Emit evidence artifacts on Pass.

xtask verify-attestation <attestation.json> [--require-signature] [--require-scope full]
    Deploy-gate: recompute digests, verify signature, check freshness/epoch/advisory-db.

xtask doctor [--scope <scope>]
    Report required tools for the CURRENT scope/policy (not all tools blindly).
    Fail-closed health report.
```

Exit codes: `0` Pass, `1` CodeReject, `2` GateReject, `3` PolicyError, `4` InputError, `>=5` internal.

---

## 21. Definition of Done (v1)

1. `xtask gate --scope edit` runs lanes 0–4b+8 and emits the disjoint `Report` JSON.
2. `xtask gate --scope full` adds supply/feature/mutants/benchmark lanes.
3. Two-environment architecture: verifier sandbox runs all Cargo commands with NO signing key; signer signs canonical evidence only.
4. `EvidenceDigest` (deterministic) + `Attestation` (signed, fresh, scope-encoded, artifact-bound) replace the old monolithic certificate.
5. Deploy-gate checks: signature, freshness (`not_after`), advisory-db-digest, policy-epoch, artifact-digest, scope=full.
6. Any unrunnable scoped lane → `GateReject` (fail-closed).
7. Full Holzman panic-free + functional-rust doctrine encoded with TOOL-CORRECT lint names.
8. Critical lints use `-F` (forbid); `#[allow]`/`#[expect]` bypass scan active.
9. Anti-circular policy: policy-PRs checked against PREVIOUS main policy + CODEOWNER approval.
10. Sandbox: network-off, readonly source, frozen PATH, no secrets.
11. Bypass-surface scans (§12) active for all known vectors.
12. Xtask's own source passes its own gate.
13. Killer demo: AI writes Rust with `.unwrap()` + `for` loop → `xtask gate --scope edit` rejects with `Patch`/`UseIteratorPipeline` hints → AI fixes → gate passes → full gate passes → attestation signed in signer env → deploy-gate accepts.

---

## 22. v2 → v3 Issue Mapping (the black-hat review, addressed)

**FATAL security:**
- Signing key in untrusted env → §7 two-environment split, key NEVER in verifier.
- Certificate not artifact-bound → §10.1 `artifact_digest`, build profile, target triple, feature set.
- `timestamp_utc` breaks determinism → §10 split `EvidenceDigest` (deterministic) + `Attestation` (non-det, signed, detached).
- No freshness/revocation → §10.2 `not_before`/`not_after`/`advisory_db_digest`/`policy_epoch`/`key_id`.
- Key-leak mitigation wrong → §17 corrected: revocation, KMS/HSM/keyless, transparency logging.
- Policy circular → §13 anti-circular: previous-policy + CODEOWNER + diff classification.
- Command injection via proof-obligations → proofs deferred; when added, closed schema `{verifier, package, target, harness, bounds, flags}`, no command strings.
- "AI cannot route around" → §12 comprehensive bypass countermeasures; §1 adversarial threat model.

**Structural:**
- Root enum → §5.1 `Report = Pass | CodeReject | GateReject | PolicyError | InputError`.
- `TopLevelError` not in root → folded into `Report`.
- `Skipped` underspecified → §5.2 four `SkipReason` variants.
- Warning severity meaningless → §16 removed; findings reject or they're informational.
- Certificate.signature: Option awkward → §10.2 detached signature over canonical pre-sign payload.
- Span mandatory → §5.3 `Location` sum type.
- RepairHint lifetimes → §5.4 serializable `#[derive(Serialize, Deserialize)]`.

**Contradictions:**
- "No runtime" → §6.4 honest: benchmarks ARE runtime; "no runtime" = no business-logic execution.
- "No macros" → §2 corrected: "no Xtask-specific authoring macros"; ordinary Rust macros allowed through policy.
- "Layers 0-6 always run" vs "L3 skipped" → §8 corrected: L3 (semgrep) runs regardless of compilation.
- L2 source-only vs --examples → §8 corrected: `--lib --bins` only.
- L4 excludes build.rs → §8 lane 4b: build.rs gets its own stricter lane.
- Fast cert vs full cert → §10.3 scope encoded in attestation; deploy requires full.
- Layer 9 vs 10 (reject vs ledger trust) → proofs deferred; when added, ledger DEGRADES claim, doesn't disappear.

**Tool correctness:**
- `non_exhaustive_patterns` → `non_exhaustive_omitted_patterns` (§6.2).
- `fn_args_justly` → `fn_params_excessive_bools` (§6.2).
- `clippy::all` insufficient → all groups denied + restriction (§6.2).
- `-W` not `-D` → critical lints use `-F` (forbid) (§6.2).
- `#[allow]` overrides `-D` → §6.2 `-F` + allow-scan.
- `--cap-lints allow` for deps → §6.2 stated: first-party source only.
- `unwrap_used` doesn't catch `unwrap_or*` → §6.2 separate denies; `unwrap_or` is STYLE not panic-safety.
- `panic_in_result_fn` can't prove no panics → §6.1 honest caveat.
- `unused_must_use` limited → §6.1 add `unused_results`.
- `portable_simd`/`try_blocks` nightly-only → §6.7 corrected: can't be on stable pin.
- "Pinned stable" floats → §6.7 version-pinned `1.x.y` or date-pinned nightly.
- `cargo machete` imprecise → §6.5 baseline/triage, not blanket reject.
- `cargo vet` not proof → §6.5 honest.
- `cargo audit` known-only → §6.5 honest.
- Kani not blanket proof → proofs deferred; honest caveats in §18.
- Miri not proof → §6.5/§18 honest.
- Loom intrusive → §18 honest.

**Static-analysis gaps:**
- Macro-expanded code blind spot → §12/§17 honest: semgrep blind, clippy partial.
- Regex false positives → §4 canonical scanning, cfg-aware.
- `#[cfg(test)]` in src → §12 cfg-aware scanning.
- cfg-gated code → §12 conservative parse; target ≠ feature powerset.
- `include!`/`#[path]` → §12 canonical discovery + source-escape ban.
- OUT_DIR code → §12 scanned and ledgered.
- `.cargo/config.toml` → §9/§11 policy surface, constrained, in digest.
- RUSTFLAGS/WRAPPER/PATH → §9/§12 scanned, frozen, rejected.
- Tool-native suppressions → §12 all modeled.

**Certificate/deploy:**
- No canonical serialization → §10.4 canonical JSON, sorted, normalized.
- LaneOutcome::Pass lacks evidence → §5.5 `LaneEvidence`.
- toolchain_digest vague → §10.1 resolved-binary hash or OCI digest.
- External DBs not pinned → §10.1 `advisory_db_digest`/`cargo_vet_db_digest`.
- Network breaks determinism → §9 network-off, `--locked`/`--frozen`, vendored.
- No dependency source digest → §10.1 `dependency_source_digest`.
- Moon cache replay → §15 comprehensive inputs; deploy recomputes.
- Moon affected-target → §14 deploy-gate explicit dependency.

**Operational:**
- Fast loop not fast → §3 scope tiers (edit/prepush/full).
- Time budgets fantasy → §3 SLOs scaled by crate size, not correctness gates.
- cargo vet bootstrap → §6.5 noted.
- cargo mutants equivalent-mutant → §6.5/§16 triage states.
- Tests not first-class → §8 lane 8 explicit.
- cargo check not a build → §10.1 artifact_digest + build for deploy-bound certs.
- Build scripts mutate source → §9 readonly source + digest compare.
- Signing key absent from Cargo jobs → §7 explicit invariant.
- Tool PATH frozen → §9.
- No sandbox → §9 full profile.
- No schema versioning → §5.4/§10 schema_version on report + evidence.
- PROOF_LAUUNDERING typo → §16 corrected to BYPASS_* / POLICY_* families.
- doctor policy-aware → §20 `--scope` aware.
- No threat model → §1.
- No adoption profile → honest: general Rust projects won't accept zero-loops/zero-unwrap_or/cargo-vet from day one; staged rollout is a future concern, not a v1 gate.

---

## 23. References (read in full)

- `holzman-rust/SKILL.md` + all 6 references (nasa-jpl-standards, latency-throughput-playbook, runtime-performance-architecture, zero-cost-abstractions, simd-patterns, mechanical-empathy-toolchain)
- `functional-rust/SKILL.md` + all 3 references (scott-ddd-types, typing-refactor-checklist, complete-workflow)
- `moon-v2/SKILL.md` — canonical `moon ci` gate
- Proof skills (read for context, deferred from v1): verus, kani, flux-rs, loom, miri, rust-fuzzer, formal-verifier, proof-planner, rust-contract
