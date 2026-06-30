//! Typed errors for the domain primitives. One error enum per constructor,
//! using `thiserror` so the messages are stable and machine-consumable.

use std::io;

use thiserror::Error;

/// Errors produced by [`crate::Digest::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DigestError {
    #[error("digest must be exactly 64 characters, got {0}")]
    WrongLength(usize),
    #[error("digest must contain only lowercase hex characters; bad position {0}")]
    NonHexChar(usize),
}

/// Errors produced by [`crate::RuleId::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuleIdError {
    #[error("rule id must not be empty")]
    Empty,
    #[error("rule id must contain at least one underscore")]
    NoUnderscore,
    #[error("rule id must be uppercase ASCII; bad character {0:?} at byte {1}")]
    NotUppercase(char, usize),
}

/// Errors produced by [`crate::WorkspacePath::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkspacePathError {
    #[error("workspace path must not be empty")]
    Empty,
    #[error("workspace path must not start with '/'")]
    LeadingSlash,
    #[error("workspace path must not contain '..'")]
    ContainsDotDot,
    #[error("workspace path must not contain backslashes")]
    ContainsBackslash,
    #[error("workspace path must not contain null bytes")]
    ContainsNull,
    #[error("workspace path must not contain control characters; bad byte {0}")]
    ControlByte(u8),
}

/// Errors produced by [`crate::TextRange::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TextRangeError {
    #[error("text range end ({end}) must be >= start ({start})")]
    EndBeforeStart { start: u32, end: u32 },
}

/// Errors produced by [`crate::TargetProject::try_from_path`] and
/// [`crate::discover_target`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TargetProjectError {
    #[error("target project path must not be empty")]
    Empty,
    #[error("target project path must be absolute, got {0:?}")]
    NonAbsolute(String),
    #[error("target project path is not valid UTF-8")]
    NotUtf8,
    #[error("target project path does not exist")]
    NotFound,
    #[error("target project path exists but is not a directory")]
    NotADirectory,
    #[error("target project directory does not contain a Cargo.toml file")]
    NoCargoToml,
    #[error("target project Cargo.toml path exists but is not a file")]
    CargoTomlNotFile,
    #[error("target project Cargo.toml is malformed: {path}")]
    MalformedCargoToml { path: String },
    #[error("I/O error accessing {path}: {kind:?}")]
    Io { path: String, kind: io::ErrorKind },
}

/// Errors produced by [`crate::QualityReceipt`] and [`crate::LaneDigest`]
/// constructors or deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReceiptError {
    #[error("unsupported receipt schema version {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("lane name must not be empty")]
    EmptyLaneName,
    #[error("lane name must not contain NUL bytes")]
    InvalidLaneName,
    #[error("lane passed count {passed} exceeds scanned count {scanned}")]
    PassedExceedsScanned { passed: u32, scanned: u32 },
    #[error("receipt finished_at {finished_at} is before started_at {started_at}")]
    FinishedBeforeStarted { started_at: u64, finished_at: u64 },
    #[error("receipt target_root must not be empty")]
    TargetRootEmpty,
    #[error("receipt target_root must be absolute, got {0:?}")]
    TargetRootNonAbsolute(String),
    #[error("receipt target_root must not contain NUL bytes")]
    TargetRootContainsNul,
}

/// Aggregate for callers that want a single error type across primitives.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error(transparent)]
    Digest(#[from] DigestError),
    #[error(transparent)]
    RuleId(#[from] RuleIdError),
    #[error(transparent)]
    WorkspacePath(#[from] WorkspacePathError),
    #[error(transparent)]
    TextRange(#[from] TextRangeError),
    #[error(transparent)]
    TargetProject(#[from] TargetProjectError),
    #[error(transparent)]
    Receipt(#[from] ReceiptError),
}
