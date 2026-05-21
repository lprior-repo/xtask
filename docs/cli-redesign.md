# xtask CLI Redesign

## Problem

Two competing CLI architectures:
- **Required command families** (20 commands, all placeholder, JSON-lines output)
- **Legacy clap commands** (15 commands, actually work, flat unstructured)

Result: 35 commands with no logical grouping, inconsistent flags, hardcoded velvet-ballistics paths.

## Proposed Unified CLI

```
xtask [OPTIONS] <COMMAND>

Options:
  -c, --config <FILE>     Config file path [default: .xtask.toml]
  -w, --workspace <DIR>   Workspace root [default: auto-detect]
  -j, --jobs <N>          Parallel jobs [default: auto]
      --json              Output JSON
      --dry-run           Show what would run
  -v, --verbose           Verbose output
  -q, --quiet             Suppress non-error output
  -h, --help              Print help
  -V, --version           Print version

Commands:
  check          Run quality gates
    fast         Quick checks (fmt, clippy, unit tests)
    deep         Thorough checks (+ proof, loom, miri)
    release      Release gates (+ evidence, bundle validation)

  proof          Formal verification orchestration
    plan         List proof obligations for a crate
    run          Run proof suite with profile
    check        Check specific proof level
    evidence     Generate evidence bundle
    drift        Detect spec drift

  test           Testing operations
    unit         Run unit tests
    loom         Run loom concurrency tests
    ui           UI test operations
      snapshot   Render UI snapshots
      tokens     Generate UI tokens from design spec
      overlap    Check UI element overlap
    affected     Test only crates changed since base ref

  scan           Codebase scanning
    forbidden    Scan for banned patterns (unwrap, panic, unsafe)
    contracts    Validate contract files

  workspace      Workspace introspection
    list         List workspace crates
    deps         Show dependency graph
    health       Check workspace health

  completions    Generate shell completions (bash, zsh, fish)
```

## Key Improvements

### 1. Logical Grouping
All 35 commands collapse into ~20 commands under 6 clear groups. No more `proof-plan` vs `proof plan` vs `proof --plan` confusion.

### 2. Config File Support
`.xtask.toml` in workspace root or passed via `--config`:

```toml
[workspace]
root = "."                    # auto-detected if omitted
crates = ["vb_core", "vb_storage"]  # limit to these crates

[check]
profiles = { fast = "fmt,clippy,test", deep = "fast+proof+loom", release = "deep+evidence" }

[proof]
lanes = ["kani", "verus", "flux", "loom", "miri", "proptest"]
timeout = 300

[scan]
forbidden = ["unwrap", "panic", "unsafe", "todo", "dbg_macro"]

[output]
format = "pretty"             # pretty | json | jsonl
progress = true
colors = "auto"               # auto | always | never
```

### 3. Consistent Global Flags
Every subcommand inherits: `--config`, `--workspace`, `--json`, `--dry-run`, `--verbose`, `--quiet`, `--jobs`

### 4. Better Output
- Progress bars for long operations (proof runs, scans)
- Colored output with sensible auto-detection
- Structured JSON output via `--json`
- Summary tables at end of multi-crate operations

### 5. Auto-Discovery
- Find workspace root by walking up looking for `Cargo.toml` with `[workspace]`
- Detect available proof lanes automatically
- Detect crates automatically

### 6. Shell Completions
`xtask completions bash > /etc/bash_completion.d/xtask`

## Migration Path

| Old Command | New Command |
|-------------|-------------|
| `ai-fast` | `xtask check fast` |
| `ai-deep` | `xtask check deep` |
| `ai-release` | `xtask check release` |
| `proof-plan` | `xtask proof plan` |
| `proof-check` | `xtask proof check` |
| `proof-evidence` | `xtask proof evidence` |
| `proof-drift` | `xtask proof drift` |
| `proof run` | `xtask proof run` |
| `proof crate` | `xtask proof run --crate <name>` |
| `proof affected` | `xtask test affected` |
| `ui-snapshot` | `xtask test ui snapshot` |
| `ui-tokens` | `xtask test ui tokens` |
| `ui-overlap-check` | `xtask test ui overlap` |
| `loom` | `xtask test loom` |
| `forbidden-scan` | `xtask scan forbidden` |
| `contracts` | `xtask scan contracts` |
| `list-crates` | `xtask workspace list` |

## Implementation Plan

1. **Phase 1**: Remove dual CLI, create unified clap structure
2. **Phase 2**: Add config file support
3. **Phase 3**: Add progress bars and better output
4. **Phase 4**: Add shell completions
5. **Phase 5**: Deprecate old commands with warnings
