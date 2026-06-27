# Architecture Spec: Xtask — The Deterministic Rust Quality Gate

> Status: ARCHITECTURE SPEC v5.0 — chain-of-custody hardened, phased
> v5 fixes: trusted-runner bootstrap, signer provenance, evidence isolation, artifact build lane, source-only clippy, `--frozen` hermeticity, mixed-failure Report, LaneEvidence split, canonical types, phased DoD.
> Next step: run `arch-spec-to-beads` to shred this into molecular tasks.

---

## 0. Product Sentence

```
Xtask is a deterministic Rust quality gate for AI-authored code. It runs a
pinned, hermetic toolchain over real Rust crates and emits a structured report
plus a signed attestation (CI only) when every policy-selected lane passes. It
enforces panic-surface discipline, unsafe restrictions, lint zero-tolerance,
functional-style structural rules, supply-chain hygiene, feature-matrix
compilation, tests, and mutation-resistance. It does not prove behavioral
correctness; it certifies conformance to a checked-in quality policy.
```

Xtask is not a theorem prover. It is a deterministic evidence gate with a chain of custody.

---

## 1. Threat Model

**The AI code author is potentially MALICIOUS.** Design assumes attempts via `#[allow]`, `.cargo/config.toml`, build scripts, proc macros, `include!`/`#[path]`, `cfg`-gated code, PATH poisoning, tool-native suppressions, generated code, dependency tricks, cache poisoning, and evidence tampering.

**The signing key is a crown-jewel secret.** Cargo executes repository-controlled code through build scripts, proc macros, tests, and mutation harnesses. The signing key NEVER exists in any environment that runs a Cargo command on untrusted code (§9).

**Xtask's own source is trusted but NOT self-approving.** A PR that modifies Xtask cannot be gated by the Xtask built from that PR — that is recursion wearing a badge (§7.1).

---

## 2. Non-Goals

- **NO Xtask-specific authoring macros or DSL.** Ordinary Rust macros allowed through policy.
- **NO LLM inside the gate.** AI is external; consumes report JSON.
- **NO formal verification, proofs, bounded model checking, refinement types, UB interpreters, or concurrency model checking in v1.**
- **NO bypass flag.** The only escape is a policy-PR checked against the PREVIOUS policy (§14).
- **NO claim of omniscience.** Xtask certifies conformance to a pinned policy.

---

## 3. Terminology (consistent throughout)

| Term | Meaning |
|---|---|
| **Evidence** | Deterministic, unsigned record: source/policy/toolchain/artifact/layer digests. |
| **Attestation** | Signed statement over Evidence (Ed25519, CI only). |
| **Certificate** | User-facing bundle = Evidence + Attestation. |
| **Report** | Per-invocation structured output (pass/reject + findings). Local, unsigned. |

---

## 4. Scope Tiers

| Scope | Lanes | Use case |
|---|---|---|
| `edit` | fmt, check, clippy (source-only), semgrep, panic/assert+build.rs scan | AI repair loop |
| `prepush` | edit + tests + supply chain + feature matrix | before push |
| `full` | prepush + artifact build + mutation testing | CI / deploy-gate |

```
edit     = fmt + check + clippy(source) + semgrep + panic/assert+build.rs scan
prepush  = edit + cargo test + supply chain + feature matrix
full     = prepush + artifact build + cargo mutants
```

---

## 5. Domain Model (corrected — mixed failures, canonical types)

### 5.1 Report — single disjoint root, can contain BOTH code findings AND gate failures

```rust
pub enum Report {
    Pass {
        evidence: EvidenceDigest,
        evidence_path: WorkspacePath,
        per_lane: Box<[LaneOutcome]>,
    },
    Reject {
        code_findings: Box<[Finding]>,
        gate_failures: Box<[LaneFailure]>,
        per_lane: Box<[LaneOutcome]>,
        kind: RejectKind,           // CodeOnly | GateOnly | Mixed
    },
    PolicyError { diagnostics: Box<[PolicyDiagnostic]> },
    InputError { diagnostics: Box<[InputDiagnostic]> },
}

pub enum RejectKind { CodeOnly, GateOnly, Mixed }
```

A real run CAN produce both: semgrep finds `#[allow]` AND cargo-mutants is missing. The Report carries both; `kind` tells the AI whether to fix code, report infra, or both.

### 5.2 Lane outcomes + skip reasons

```rust
pub enum LaneOutcome {
    Clean { evidence: DeterministicLaneEvidence, telemetry: ObservedLaneTelemetry },
    Findings(Box<[Finding]>),
    Failed(LaneFailure),
    Skipped(SkipReason),
}

pub enum SkipReason {
    PriorCompilationFailure,
    NotSelectedByScope,
    NotApplicable,
    PolicyDisabled,
}
```

### 5.3 Lane evidence SPLIT — deterministic vs telemetry

```rust
// Deterministic — part of the Evidence digest
pub struct DeterministicLaneEvidence {
    pub command: CommandEvidence,        // argv array, not shell string
    pub tool_digest: Digest,             // hash of resolved binary or OCI image layer
    pub env_digest: Digest,              // frozen env vars
    pub input_digest: Digest,            // digest of lane inputs (source, lockfile, etc.)
    pub parsed_result_digest: Digest,    // digest of parsed tool output (findings/result)
}

// Telemetry — in the Report, NOT in the Evidence digest
pub struct ObservedLaneTelemetry {
    pub duration_ms: u64,
    pub max_rss_bytes: u64,
    pub exit_status: ProcessTermination,
    pub stdout_excerpt: Option<String>,  // truncated, for debugging
    pub stderr_excerpt: Option<String>,
}
```

### 5.4 ProcessTermination (not just exit code)

```rust
pub enum ProcessTermination {
    Exited { code: i32 },
    Signaled { signal: i32 },
    TimedOut,
    MemoryLimitExceeded,
    SandboxViolation { reason: String },
    SpawnFailed,
}
```

### 5.5 CommandEvidence (argv, not shell string)

```rust
pub struct CommandEvidence {
    pub executable: AbsolutePath,        // resolved absolute path, no PATH lookup
    pub argv: Box<[String]>,
    pub cwd: WorkspacePath,
}
```

### 5.6 Canonical path/text types

```rust
/// Normalized UTF-8 workspace-relative path. No backslashes, no `..`, no symlink escape.
pub struct WorkspacePath(String);

/// Byte offsets over UTF-8 source bytes. Deterministic patching.
pub struct TextRange { pub start_byte: u32, pub end_byte: u32 }

/// Digest with algorithm tag + lowercase hex.
pub struct Digest { pub algorithm: DigestAlgorithm, pub hex: String } // Blake3, 64 chars
```

Line/column convention: 1-based lines, 0-based columns, counting Unicode scalar values. Byte offsets in TextRange are the canonical patch coordinate.

### 5.7 Location (sum type — many findings are not spans)

```rust
pub enum Location {
    Span { file: WorkspacePath, start: TextPos, end: TextPos },
    Dependency { crate_name: String, version: String },
    Manifest { file: WorkspacePath },
    Workspace,
    Tool { name: String, version: String },
    Artifact { digest: Digest },
}
```

### 5.8 Finding + RepairHint

```rust
#[derive(Serialize, Deserialize)]
pub struct Finding {
    pub lane: Lane,
    pub rule_id: RuleId,
    pub location: Location,
    pub message: String,
    pub repair: RepairHint,
}

#[derive(Serialize, Deserialize)]
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

### 5.9 LaneFailure categories (GateReject is not always "don't edit code")

```rust
pub enum LaneFailure {
    InfraFailure { tool: String, reason: String },           // missing binary, version mismatch
    ToolFailure { tool: String, termination: ProcessTermination }, // crashed on input
    ResourceFailure { tool: String, limit: String },         // timeout, memory cap
    SuspiciousFailure { tool: String, evidence: String },    // likely hostile input/build-script
}
```

`SuspiciousFailure` — the AI MAY need to edit code (a macro-expansion bomb, a build-script infinite loop). Not a blanket "don't edit code."

---

## 6. The Doctrine (Holzman + functional-rust — tool-correctness fixed)

### 6.1 Panic-free standard

`unsafe_code = forbid`. `unwrap`/`expect`/`panic`/`todo`/`unimplemented`/`unreachable!`/`dbg!` denied. Production `assert!`/`assert_eq!`/`assert_ne!` scanned (parser-backed for Rust source, rg as coarse prefilter only).

Honest residuals: division by zero, char/string boundary ops, third-party panics, `Drop` panics, dependency-internal panics, macro-expanded panics (semgrep blind). Clippy helps; it does not prove panic freedom.

### 6.2 Strict clippy (source-only, correct config placement)

**Critical: clippy runs source-only `--lib --bins`, NOT `--all-targets`.** `--all-targets` includes tests/benches/examples → would nuke normal test code using `unwrap`/`assert!`/loops. Tests compile via `cargo test` and are behavior-gated, NOT style-gated.

**Lint levels in `Cargo.toml [workspace.lints.*]`; thresholds in `clippy.toml`:**

```toml
# Cargo.toml — lint LEVELS
[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
cargo = { level = "deny", priority = -1 }
# restriction group: deny specific lints, not blanket-warn the whole group
unwrap_or_default = "deny"       # real lint
exit = "deny"
too_many_lines = "deny"          # threshold in clippy.toml
too_many_arguments = "deny"      # threshold in clippy.toml
default_numeric_fallback = "deny"
missing_errors_doc = "deny"
fn_params_excessive_bools = "deny"  # threshold in clippy.toml

[workspace.lints.rust]
unsafe_code = "forbid"
unused_must_use = "deny"
unused_results = "warn"
non_exhaustive_omitted_patterns = "deny"   # CORRECT name
rust_2018_idioms = { level = "deny", priority = -1 }
```

```toml
# clippy.toml — THRESHOLDS only
too-many-lines-threshold = 40
too-many-arguments-threshold = 5
max-fn-params-bools = 1
```

**Critical lints passed as `-F` on the command line (forbid — `#[allow]` cannot lower):**
```
cargo clippy --workspace --lib --bins --frozen -- \
  -F clippy::unwrap_used -F clippy::expect_used -F clippy::panic \
  -F clippy::panic_in_result_fn -F clippy::todo -F clippy::unimplemented \
  -F clippy::indexing_slicing -F clippy::string_slice -F clippy::get_unwrap \
  -F clippy::arithmetic_side_effects -F clippy::dbg_macro \
  -D warnings
```

**`unwrap_or`/`unwrap_or_else` are NOT valid clippy lint IDs.** A blanket ban on `.unwrap_or*` requires a semgrep rule, not a fake clippy lint. `unwrap_or_default` IS a real lint. Corrected.

**`restriction` group is NOT blanket-warned.** Specific restriction lints are individually denied. No decorative "warn" that has no semantics in the report model (findings reject or are informational — no middle ground).

**`#[allow]`/`#[expect]` scan:** parser-backed scan for `#[allow(...)]`, `#[expect(...)]`, `#![allow(...)]`, `#![expect(...)]`, `cfg_attr(..., allow(...))`, `#[allow_internal_unstable]`, `#[allow_internal_unsafe]`, AND Cargo `[lints]` sections that lower required lints. Any un-approved suppression = `BYPASS_*` finding.

**`--cap-lints allow` for deps:** Cargo caps lints for dependencies. Regime is first-party source only. Stated explicitly.

### 6.3 Functional-rust doctrine (honest about decidability)

Same as v4 §6.3 — house style rules enforced by semgrep, with honest caveats about undecidability (no hidden I/O, no recursion, parse-don't-validate, zero-copy are NOT hard gates). No wildcard arms has exceptions for `#[non_exhaustive]` external enums.

### 6.4 Supply-chain (honest, operationally separated)

Same tool table as v4. Additions:
- **Conflict resolution:** any advisory from either cargo-audit OR cargo-deny rejects. Duplicates normalized by advisory ID.
- **cargo-machete baseline:** ignore/baseline config is a policy file with owner/reason/expiry per entry.
- **cargo-geiger thresholds by dependency class:** runtime/build/proc-macro = 0 unsafe fns; dev = policy-defined. Proc-macro deps execute during compilation — stricter.
- **cargo-vet modes:** `audit = "enforce" | "report" | "bootstrap"`. Deploy requires `enforce`. Onboarding uses `bootstrap`.

### 6.5 Unsafe policy

v1: zero first-party `unsafe` (`forbid`). Dependency unsafe measured (geiger), not forbidden by rustc. No SIMD/FFI waiver in v1.

### 6.6 Pinned toolchain + hermeticity

- `rust-toolchain.toml` with version-pinned channel (`1.x.y` explicit or `nightly-YYYY-MM-DD`).
- **Do NOT invoke rustup shims.** Resolve absolute `cargo`/`rustc` binaries from the trusted toolchain image. Call those directly. This defeats `RUSTUP_TOOLCHAIN` and rustup directory overrides.
- **`CARGO_HOME`** set to a controlled, read-only, digest-bound directory. **`RUSTUP_HOME`** same.
- **Reject parent-directory cargo configs:** Cargo searches parent dirs and `$CARGO_HOME/config.toml`. Xtask runs from canonical workspace root, sets `CARGO_HOME`, and rejects configs outside the workspace.
- `.cargo/config` (extensionless, legacy) AND `.cargo/config.toml` both checked.
- `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER` scanned, frozen, rejected unless policy-approved.
- `RUSTC_BOOTSTRUP` = violation.

### 6.7 Allowed library policy (profile-scoped, not universal)

The approved/banned crate table is a **named policy profile** (`server-strict`), NOT baked into the core product. Core Xtask supports crate allowlists; the default profile is small. `std::sync::Mutex` is enforced contextually (clippy `await_holding_lock`), not as a blanket crate ban. `chrono::Local` is a determinism-API rule, not a hashing rule.

### 6.8 Test & Mutation Evidence

Same as v4 §6.8. Additions:
- `cargo test --workspace --frozen -- --test-threads=1` for deterministic harness config.
- **TEST_NONDETERMINISTIC** emitted only for explicit known cases (Option C from review): random seed missing, proptest seed missing, test thread count > 1, wall-clock access scan. Xtask cannot prove determinism from one run.
- "Tests must run with fixed seeds" is **guidance**, not enforced (Xtask doesn't know about proptest/quickcheck internals unless policy declares).
- **Doctests** run by `cargo test` by default — state whether allowed/disabled/sandboxed (v1: allowed, same sandbox as tests).
- cargo-mutants config (`.cargo/mutants.toml`) included in policy digest.

---

## 7. Trusted-Runner Bootstrap (anti-self-gating — CRITICAL)

**A PR that modifies Xtask's own source CANNOT be gated by Xtask built from that PR.**

```text
PRs are gated by a trusted Xtask runner image built from the protected main
branch, not by Xtask code from the PR itself.

Changes to Xtask's own source are checked by Xtask N-1 (the trusted runner).
The newly built Xtask binary is promoted to trusted-runner status only after:
  1. protected-branch merge
  2. CODEOWNER review
  3. successful self-check under the previous trusted runner

The trusted Xtask runner image digest is bound into Evidence.
```

**Gate-control surface** (all require CODEOWNER + previous-policy evaluation, not just `clippy.toml`):
```
.xtask/**  .moon/**  .github/workflows/**  Cargo.toml [workspace.lints]
rust-toolchain*  .cargo/**  xtask crates  signing config  sandbox config
```

---

## 8. The Enforcement Lanes

| Layer | Tool(s) | Scope | Depends on compile? | Rejects |
|---|---|---|---|---|
| 0 | `cargo fmt --check` | edit | No | formatting drift |
| 1 | `cargo check --workspace --frozen` + rustc lints | edit | — | compile errors, denied lints |
| 2 | `cargo clippy --workspace --lib --bins --frozen` with `-F` critical lints (source-only; NOT `--all-targets`) | edit | Yes | denied lints |
| 3 | semgrep / structural source rules + `#[allow]` scan | edit | **No** | structural violations, bypass attributes |
| 4 | production panic/assert scan (parser-backed) + build-script scan (manifest-declared, not just `build.rs`) | edit | No | production panic macros; build-script violations |
| 5 | `cargo test --workspace --frozen -- --test-threads=1` | prepush | Yes | failing tests |
| 6 | supply chain: `cargo audit` + `cargo deny` + `cargo vet` + `cargo geiger` + `cargo machete` | prepush | No | advisories, bans, licenses, vet gaps, unsafe-dep, unused deps |
| 7 | `cargo hack check --workspace --feature-powerset --frozen` (bounded-depth per policy) | prepush | Yes | broken feature combination |
| 8 | **artifact build**: `cargo build --workspace --release --frozen --target <t> --features <f>` (policy-declared from closed schema) | full | Yes | build failure; produces artifact_digest |
| 9 | `cargo mutants` (with `.cargo/mutants.toml` baseline) | full | Yes | surviving non-baselined mutants |

**`--frozen` everywhere, not `--locked`.** `--locked` only asserts lockfile consistency; `--frozen` = `--locked` + `--offline` (no network).

**Skip rules:** compilation-dependent lanes (2, 5, 7, 8, 9) skip if Layer 1 fails. Lanes 0, 1, 3, 4 always run. Layer 3 (semgrep) runs on source regardless of compilation.

**Feature matrix policy:** full powerset can explode. Policy declares `mode = "powerset" | "bounded-depth" | "declared"` with depth/grouping/exclusions. Certificate records the mode. No silent "some feature combos."

**cargo-hack modes that modify manifests (`--no-dev-deps`, `--no-private`):** banned — conflict with readonly source and deterministic source digest.

**Generated OUT_DIR code:** `include!(concat!(env!("OUT_DIR"), ...))` is BANNED in v1. If build-time generation is needed (tonic/prost/bindgen/lalrpop/sqlx), it requires a policy-PR adding a generated-code manifest. v1 = ban + revisit.

---

## 9. Two-Environment Architecture + Evidence Isolation

### 9.1 Trusted runner + untrusted execution + isolated evidence

```
┌─────────────────────────────────────────────────────────────────┐
│ UNTRUSTED EXECUTION JOB                                         │
│ (inside trusted Xtask runner image built from protected main)   │
│                                                                 │
│ Mounts:                                                         │
│   /work/source/     — READ-ONLY (gated repo)                   │
│   /work/target-cargo/ — WRITABLE (cargo children only)          │
│   /work/out/         — WRITABLE (OUT_DIR)                      │
│   /work/tmp/         — WRITABLE (temp)                         │
│   /evidence/         — WRITABLE BY XTASK ONLY, not mounted     │
│                        into cargo child processes              │
│                                                                 │
│ Xtask runs lanes. Cargo children CANNOT write /evidence/.       │
│ Xtask copies trusted lane outputs to /evidence/ after children  │
│ complete.                                                       │
│                                                                 │
│ NO SIGNING KEY. NO HOST SOCKETS. NO DOCKER SOCKET.              │
│ NO SSH/GPG AGENTS. NO CLOUD METADATA. NO HOST HOME.             │
│ Fresh CARGO_HOME/RUSTUP_HOME. Fresh target dir.                 │
│                                                                 │
│ OUTPUTS: /evidence/ bundle + evidence digest                    │
└──────────────────────────┬──────────────────────────────────────┘
                           │ evidence bundle + CI provenance token
                           │ (NOT a live checkout)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│ SIGNING JOB (hardened, no code execution)                       │
│                                                                 │
│ Verifies (not just receives digests):                           │
│   1. CI workflow identity matches expected                      │
│   2. Trusted Xtask runner image digest matches policy           │
│   3. Repository/ref/branch policy matches                       │
│   4. Execution job completed successfully (CI status)           │
│   5. Evidence schema validates                                  │
│   6. Evidence internal digests self-consistent                  │
│   7. Artifact digest matches the artifact promoted to deploy    │
│   8. Source digest, policy epoch, advisory epoch consistent     │
│                                                                 │
│ HAS THE SIGNING KEY (KMS/HSM or keyless CI identity)            │
│                                                                 │
│ OUTPUTS: signed Attestation + ledger entry                      │
└─────────────────────────────────────────────────────────────────┘
```

The signer does NOT run repository code. It verifies provenance, then signs. "Digest came over the wall" is not enough.

### 9.2 Sandbox additions (beyond v4)

No host sockets, no Docker/container runtime socket, no SSH/GPG agents, no cloud metadata (169.254.169.254), no `/proc` scraping, no shared sccache unless content-addressed and untrusted-safe, no host `HOME`. Build scripts that compile C code: `cc`/`clang`/`ar`/`ld`/`pkg-config` digests included in build-env evidence (or ban native links in v1).

### 9.3 Shared cache poisoning

sccache and Moon remote cache: if cache accepts writes from untrusted jobs, a malicious run can poison outputs. Either ban shared compiler caches for gate runs, or bind cache provenance into evidence (content-addressed, verified on hit).

---

## 10. Certificate Model

### 10.1 Evidence (deterministic)

```rust
pub struct Evidence {
    pub schema_version: u16,
    pub source_manifest_digest: Digest,      // canonical source manifest (§10.5)
    pub cargo_lock_digest: Digest,
    pub dependency_source_digest: Digest,     // vendored dep tree
    pub artifact_digest: Option<Digest>,      // required for deploy certs (Layer 8)
    pub artifact_kind: Option<ArtifactKind>,  // Binary | Cdylib | Container | Wasm
    pub build_profile: Option<String>,
    pub target_triples: Option<Box<[String]>>,
    pub feature_set: Option<Box<[String]>>,
    pub build_command_argv_digest: Option<Digest>,
    pub policy_digest: Digest,
    pub toolchain_image_digest: Digest,       // OCI/Nix image digest, not version strings
    pub xtask_runner_image_digest: Digest,    // trusted runner that ran the gate
    pub advisory_db_digest: Digest,
    pub advisory_epoch: u64,                  // increments on advisory snapshot update
    pub cargo_vet_db_digest: Digest,
    pub feature_matrix_digest: Digest,        // records mode (powerset/bounded/declared)
    pub mutation_baseline_digest: Option<Digest>,
    pub per_lane: Box<[DeterministicLaneEvidence]>,
    pub scope: GateScope,
    pub env_digest: Digest,
    pub native_toolchain_digest: Option<Digest>, // cc/ld/pkg-config if build scripts link native
}
```

### 10.2 Attestation (signed, non-deterministic)

```rust
pub struct Attestation {
    pub evidence_digest: Digest,        // blake3 of canonical-serialized Evidence
    pub issued_at_utc: DateTime<Utc>,
    pub expires_at_utc: DateTime<Utc>,   // max TTL enforced by policy (e.g. 24h)
    pub signing_key_id: String,
    pub ci_workflow_identity: String,    // provenance: which CI workflow produced this
    pub runner_image_digest: Digest,     // provenance: trusted runner image
    pub signature: Vec<u8>,              // Ed25519 over domain-separated pre-sign payload
}
```

**Domain separation:** signature is over `b"XTASK_ATTESTATION_V1" || canonical_pre_sign_payload`. Prevents cross-protocol signature reuse.

**Max TTL:** policy declares `max_attestation_ttl` (e.g. 24h). Deploy-gate rejects attestations whose `expires_at - issued_at` exceeds policy.

### 10.3 Certificate (bundle)

Certificate = Evidence + Attestation, serialized together. This is the user-facing/deploy-facing artifact.

### 10.4 Canonical serialization

Binary canonical format (Borsh or postcard) for Evidence digests — JSON canonicalization is a swamp (number formatting, Unicode normalization, duplicate keys). JSON is produced only as a human/AI report, NOT for digest computation. Digests: `{ algorithm: Blake3, hex: lowercase-64-chars }`.

### 10.5 Source manifest (canonical file set)

```rust
pub struct SourceManifestEntry {
    pub path: WorkspacePath,
    pub kind: FileKind,         // File | Symlink | Directory
    pub digest: Digest,
    pub executable_bit: bool,
}
```

Includes: all `.rs` files, `Cargo.toml`/`Cargo.lock`, `build.rs` (manifest-declared), `rust-toolchain.toml`, policy files, `.cargo/config*`, semgrep rules, `include_str!`/`include_bytes!` targets, `#[path]` includes. Source escapes (files outside workspace, symlink targets outside workspace) = errors. `include!(concat!(env!("OUT_DIR"), ...))` banned in v1.

---

## 11. Key Revocation + Freshness

```rust
// In policy:
pub keyring_digest: Digest,            // trusted public keys
pub revocation_list_digest: Digest,    // revoked key_ids
```

Deploy-gate checks: signing key not in revocation list. Keyring matches policy. If live fetch is needed for revocation, it is a freshness check OUTSIDE evidence (explicitly nondeterministic), not part of the deterministic evidence chain.

**Advisory epoch:** deploy certificates are per-advisory-epoch. Any advisory snapshot update invalidates prior deploy attestations until re-gated. State this explicitly: redeploying yesterday's binary after today's RustSec update fails until re-gate.

---

## 12. Bypass-Surface Countermeasures

Same as v4 §12, plus:
- `#![allow(...)]`, `#![expect(...)]`, `#[allow_internal_unstable]`, `#[allow_internal_unsafe]` scanned.
- Cargo `[lints]` sections that lower required lints scanned and rejected.
- First-party `--cap-lints` injection via `RUSTFLAGS`/`.cargo/config`/wrapper scanned.
- Manifest-declared build scripts (not just `build.rs`) scanned.
- Build-dep and proc-macro-dep build scripts considered executable supply-chain code (allowlisted, source-digested).
- Parser-backed scan for Rust source (not just rg) for `#[allow]`/assert-macros. rg as coarse prefilter only.
- `cfg`-aware scanning with defined cfg universe (test=false, target matrix, feature matrix).

---

## 13. Escape Hatch (anti-circular policy)

NO per-site bypass. Policy-PR checked against PREVIOUS main-branch policy + CODEOWNER + diff classification. Gate-control surface expanded to include `.moon/**`, `.github/workflows/**`, CI config, xtask crates, signing config, sandbox config — not just lint files.

---

## 14. Deploy-Gate

Checks ALL of:
1. Certificate present (Evidence + Attestation), `scope = Full`.
2. Signature valid under `signing_key_id`, domain-separated, key not revoked.
3. Fresh: `issued_at ≤ now ≤ expires_at`, TTL ≤ policy max.
4. CI provenance: workflow identity + runner image digest match policy.
5. Evidence matches: recompute from actual artifact+source+policy+toolchain+advisory-db+env. Mismatch → REJECT.
6. Advisory epoch current: `advisory_db_digest` + `advisory_epoch` match current pinned snapshot.
7. Artifact-bound: `artifact_digest` + `artifact_kind` + `target_triples` + `build_profile` + `feature_set` match the deployment request.
8. If deploy-gate cannot run → REJECT (fail-closed).

**Deploy request must include:** artifact, evidence.json, attestation.json, source manifest/commit digest, policy manifest digest, toolchain image digest. Do not imply deploy can recompute from things it doesn't possess.

**Moon:** `runInCI: 'always'` (not `true`) for deploy-gate — never skipped by affected-target logic. Explicit `deps: ['sign-attestation']`.

---

## 15. Audit Ledger (SQLite, hash-chained)

Same as v4 §8.5. Every gate_run, attestation, policy_change, revocation recorded. Local ledger (verifier) + CI ledger (signer, authoritative). `xtask ledger verify` walks the chain. `xtask ledger query` for audit history.

---

## 16. Moon CI/CD Integration

```yaml
gate-edit:
  command: 'xtask gate --scope edit --emit json --out /evidence/report.json'
  options: { runInCI: true }
  inputs: ['@globs(sources)', '.xtask/**', 'Cargo.toml', 'Cargo.lock', '**/Cargo.toml',
           '.cargo/**', 'rustfmt.toml', 'clippy.toml', 'deny.toml', 'supply-chain/**',
           'rust-toolchain.toml']

gate-full:
  command: 'xtask gate --scope full --emit json --out /evidence/report.json'
  inputs: ['@globs(sources)', '.xtask/**', 'Cargo.toml', 'Cargo.lock', '**/Cargo.toml',
           '.cargo/**', 'rustfmt.toml', 'clippy.toml', 'deny.toml', 'supply-chain/**',
           'rust-toolchain.toml', '.cargo/mutants.toml', '.xtask/mutants-baseline.json',
           '.xtask/advisory-db-snapshot/', '.xtask/feature-matrix.toml',
           '.xtask/target-matrix.toml']

sign-attestation:
  command: 'xtask sign-evidence /evidence/evidence.json --out /evidence/attestation.json'
  deps: ['gate-full']
  options: { runInCI: true }
  # RUNS IN SIGNER ENVIRONMENT — no repository code execution
  outputs: ['/evidence/attestation.json']

deploy-gate:
  command: 'xtask verify-attestation /evidence/attestation.json --require-signature --require-scope full'
  deps: ['sign-attestation']
  options: { runInCI: 'always' }   # NOT 'true' — never skipped by affected logic
```

---

## 17. Error Taxonomy

### Report root
`Pass | Reject{code_findings, gate_failures, kind} | PolicyError | InputError`

### Lane failures
`InfraFailure | ToolFailure | ResourceFailure | SuspiciousFailure`

### Rule families
`HOLZMAN_PANIC_*`, `HOLZMAN_UNSAFE_*`, `HOLZMAN_CHECKED_*`, `FUNC_LOOPS_*`, `FUNC_NESTING_*`, `FUNC_STYLE_*`, `SUPPLY_ADVISORY`, `SUPPLY_LICENSE`, `SUPPLY_BANNED_CRATE`, `SUPPLY_VET_GAP`, `SUPPLY_UNUSED_DEP`, `SUPPLY_UNSAFE_DEP_THRESHOLD`, `FEATURE_COMBO_FAILED`, `TEST_FAILURE`, `MUTANT_SURVIVED`, `MUTANT_BASELINE_EXPIRED`, `MUTANT_BASELINE_UNOWNED`, `BYPASS_*`, `POLICY_*`, `CERT_*`, `INPUT_*`, `GATE_*`, `BUILD_*`

No decorative severity. Findings reject or are informational.

---

## 18. Toolchain Requirements (scope-tiered)

| Scope | Hard-required |
|---|---|
| edit | `cargo`, `rustc`, `rustfmt`, `clippy`, `rg`, `semgrep` |
| prepush | edit + `cargo-audit`, `cargo-deny`, `cargo-vet`, `cargo-geiger`, `cargo-machete`, `cargo-hack` |
| full | prepush + `cargo-mutants` |

Optional in tests (Xtask v1 doesn't know about them unless via `cargo test`): `proptest`, `quickcheck`, `insta`, `rstest`, `loom`, `miri`, `cargo-fuzz`.

All pinned. `toolchain_image_digest` = OCI/Nix image digest (not version strings). `doctor` reports available vs **trusted** (expected digest, actual digest, absolute path, source of installation).

---

## 19. Second-Order & Pre-Mortem

**Key-leak mitigation:** revocation, KMS/HSM/keyless (Sigstore), transparency logging (Rekor), branch-protected signing, separation from untrusted execution.

**3 AM disaster:** logical correctness bug passing all lanes. Xtask certifies policy conformance, not correctness.

**Macro-expanded code blind spot:** semgrep blind; clippy partial. Known residual.

**Cache poisoning:** shared sccache/Moon cache can be poisoned. Ban or content-address.

---

## 20. Honest Trust Boundary

Xtask certifies conformance to a pinned quality policy. Mechanically checked, policy-conformant, evidence-backed, fail-closed, artifact-bound, deterministic for a pinned input set, with a chain of custody from trusted runner to signed attestation.

Does NOT guarantee: behavioral correctness, all UB freedom, macro-expanded code quality, unknown vulnerabilities, concurrency soundness, that `unwrap_or` is a panic risk (house style), dependency soundness (geiger counts, doesn't prove).

---

## 21. Component / Module Map

- `xtask-bin` — CLI: `gate`, `verify-attestation`, `sign-evidence`, `ledger`, `doctor`.
- `xtask-core` — domain types (§5).
- `xtask-policy` — policy loading, meta-policy, profile loading.
- `xtask-lanes` — lane runners: `fmt`, `rustc`, `clippy`, `semgrep`, `assert_build_scan`, `test`, `supply`, `feature`, `build`, `mutants`.
- `xtask-sandbox` — mount isolation, network-off, frozen PATH/env, cgroup caps, no sockets/agents.
- `xtask-evidence` — canonical serialization (Borsh/postcard), Evidence computation, source manifest.
- `xtask-signer` — Attestation signing, Ed25519 domain-separated, provenance verification, key_id, revocation.
- `xtask-ledger` — SQLite hash-chained audit ledger.
- `xtask-bypass` — parser-backed bypass scans.
- `xtask-output` — report JSON schema (versioned), doctor diagnostics.

---

## 22. CLI Surface

```
xtask gate [--input <crate|diff>] [--scope edit|prepush|full] [--emit json] [--out <path>]
xtask sign-evidence <evidence.json> [--out <attestation.json>]     # signer env only
xtask verify-attestation <attestation.json> [--require-signature] [--require-scope full]
xtask ledger verify
xtask ledger query [--type <type>] [--limit <n>] [--source-digest <digest>]
xtask doctor [--scope <scope>]
```

Exit: `0` Pass, `1` Reject, `2` PolicyError, `3` InputError, `>=4` internal.

---

## 23. Definition of Done — Phased (v1a → v1d)

**v1a — edit scope + report schema + policy + bypass:**
1. `xtask gate --scope edit` runs fmt, check, clippy (source-only `--lib --bins`), semgrep, panic/assert+build.rs scan.
2. Report schema: `Pass | Reject{code_findings, gate_failures, kind} | PolicyError | InputError`.
3. Policy loading, validation, `policy_digest`.
4. Parser-backed `#[allow]`/`#[expect]` bypass scan.
5. `--frozen`, `CARGO_HOME`/`RUSTUP_HOME` controlled, absolute binary paths.

**v1b — prepush scope + supply chain + tests + features:**
6. `cargo test --workspace --frozen -- --test-threads=1`.
7. Supply chain (audit/deny/vet/geiger/machete with triage baselines).
8. Feature matrix (bounded-depth per policy).
9. Scope-tiered tool requirements; `doctor` available-vs-trusted.

**v1c — full scope + artifact build + mutants:**
10. Artifact build lane: `cargo build --release --frozen --target --features`. `artifact_digest` + kind + target + profile in evidence.
11. `cargo mutants` with baseline + triage.

**v1d — evidence + signer + deploy + ledger:**
12. Trusted-runner bootstrap (PRs gated by N-1, not self).
13. Evidence isolation (`/evidence/` not writable by cargo children).
14. Signer with provenance verification (workflow identity, runner image, schema, artifact).
15. Deploy-gate: signature, freshness, epoch, artifact-bound, provenance.
16. SQLite audit ledger (hash-chained, local + CI).
17. Moon `sign-attestation` task, `deploy-gate` with `runInCI: 'always'` + explicit deps.
18. Xtask's own source passes the full gate under the trusted runner.

**Killer demo (after v1d):** AI writes Rust with `for` loop + `.unwrap()` → `xtask gate --scope edit` rejects with `FUNC_LOOPS_*` + `HOLZMAN_PANIC_UNWRAP` → AI fixes → full CI gate passes → signer issues attestation → deploy-gate accepts.

---

## 24. References

- `holzman-rust/SKILL.md` + all 6 references
- `functional-rust/SKILL.md` + all 3 references
- `moon-v2/SKILL.md`
