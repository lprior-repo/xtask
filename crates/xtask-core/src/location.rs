//! Location types: where a finding was detected.

use serde::{Deserialize, Serialize};

/// A normalized workspace-relative path. No backslashes, no `..`, no absolute paths.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspacePath(pub String);

impl WorkspacePath {
    /// Create a validated workspace path.
    ///
    /// # Errors
    /// Returns an error if the path contains backslashes, `..`, or is absolute.
    pub fn new(path: String) -> Result<Self, WorkspacePathError> {
        if path.contains('\\') {
            return Err(WorkspacePathError::Backslash);
        }
        if path.contains("/../") || path == ".." || path.starts_with("../") {
            return Err(WorkspacePathError::ParentTraversal);
        }
        if path.starts_with('/') {
            return Err(WorkspacePathError::Absolute);
        }
        Ok(Self(path))
    }
}

/// Error from invalid workspace path construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspacePathError {
    /// Path contains backslashes.
    Backslash,
    /// Path contains `..` parent traversal.
    ParentTraversal,
    /// Path is absolute.
    Absolute,
}

/// Where a finding was detected. Not all findings have source spans.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Location {
    /// A source code location.
    Span {
        file: WorkspacePath,
        /// 1-based line number.
        line_start: u32,
        /// 0-based column (Unicode scalar values).
        col_start: u32,
        /// 1-based line number.
        line_end: u32,
        /// 0-based column.
        col_end: u32,
    },
    /// A dependency finding (advisory, banned crate).
    Dependency { crate_name: String, version: String },
    /// A manifest finding.
    Manifest { file: WorkspacePath },
    /// A workspace-level finding.
    Workspace,
    /// A tool finding.
    Tool { name: String, version: String },
}
