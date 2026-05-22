# xtask Rule Engine

## Philosophy

xtask is a deterministic evidence compiler. Every rule is a gate that produces structured PASS/FAIL evidence.

No soft gates. No human guesswork. No agent self-attestation.

## Rule Categories

| Category | ID Prefix | Purpose |
|----------|-----------|---------|
| Holzman | `HZ-` | NASA/JPL Power-of-Ten rules |
| Functional | `FN-` | Functional Rust purity |
| Static | `ST-` | AST/static analysis |
| Architecture | `AR-` | Module/layer/dependency rules |
| Testing | `TS-` | Test quality and coverage |
| Performance | `PF-` | Performance constraints |
| Security | `SC-` | Security hardening |
| Evidence | `EV-` | Evidence completeness |
| Fitness | `FT-` | Fitness functions |

## Rule Severity

| Level | Behavior |
|-------|----------|
| `fatal` | Blocks all downstream gates |
| `error` | Blocks landing, retry required |
| `warn` | Requires explicit waiver |
| `info` | Advisory, tracked for trends |

## Rule Output Schema

Every rule produces:

```json
{
  "rule_id": "HZ-001",
  "category": "holzman",
  "severity": "fatal",
  "status": "pass|fail|inconclusive",
  "file": "path/to/file.rs",
  "line": 42,
  "column": 18,
  "function": "module::function_name",
  "message": "Human-readable description",
  "contract_violation": "REQ-xxx or INV-yyy",
  "repair_guidance": [
    "Specific fix pattern",
    "Alternative approach"
  ],
  "forbidden_repairs": [
    "Do not do X",
    "Do not do Y"
  ],
  "fitness_score": 0.0,
  "evidence": {
    "command": "cargo ...",
    "output_digest": "blake3:...",
    "raw_output_path": ".evidence/..."
  }
}
```

## Fitness Functions

Each rule computes a fitness score `[0.0, 1.0]`:

- `1.0` = Perfect compliance
- `0.0` = Complete violation
- Intermediate values for partial compliance

Fitness scores are aggregated per-bead, per-crate, and per-workspace.

## Railway Programming Rules (RW-001 to RW-008)

These rules enforce `Result<T, TypedError>` for all fallible operations and ban `anyhow`/`eyre`/string errors in core.

| ID | Rule | Severity |
|----|------|----------|
| RW-001 | All fallible core functions return `Result<T, TypedError>` | fatal |
| RW-002 | `?` allowed only with explicit typed error conversion | fatal |
| RW-003 | No `anyhow`, `eyre`, `Box<dyn Error>` in core | fatal |
| RW-004 | No stringly-typed errors in core | fatal |
| RW-005 | No bool success/failure return from fallible operations | error |
| RW-006 | Every error variant has a producing test/path | error |
| RW-007 | Every dependency error maps to exact domain error | error |
| RW-008 | No swallowed errors (no `let _ = result`) | fatal |

## Newtype Rules (NT-001 to NT-008)

These rules enforce domain primitive newtypes with private fields, checked constructors, and no direct `Deserialize` into validated types.

| ID | Rule | Severity |
|----|------|----------|
| NT-001 | Every domain-significant primitive is a newtype | fatal |
| NT-002 | Newtype fields are private | fatal |
| NT-003 | Checked constructor required for untrusted input | fatal |
| NT-004 | Raw → domain conversion is `TryFrom`, not `From` | fatal |
| NT-005 | No `Default` unless contract declares a valid default | error |
| NT-006 | No direct `Deserialize` into validated or witness types | fatal |
| NT-007 | Witness type cannot be public-constructed | fatal |
| NT-008 | Invalid state has compile-fail or negative runtime test | error |

## Zero-Unwrap Policy

assurec enforces a zero-unwrap policy across ALL Rust artifacts without exception:

- **Banned everywhere**: `unwrap`, `unwrap_err`, `unwrap_or`, `unwrap_or_else`, `unwrap_or_default`, `unwrap_unchecked`, `expect`, `expect_err`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, `dbg!`
- **Scope**: production source, tests, benches, examples, build.rs, generated code, Kani harnesses, proptest/fuzz targets, xtask/assurec itself
- Implicit defaulting is forbidden; `unwrap_or_default` is always rejected
- Default behavior must be contract-declared and named

See [assurance-compiler-prd.md](assurance-compiler-prd.md) for the full policy.
