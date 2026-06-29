//! Typed errors for the domain primitives. One error enum per constructor,
//! using `thiserror` so the messages are stable and machine-consumable.

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
}
