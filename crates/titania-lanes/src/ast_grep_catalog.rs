//! Embedded ast-grep rule catalog and a deterministic fixture scanner.
//!
//! The production lane can swap the scanner backend for `ast-grep-core`; this
//! module keeps the v1 contract anchored in checked-in rule metadata with
//! concrete repair hints.

use crate::Finding;

const FUNCTIONAL_YAML: &str = include_str!("../rules/functional.yml");
const BYPASS_YAML: &str = include_str!("../rules/bypass.yml");
const ARCHITECTURE_YAML: &str = include_str!("../rules/architecture.yml");

/// One embedded rule definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AstGrepRule {
    id: &'static str,
    severity: &'static str,
    pattern: &'static str,
    message: &'static str,
    repair_hint: &'static str,
}

impl AstGrepRule {
    /// Stable v1 rule id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// Rule severity. v1 uses `error` for all catalog entries.
    #[must_use]
    pub const fn severity(self) -> &'static str {
        self.severity
    }

    /// ast-grep pattern or family marker.
    #[must_use]
    pub const fn pattern(self) -> &'static str {
        self.pattern
    }

    /// Human-readable finding message.
    #[must_use]
    pub const fn message(self) -> &'static str {
        self.message
    }

    /// Concrete repair text; never a generic human-review placeholder.
    #[must_use]
    pub const fn repair_hint(self) -> &'static str {
        self.repair_hint
    }
}

/// Path policy used by architecture rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchitecturePolicy {
    core_dirs: &'static [&'static str],
    infra_crates: &'static [&'static str],
}

impl ArchitecturePolicy {
    /// v1 strict-ai defaults from v1-spec §9.7.
    #[must_use]
    pub const fn strict_ai() -> Self {
        Self {
            core_dirs: &["src/core", "src/domain", "crates/*-core/src"],
            infra_crates: &["tokio", "axum", "sqlx", "reqwest"],
        }
    }

    /// Returns true when `path` is in a configured core directory.
    #[must_use]
    pub fn is_core_path(self, path: &str) -> bool {
        self.core_dirs.iter().any(|dir| core_dir_matches(path, dir))
    }

    /// Returns true when `crate_name` is a forbidden infra crate in core code.
    #[must_use]
    pub fn is_infra_crate(self, crate_name: &str) -> bool {
        self.infra_crates.contains(&crate_name)
    }
}

const RULES: &[AstGrepRule] = &[
    AstGrepRule {
        id: "FUNC_LOOPS_FOR",
        severity: "error",
        pattern: "for $PAT in $ITER { $$$BODY }",
        message: "replace imperative for loop with an iterator pipeline",
        repair_hint: "Rewrite `for item in iter { body }` as `iter.into_iter().map(...).collect()` or `fold(...)`.",
    },
    AstGrepRule {
        id: "FUNC_LOOPS_WHILE",
        severity: "error",
        pattern: "while $COND { $$$BODY }",
        message: "replace while loop with a bounded iterator/state transition",
        repair_hint: "Parse a bounded range or state iterator, then use `try_fold` to return typed errors.",
    },
    AstGrepRule {
        id: "FUNC_LOOPS_LOOP",
        severity: "error",
        pattern: "loop { $$$BODY }",
        message: "replace unbounded loop with an explicit bounded transition",
        repair_hint: "Use a named bounded iterator or `std::iter::successors(...).take(MAX)` with typed termination.",
    },
    AstGrepRule {
        id: "FUNC_PRINT_STDOUT",
        severity: "error",
        pattern: "println!($$$ARGS)",
        message: "library code must return structured output instead of printing stdout",
        repair_hint: "Return a `Finding`/report value or write through an injected sink instead of `println!`.",
    },
    AstGrepRule {
        id: "FUNC_PRINT_STDERR",
        severity: "error",
        pattern: "eprintln!($$$ARGS)",
        message: "library code must return structured output instead of printing stderr",
        repair_hint: "Return a typed error/finding and let the binary boundary render stderr.",
    },
    AstGrepRule {
        id: "BYPASS_ALLOW_ATTR",
        severity: "error",
        pattern: "#[allow($$$LINTS)]",
        message: "lint suppression requires a strict-ai exception entry",
        repair_hint: "Remove `#[allow(...)]`; if justified, add owner/reason/expiry/review to `.titania/profiles/strict-ai/exceptions.toml`.",
    },
    AstGrepRule {
        id: "BYPASS_EXPECT_ATTR",
        severity: "error",
        pattern: "#[expect($$$LINTS)]",
        message: "expected lint suppression requires a strict-ai exception entry",
        repair_hint: "Remove `#[expect(...)]`; if justified, add owner/reason/expiry/review to `.titania/profiles/strict-ai/exceptions.toml`.",
    },
    AstGrepRule {
        id: "FUNC_WILDCARD_IMPORT",
        severity: "error",
        pattern: "use $PATH::*;",
        message: "wildcard imports hide dependencies and defeat review",
        repair_hint: "Replace the wildcard import with explicit imported names, e.g. `use path::{Type, function};`.",
    },
    AstGrepRule {
        id: "ARCHITECTURE_IMPORT_CORE_INFRA",
        severity: "error",
        pattern: "use tokio|axum|sqlx|reqwest in core paths",
        message: "core/domain code must not depend on infrastructure crates",
        repair_hint: "Move the import to an adapter/shell crate and pass a typed port into the core boundary.",
    },
    AstGrepRule {
        id: "ARCHITECTURE_IMPORT_CORE_FS",
        severity: "error",
        pattern: "use std::fs in core paths",
        message: "core/domain code must not perform filesystem I/O",
        repair_hint: "Move filesystem access to an adapter and pass parsed data into the pure core.",
    },
    AstGrepRule {
        id: "ARCHITECTURE_IMPORT_CORE_TIME",
        severity: "error",
        pattern: "use std::time in core paths",
        message: "core/domain code must not read ambient time",
        repair_hint: "Accept a typed timestamp/clock value at the boundary instead of importing `std::time`.",
    },
    AstGrepRule {
        id: "ARCHITECTURE_IMPORT_CORE_RANDOM",
        severity: "error",
        pattern: "use rand in core paths",
        message: "core/domain code must not read ambient randomness",
        repair_hint: "Inject deterministic entropy from an adapter via a typed value object.",
    },
];

/// Embedded v1 catalog.
#[must_use]
pub const fn embedded_rules() -> &'static [AstGrepRule] {
    RULES
}

/// Raw checked-in YAML sources embedded by the binary.
#[must_use]
pub const fn embedded_rule_sources() -> &'static [(&'static str, &'static str)] {
    &[("functional.yml", FUNCTIONAL_YAML), ("bypass.yml", BYPASS_YAML), ("architecture.yml", ARCHITECTURE_YAML)]
}

/// Deterministic scanner for contract fixtures.
#[must_use]
pub fn scan_source(path: &str, source: &str, policy: ArchitecturePolicy) -> Vec<Finding> {
    source
        .lines()
        .enumerate()
        .flat_map(|(index, line)| findings_for_line(path, line, line_number(index), policy))
        .collect()
}

fn line_number(index: usize) -> u32 {
    u32::try_from(index).map_or(u32::MAX, |line| line.saturating_add(1))
}

fn findings_for_line(
    path: &str,
    line: &str,
    line_number: u32,
    policy: ArchitecturePolicy,
) -> Vec<Finding> {
    embedded_rules()
        .iter()
        .copied()
        .filter(|rule| line_matches_rule(path, line, *rule, policy))
        .map(|rule| Finding::new(rule.id(), path, line_number, finding_message(rule)))
        .collect()
}

fn finding_message(rule: AstGrepRule) -> String {
    format!("{}; fix: {}", rule.message(), rule.repair_hint())
}

fn line_matches_rule(path: &str, line: &str, rule: AstGrepRule, policy: ArchitecturePolicy) -> bool {
    match rule.id() {
        "FUNC_LOOPS_FOR" => contains_code(line, "for ") && line.contains(" in "),
        "FUNC_LOOPS_WHILE" => contains_code(line, "while "),
        "FUNC_LOOPS_LOOP" => contains_code(line, "loop"),
        "FUNC_PRINT_STDOUT" => contains_code(line, "println!") || contains_code(line, "print!"),
        "FUNC_PRINT_STDERR" => contains_code(line, "eprintln!") || contains_code(line, "eprint!"),
        "BYPASS_ALLOW_ATTR" => contains_code(line, "#[allow("),
        "BYPASS_EXPECT_ATTR" => contains_code(line, "#[expect("),
        "FUNC_WILDCARD_IMPORT" => contains_code(line, "::*"),
        "ARCHITECTURE_IMPORT_CORE_INFRA" => policy.is_core_path(path) && infra_import(line, policy),
        "ARCHITECTURE_IMPORT_CORE_FS" => policy.is_core_path(path) && contains_code(line, "std::fs"),
        "ARCHITECTURE_IMPORT_CORE_TIME" => policy.is_core_path(path) && contains_code(line, "std::time"),
        "ARCHITECTURE_IMPORT_CORE_RANDOM" => policy.is_core_path(path) && contains_code(line, "rand::"),
        _ => false,
    }
}

fn infra_import(line: &str, policy: ArchitecturePolicy) -> bool {
    policy.infra_crates.iter().any(|crate_name| {
        contains_code(line, &format!("use {crate_name}")) || contains_code(line, &format!("{crate_name}::"))
    })
}

fn contains_code(line: &str, needle: &str) -> bool {
    line.split("//").next().is_some_and(|code| code.contains(needle))
}

fn core_dir_matches(path: &str, dir: &str) -> bool {
    if dir == "crates/*-core/src" {
        return matches_core_crate(path);
    }
    path == dir || path.starts_with(&format!("{dir}/"))
}

fn matches_core_crate(path: &str) -> bool {
    path.strip_prefix("crates/").is_some_and(|rest| {
        rest.split_once('/').is_some_and(|(crate_name, after_crate)| {
            crate_name.ends_with("-core") && (after_crate == "src" || after_crate.starts_with("src/"))
        })
    })
}
