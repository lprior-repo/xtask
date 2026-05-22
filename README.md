# xtask

Developer tooling for the velvet-ballistics workspace.

## Assurance Compiler (assurec)

xtask ships an **assurance compiler** that enforces deterministic hard gates across all Rust artifacts — zero hallucination, zero unwrap, typed expressions, and railway programming.

See [docs/assurance-compiler-prd.md](docs/assurance-compiler-prd.md) for the full design doc.

## What it does

xtask is the automation and quality-gate runner for velvet-ballistics. It provides deterministic, scriptable commands that orchestrate the development workflow:

- **Proof orchestration** — runs Kani, Verus, Flux, Loom, Miri, and proptest harnesses across workspace crates with profiles (`fast`, `standard`, `deep`, `proof`, `all`)
- **Evidence capture** — collects and validates proof artifacts, UI snapshots, and release bundles
- **Contract validation** — checks spec alignment and detects drift between requirements and implementation
- **AI profile gates** — `ai-fast`, `ai-deep`, `ai-release` for tiered quality checks
- **Forbidden pattern scanning** — ensures no `unwrap`, `panic`, `unsafe`, or other banned constructs leak into the codebase
- **UI tooling** — snapshot rendering, overlap detection, token extraction for the Makepad UI layer

## Usage

From the velvet-ballistics workspace, run via the cargo alias:

```bash
cargo xtask contracts --check
cargo xtask proof --run --profile fast
cargo xtask forbidden-scan
cargo xtask ai-release
```

From this repo directly:

```bash
cargo run -- contracts --check
cargo run -- proof --run --profile standard
cargo run -- forbidden-scan
```

## Dependencies

xtask depends on two crates from the sibling `velvet-ballistics` repo:

- `vb_ui_snapshot` — UI snapshot testing infrastructure
- `vb_validate` — schema validation and gate framework

These are referenced via relative path (`../velvet-ballistics/...`). When those crates are published to crates.io, xtask will switch to versioned dependencies.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --locked --lib --bins --examples --all-features -- -D warnings
cargo test --locked --all-features
```
