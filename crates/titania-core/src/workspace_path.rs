//! Workspace-relative path string. Validated so the inner value is always
//! a relative, sanitized UTF-8 path suitable for human-readable output,
//! JSON serialization, and stable cross-platform hashing.
//!
//! Invariants enforced by construction:
//! - Non-empty.
//! - No leading `/`.
//! - No `..` segment anywhere (no `..` as a complete path segment).
//! - No `\` (we use forward-slash paths only).
//! - No NUL byte.
//! - No control characters (0x00–0x1F except in the byte stream; tab 0x09 is rejected).

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use unicode_normalization::UnicodeNormalization;

use crate::error::WorkspacePathError;

/// A validated workspace-relative POSIX path string.
///
/// Compare with [`std::path::Path`] which is host-OS dependent; a
/// [`WorkspacePath`] is always `str`-safe, forward-slash, and unambiguous
/// across Unix and Windows.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkspacePath(String);

impl WorkspacePath {
    /// Construct a validated workspace path. Forward slashes, no `..`
    /// segments, no leading slash, no backslash, no control bytes.
    ///
    /// # Errors
    /// See [`WorkspacePathError`] for the full taxonomy.
    pub fn new(s: &str) -> Result<Self, WorkspacePathError> {
        Self::validate(s)?;
        Ok(Self(s.nfc().collect()))
    }

    /// Borrow the underlying path string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Number of segments (split on `/`). A path with no slashes has 1
    /// segment.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.0.split('/').filter(|s| !s.is_empty()).count()
    }

    /// Whether this path starts with the given prefix segment (e.g.
    /// `WorkspacePath("src/foo.rs").starts_with("src") == true`).
    #[must_use]
    pub fn starts_with_segment(&self, segment: &str) -> bool {
        self.0.split('/').next().is_some_and(|first| first == segment)
    }

    fn validate(s: &str) -> Result<(), WorkspacePathError> {
        if s.is_empty() {
            return Err(WorkspacePathError::Empty);
        }
        if s.starts_with('/') {
            return Err(WorkspacePathError::LeadingSlash);
        }
        let bytes = s.as_bytes();
        for &b in bytes {
            if b == b'\\' {
                return Err(WorkspacePathError::ContainsBackslash);
            }
            if b == 0 {
                return Err(WorkspacePathError::ContainsNull);
            }
            if b < 0x20 {
                return Err(WorkspacePathError::ControlByte(b));
            }
        }
        // Check for '..' as a complete segment.
        for seg in s.split('/') {
            if seg == ".." {
                return Err(WorkspacePathError::ContainsDotDot);
            }
        }
        Ok(())
    }
}

impl fmt::Display for WorkspacePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for WorkspacePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorkspacePath({})", self.0)
    }
}

impl AsRef<str> for WorkspacePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Serialize for WorkspacePath {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WorkspacePath {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'_, str> as Deserialize>::deserialize(de)?;
        Self::new(&s).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<&str> for WorkspacePath {
    type Error = WorkspacePathError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}
