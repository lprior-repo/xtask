//! Pure-domain helpers shared by the titania-lanes CI/CD binaries.
//!
//! Each lane binary lives in `src/bin/<name>.rs` and follows the same
//! shape:
//!
//! 1. Parse argv into a `LaneInput` (path, mode, scope).
//! 2. Run pure check calculations (data → calc → actions layering).
//! 3. Emit typed findings and an exit code.
//!
//! No binary here does I/O outside the filesystem reads the bash
//! originals did. No async, no `unsafe`, no `unwrap`.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::string_slice)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::arithmetic_side_effects)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::as_conversions)]
#![forbid(unsafe_code)]

use std::{env, io};

use thiserror::Error;
use titania_core::{TargetProject, TargetProjectError, discover_target};

pub mod command;
pub mod helpers;

pub use command::{CommandBudget, CommandIn, CommandOutput, EnvPolicy, LaneError, OutputStream};

/// Errors produced while resolving the target project from the process CWD.
#[derive(Debug, Error)]
pub enum CurrentTargetError {
    #[error("cannot read current directory")]
    CurrentDir(#[source] io::Error),
    #[error(transparent)]
    Target(#[from] TargetProjectError),
}

/// Discover the target Rust project from the current working directory.
///
/// Lanes are launched from the project they should judge; this helper is the
/// single adapter that turns the ambient CWD into the typed `TargetProject`
/// value used by subprocess code.
pub fn current_target_project() -> Result<TargetProject, CurrentTargetError> {
    let cwd = env::current_dir().map_err(CurrentTargetError::CurrentDir)?;
    discover_target(&cwd).map_err(CurrentTargetError::Target)
}
/// One typed finding produced by a lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Stable lane-internal rule id, e.g. `"DISCARD-001"`.
    rule: &'static str,
    /// Repository-relative path that produced the finding.
    path: String,
    /// 1-indexed line number in `path`. `0` if the finding is file-level.
    line: u32,
    /// Human-readable message.
    message: String,
}

impl Finding {
    #[must_use]
    pub fn new(
        rule: &'static str,
        path: impl Into<String>,
        line: u32,
        message: impl Into<String>,
    ) -> Self {
        Self { rule, path: path.into(), line, message: message.into() }
    }

    #[must_use]
    pub fn rule(&self) -> &'static str {
        self.rule
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn line(&self) -> u32 {
        self.line
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Lane output: collected findings plus summary counters.
#[derive(Debug, Default, Clone)]
pub struct LaneReport {
    findings: Vec<Finding>,
    scanned: u32,
    passed: u32,
}

impl LaneReport {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    pub fn push(&mut self, finding: Finding) {
        self.findings.push(finding);
    }

    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    #[must_use]
    pub fn finding_count(&self) -> usize {
        self.findings.len()
    }

    pub fn record_pass(&mut self) {
        self.passed = self.passed.saturating_add(1);
    }

    pub fn record_scan(&mut self) {
        self.scanned = self.scanned.saturating_add(1);
    }

    /// Stable `path:line: rule -- message` line for each finding.
    #[must_use]
    pub fn render(&self) -> String {
        self.findings
            .iter()
            .map(|f| format!("{}:{}: {} -- {}\n", f.path, f.line, f.rule, f.message))
            .collect()
    }
}

/// Typed process/disposition convention used by every lane binary.
///
/// `LaneExit::Clean` and `LaneExit::NotApplicable` both map to process exit
/// code `0`, but they remain distinct lane/report dispositions: CI process
/// success differs from the receipt/report meaning that a lane had no valid
/// subject to judge. Other codes are `1` = violations, `2` = usage/config
/// error, `3` = upstream dependency missing or fixture self-test failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneExit {
    Clean,
    NotApplicable,
    Violations,
    Usage,
    Failure,
}

impl LaneExit {
    /// Stable process exit code. [`LaneExit::NotApplicable`] returns `0`
    /// because a non-applicable lane is a successful process completion.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        match self {
            LaneExit::Clean => 0,
            LaneExit::NotApplicable => 0,
            LaneExit::Violations => 1,
            LaneExit::Usage => 2,
            LaneExit::Failure => 3,
        }
    }
}

/// Small wrapper around `std::process::ExitCode` so bins can `run` and
/// the test harness can `assert_eq!` on the underlying value.
#[must_use]
pub fn exit(code: LaneExit) -> std::process::ExitCode {
    std::process::ExitCode::from(code.as_u8())
}

#[cfg(test)]
mod tests {
    use super::LaneExit;

    #[test]
    fn not_applicable_is_successful_process_exit_with_distinct_disposition() {
        assert_eq!(LaneExit::NotApplicable.as_u8(), 0);
        assert_ne!(LaneExit::NotApplicable, LaneExit::Clean);
    }
}
