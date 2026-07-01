//! Scans `crates/*/src` for production panic/assert macros.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-panic-surface.sh`. Run via
//! `cargo run --bin check-panic-surface --` from the repository root or
//! via the matching Moon task in `.moon/tasks/all.yml`.
//!
//! ## Behavior parity
//! Mirrors the bash's exclusion globs and per-line allowlist rules:
//!
//! 1. **Path exclusions** — skip tests, benches, examples, fuzz harnesses,
//!    `target/`, `.beads/`, fixtures, `build.rs`, `*_tests.rs`, `tests.rs`,
//!    `lifecycle_tests/`, `kani*.rs`, `models/loom/**`, `proofs/**`, etc.
//! 2. **Production path filter** — only lines outside `#[cfg(test)]`,
//!    `#[cfg(kani)]`, and `#[kani::proof]` blocks count.
//! 3. **Comment skip** — lines whose payload (after the file:line prefix)
//!    starts with `//` are not violations (matches `rg` post-filter).
//! 4. **Pattern** — `(^|[^A-Za-z0-9_])(assert!|assert_eq!|assert_ne!|unreachable!)`
//!
//! Each violation becomes a typed `Finding`; the report's `render()`
//! gives a stable `path:line: rule -- message` line. The bash's
//! `ViolationFound` / `NoViolationFound` summaries are preserved.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use titania_lanes::{Finding, LaneExit, LaneReport, SourceLine, current_target_project, exit};

/// Macros the bash lane flags. Kept as a single array so additions land
/// in one obvious place.
const PANIC_MACROS: &[&str] = &["assert!", "assert_eq!", "assert_ne!", "unreachable!"];

/// Path segments whose presence means the file is non-production.
const EXCLUDED_SEGMENTS: &[&str] = &[
    "/workspace_tests/",
    "/test_loop_inventory/",
    "/tests/",
    "/lifecycle_tests/",
    "/benches/",
    "/examples/",
    "/proofs/",
    "/models/loom/",
    "/target/",
    "/.beads/",
    "/fixtures/",
    "/fuzz/",
    "/titania-lanes/src/bin/",
];

fn main() -> std::process::ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-panic-surface] target discovery failed: {error}");
            return exit(LaneExit::Failure);
        }
    };
    eprintln!("CWD: {}", cwd_string());
    eprintln!("Command: bash scripts/check-panic-surface.sh");
    eprintln!("ScanDomain: crates/*/src");
    eprintln!(
        "NonProductionPathExcluded: tests benches examples fuzz target .beads fixtures \
         build.rs path-scoped tests.rs *_tests.rs kani harnesses loom models"
    );

    let mut report = LaneReport::new();
    for file in collect_source_files(target.as_std_path()) {
        scan_file(target.as_std_path(), &file, &mut report);
    }

    eprint!("{}", report.render());
    if report.is_clean() {
        eprintln!("NoViolationFound");
        exit(LaneExit::Clean)
    } else {
        eprintln!("ViolationFound: production panic/assert macro surface is non-empty");
        exit(LaneExit::Violations)
    }
}

fn cwd_string() -> String {
    std::env::current_dir().map_or_else(|_| String::from("?"), |p| p.display().to_string())
}

fn collect_source_files(root: &Path) -> Vec<PathBuf> {
    let crates_dir = root.join("crates");
    let Ok(entries) = std::fs::read_dir(crates_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|e| e.path().join("src"))
        .filter(|p| p.is_dir())
        .flat_map(walk_rust_files)
        .filter(|p| !is_excluded_path(p))
        .collect()
}

fn walk_rust_files(dir: PathBuf) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![dir];
    while let Some(top) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&top) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Replicate the bash `--glob '!...'`. The list mirrors
/// `check-panic-surface.sh`. We test path segments (not just
/// substrings) so e.g. `models/loom/foo.rs` matches but
/// `my_models_loom/foo.rs` does not.
fn is_excluded_path(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if EXCLUDED_SEGMENTS.iter().any(|seg| normalized.contains(seg)) {
        return true;
    }
    let name = path.file_name().and_then(|n| n.to_str()).map_or("", |value| value);
    if name == "tests.rs"
        || name == "build.rs"
        || name.ends_with("_tests.rs")
        || name == "check-panic-surface.sh"
        || name == "check_panic_surface.rs"
        || name.starts_with("kani")
    {
        return true;
    }
    false
}

fn scan_file(root: &Path, path: &Path, report: &mut LaneReport) {
    report.record_scan();
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };

    let display = rel_str(root, path);
    // Stacks of (synthetic_open_added, global_brace_depth_at_open).
    // `cfg_open` is +1 because the cfg attribute itself has no `{`; the
    // matching `{` lands on the `mod tests {` / `fn proof_fn() {` line
    // that follows. The scope closes when the global brace depth drops
    // back to the snapshotted depth.
    let mut cfg_depth: u32 = 0;
    let mut kani_proof_depth: u32 = 0;
    let mut global_depth: u32 = 0;
    let mut cfg_open_depths: Vec<u32> = Vec::new();
    let mut kani_open_depths: Vec<u32> = Vec::new();
    // Block-comment carry-over: a `/* … */` that opens on one line
    // and closes on a later line must skip every line in between.
    let mut block_comment: bool = false;

    for (idx, raw) in content.lines().enumerate() {
        let line_no = line_no_from_idx(idx);
        // The SourceLine parser strips line/block comments and
        // blanks string contents. We still operate on `raw` for
        // brace tracking and panic-macro detection, but we use
        // `parsed.is_non_code()` to detect lines that are entirely
        // comments or strings (which the panic-macro check should
        // not flag) and `parsed.code()` to look for the macros in
        // rune form (so an `assert!` inside a string literal does
        // not leak through).
        let parsed = SourceLine::parse(raw, &mut block_comment);
        if parsed.is_non_code() {
            // Even all-comment lines can affect the global brace
            // counter if they contain a stray `{` or `}` (which is
            // rare but legal in a line-comment-adjacent character).
            // Track braces in the raw line to keep the count honest.
            global_depth = global_depth.saturating_add_signed(line_brace_delta(raw.trim_start()));
            continue;
        }
        let trimmed = raw.trim_start();
        let line_brace_delta_val = line_brace_delta(trimmed);
        let opened_cfg_here = is_cfg_attr_open(trimmed, &["test", "kani"]);
        let opened_kani_proof_here = !opened_cfg_here && trimmed.starts_with("#[kani::proof]");

        if opened_cfg_here {
            cfg_depth = cfg_depth.saturating_add(1);
            // The cfg block's `{` will land on a later line; we treat
            // the current global depth + 1 as the depth the matching
            // `}` will eventually return us to.
            cfg_open_depths.push(global_depth.saturating_add(1));
        }
        if opened_kani_proof_here {
            kani_proof_depth = kani_proof_depth.saturating_add(1);
            kani_open_depths.push(global_depth.saturating_add(1));
        }

        let inside_test_or_kani = cfg_depth > 0 || kani_proof_depth > 0;
        if !inside_test_or_kani && let Some(macro_name) = first_panic_macro(parsed.code()) {
            report.push(Finding::new(
                "PANIC-SURFACE-001",
                display.clone(),
                line_no,
                format!("production panic/assert macro `{macro_name}` is forbidden"),
            ));
        }

        // Apply this line's brace delta to the global counter.
        global_depth = global_depth.saturating_add_signed(line_brace_delta_val);

        // Pop cfg/kani scopes whose synthetic open depth we have
        // returned to. The snapshot is `global_depth_at_open + 1`, so
        // the matching `{` (e.g. `mod tests {`) brings the global
        // counter to the snapshot; the matching `}` brings it below.
        // We close only on strict `<` so the cfg block's own `{` does
        // not pop the scope prematurely.
        if !opened_cfg_here
            && let Some(&target) = cfg_open_depths.last()
            && global_depth < target
        {
            cfg_open_depths.pop();
            cfg_depth = cfg_depth.saturating_sub(1);
        }
        if !opened_kani_proof_here
            && let Some(&target) = kani_open_depths.last()
            && global_depth < target
        {
            kani_open_depths.pop();
            kani_proof_depth = kani_proof_depth.saturating_sub(1);
        }
    }
}

fn rel_str(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map_or_else(|_| path.display().to_string(), |rel| rel.display().to_string())
}

/// Net `{` - `}` count for a line: positive when more open than close.
fn line_brace_delta(line: &str) -> i32 {
    let opens =
        i32::try_from(line.bytes().filter(|b| *b == b'{').count()).map_or(i32::MAX, |value| value);
    let closes =
        i32::try_from(line.bytes().filter(|b| *b == b'}').count()).map_or(i32::MAX, |value| value);
    opens.saturating_sub(closes)
}

fn is_cfg_attr_open(line: &str, scopes: &[&str]) -> bool {
    let Some(rest) = line.strip_prefix("#[cfg(") else {
        return false;
    };
    let Some(inside) = rest.strip_suffix(")]") else {
        return false;
    };
    scopes.iter().any(|s| inside.split(',').any(|p| p.trim() == *s))
}

fn first_panic_macro(line: &str) -> Option<&'static str> {
    PANIC_MACROS.iter().copied().find(|m| {
        let Some(idx) = line.find(m) else {
            return false;
        };
        let bytes = line.as_bytes();
        // `.get` rather than `bytes[idx - 1]` to avoid the indexing
        // lint. `None` means "no neighbor" which is a valid word boundary.
        let before_ok =
            idx == 0 || bytes.get(idx.wrapping_sub(1)).is_none_or(|b| !is_word_byte(*b));
        let Some(after_idx) = idx.checked_add(m.len()) else {
            return false;
        };
        let after_ok = bytes.get(after_idx).is_none_or(|b| !is_word_byte(*b));
        before_ok && after_ok
    })
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Convert a 0-indexed line offset into a 1-indexed `u32` line number,
/// saturating at `u32::MAX` on overflow. Equivalent to
/// `titania_lanes::helpers::line_no_from_idx` but kept local so the bin
/// does not need to know about the helpers module.
fn line_no_from_idx(idx: usize) -> u32 {
    u32::try_from(idx.saturating_add(1)).map_or(u32::MAX, |value| value)
}
