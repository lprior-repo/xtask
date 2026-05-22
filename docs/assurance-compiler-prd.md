# xtask Assurance Compiler (assurec) PRD

## Goal

Build xtask as a strict deterministic "assurance compiler" (assurec) that prevents AI hallucination by enforcing hard evidence gates, zero-unwrap/panic policy, railway programming, typed expressions, and defense-in-depth profiles across all Rust code, tests, generated artifacts, and proof harnesses.

## Constraints & Preferences

- Deterministic hard gates only; no soft advice
- Zero unwrap/expect/panic/todo/unimplemented/unreachable/dbg across ALL code (source, tests, benches, generated, proof harnesses, fuzz targets, build scripts, xtask itself)
- Railway programming: Result<T, TypedError> for all fallible operations; no anyhow/eyre/Box<dyn Error> in core
- Newtypes for all domain primitives; no bool bags for meaningful state
- Typed finite expression language (enums, booleans only; no string guards, no arbitrary Rust snippets)
- Independent oracle layer with verified provenance (not "source: human" labels)
- Evidence owned by CI/Moon runner; local evidence is untrusted for landing
- V1 scoped to TenantAccess pilot with JwtVerified as upstream assumption
- Moon wraps xtask; direct xtask evidence is local_untrusted
- Flux, Verus, TLA+, Dylint, MIR, Red Queen, manual QA deferred to v2+ (extension slots only)
- Strict profiles: strict-local (iteration), strict-proof (high-risk), strict-release (landing)
- Defense-in-depth: static → test → formal → adversarial → evidence closure layers

## Progress

### Done
- Created /home/lewis/src/xtask repo and pushed to github.com/lprior-repo/xtask
- Updated .cargo/config.toml in velvet-ballistics to point to ../xtask
- Created /home/lewis/src/xtask/docs/cli-redesign.md (initial CLI redesign proposal)
- Created /home/lewis/src/xtask/docs/rule-engine.md (rule philosophy and categories)
- Created /home/lewis/src/xtask/src/rules/mod.rs (rule engine types: Rule, RuleResult, Evidence, RuleReport, FitnessReport)
- Conducted 3 hostile review cycles with black-hat-reviewer, proof-reviewer, test-reviewer, and compiler-feasibility subagents
- Survived multiple "grill the design" sessions that forced: coherent wrongness detection, oracle provenance verification, typed expression language, path totality/overlap checking, evidence anti-forgery, witness bypass lockdown, Moon wrapping contract

### In Progress
- Finalizing strict assurance compiler PRD after hostile reviews and rebuttals
- Defining zero-unwrap ban list and railway programming rules
- Defining newtype/typed error/boolean-ban policy

### Blocked
- Frozen v1 scope (TenantAccess pilot)
- Typed expression language schema
- Moon task contract and CI integration
- EvidenceRecord schema with claim ceilings
- Independent reference interpreter (separate TCB from Rust codegen)
- Frozen data models before implementation

## Key Decisions

- **Coherent wrongness**: If IR is wrong, every generated artifact can be consistently wrong; fix is independent oracles + adversarial review layer
- **Deterministic vs advisory split**: contract-lint/oracle-check/path-check are deterministic gates; contract-attack/black-hat/proof-review are advisory only
- **Oracle provenance not labels**: "source: human" is forgeable; must use vcs_preexisting, approved_review, historical_bug, or sigstore attestation
- **No string guards**: when = "tenant_claim != requested_tenant" is underspecified; use typed finite expressions with enums (TenantClaimRel, MembershipFact)
- **Independent reference interpreter**: assure-oracle-eval must be separate TCB from assure-codegen-rust to prevent correlated bugs
- **CI-trusted evidence only**: local JSON files are forgeable; landing accepts only ci_signed or verified_attestation
- **JwtVerified boundary**: TenantAccess v1 does NOT prove JWT expiry/signature; those go into claim ceiling and upstream bead dependency
- **Zero unwrap ban**: unwrap/unwrap_or/unwrap_or_else/unwrap_or_default/unwrap_unchecked/expect/panic/todo/unimplemented/unreachable/dbg banned everywhere
- **Railway programming**: RW-001 to RW-008 enforce Result<T, TypedError>, no anyhow in core, explicit error conversions, no swallowed errors
- **Newtype rules**: NT-001 to NT-008 enforce private fields, checked constructors, TryFrom, no Default, no Deserialize into validated types, witness privacy

## Zero-Unwrap Policy

assurec enforces a zero-unwrap policy across ALL Rust artifacts.

### Banned Patterns (all contexts)

| Banned | Scope |
|--------|-------|
| `unwrap` | production, tests, benches, examples, build.rs, generated, harnesses, xtask itself |
| `unwrap_err` | same |
| `unwrap_or` | same |
| `unwrap_or_else` | same |
| `unwrap_or_default` | same |
| `unwrap_unchecked` | same |
| `expect` | same |
| `expect_err` | same |
| `panic!` | same |
| `todo!` | same |
| `unimplemented!` | same |
| `unreachable!` | same |
| `dbg!` | same |

### Railway Programming Rules (RW-001 to RW-008)

| ID | Rule |
|----|------|
| RW-001 | All fallible core functions return `Result<T, TypedError>` |
| RW-002 | `?` allowed only with explicit typed error conversion |
| RW-003 | No `anyhow`, `eyre`, `Box<dyn Error>` in core |
| RW-004 | No stringly-typed errors in core |
| RW-005 | No bool success/failure return from fallible operations |
| RW-006 | Every error variant has a producing test/path |
| RW-007 | Every dependency error maps to exact domain error |
| RW-008 | No swallowed errors (no `let _ = result`) |

### Newtype Rules (NT-001 to NT-008)

| ID | Rule |
|----|------|
| NT-001 | Every domain-significant primitive is a newtype |
| NT-002 | Newtype fields are private |
| NT-003 | Checked constructor required for untrusted input |
| NT-004 | Raw → domain conversion is `TryFrom`, not `From` |
| NT-005 | No `Default` unless contract declares a valid default |
| NT-006 | No direct `Deserialize` into validated or witness types |
| NT-007 | Witness type cannot be public-constructed |
| NT-008 | Invalid state has compile-fail or negative runtime test |

### Defaulting Policy

- Implicit defaulting is forbidden
- Default behavior must be contract-declared and named
- `unwrap_or_default` is always rejected

## Next Steps

1. Freeze data model schemas: OracleRecord, ContractClause, TypedExpression, DecisionPath, ClaimCeiling, EvidenceRecord, GeneratedArtifact, AssumptionLedgerEntry, TcbLedgerEntry, Waiver, BaselineFailure, ProjectionCapability
2. Build TenantAccess contract fixture with typed facts (TenantClaimRel, MembershipFact), 5 decision paths, trusted oracle records, claim ceiling
3. Implement typed IR builder (finite expression AST, no string guards)
4. Implement path totality/overlap checker (brute-force enumeration over finite domains)
5. Build codegen: witness types, domain types, error taxonomy, pure decision core, action boundary contracts
6. Build static policy: AST-grep rules, source scanner, derive/serde policy, zero-unwrap scanner, core/shell boundary
7. Build test generation: unit tests, proptest (strategies from domain boundaries), trybuild misuse tests
8. Build Kani harness generation with assumption audit and bound rationale
9. Build evidence closure with runner-owned records, freshness check, claim ceiling enforcement, baseline classification
10. Define Moon task contract and CI integration
11. Add named mutation checks (custom patch-based in v1, cargo-mutants full profile later)
12. Test against malicious implementations: grant-on-mismatch, direct construction, serde bypass, forged evidence

## Critical Context

- Architecture went through 3 major revisions after hostile reviews identified "coherent wrongness" as the core risk
- V1 acceptance test: 13+ malicious implementations must fail deterministically in CI
- Zero-unwrap policy applies to xtask itself (dogfooding)
- TenantAccess pilot scope: proves only "given JwtVerified + tenant claim + membership fact, access decision is correct"; does NOT prove JWT verification, expiry, revocation, database truthfulness
- Flux/Verus/TLA+/Dylint/MIR/Red Queen/manual QA are extension slots, not v1 build targets
- Strict profiles (strict-local/strict-proof/strict-release) replace ad-hoc gate selection
- Error messages must be compiler-quality: location, why it matters, repair options, forbidden repairs, rerun command

## Relevant Files

- /home/lewis/src/xtask/: standalone git repo at github.com/lprior-repo/xtask
- /home/lewis/src/xtask/docs/cli-redesign.md: initial CLI redesign proposal
- /home/lewis/src/xtask/docs/rule-engine.md: rule philosophy, categories, severity, fitness functions
- /home/lewis/src/xtask/src/rules/mod.rs: Rule, RuleResult, Evidence, RuleReport, FitnessReport types
- /home/lewis/src/velvet-ballistics/.cargo/config.toml: cargo alias pointing to ../xtask/Cargo.toml
- /home/lewis/src/velvet-ballistics/velvet-ballistics-MASTER.md: authoritative requirements (Section 40 CI Gate, Section 60 Evidence Artifact Format, Section 43 AI Agent Acceptance Contract)
- /home/lewis/.agents/skills/qa-enforcer/SKILL.md: execute real commands, capture evidence, no hallucinated results
- /home/lewis/.agents/skills/holzman-rust/SKILL.md: NASA/JPL Power-of-Ten rules for Rust
- /home/lewis/.agents/skills/rust-contract/SKILL.md: contract-first specification workflow
- /home/lewis/.agents/skills/functional-rust/SKILL.md: Data→Calculations→Actions separation, zero unwrap, pure core rules
- /home/lewis/.agents/skills/proof-planner/SKILL.md: proof obligation planning
- /home/lewis/.agents/skills/proof-reviewer/SKILL.md: adversarial proof review
- /home/lewis/.agents/skills/test-reviewer/SKILL.md: test quality enforcement
- /home/lewis/.agents/skills/red-queen/SKILL.md: deterministic adversarial evolution
- /home/lewis/.agents/skills/hands-on-qa/SKILL.md: manual QA execution