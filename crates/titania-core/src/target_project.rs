//! External Rust project on disk. A validated, absolute, UTF-8 directory
//! path containing a `Cargo.toml` file.
//!
//! Distinct from [`crate::WorkspacePath`]: `WorkspacePath` is a validated
//! relative path *string* used for human-readable output and stable
//! cross-platform hashing of finding locations. `TargetProject` is a real
//! filesystem path representing the project being judged — discovered
//! from CWD by [`crate::discover_target`].
//!
//! Invariants enforced by construction and deserialization:
//! - Absolute.
//! - Valid UTF-8.
//! - The path exists and is a directory.
//! - A `Cargo.toml` file is present at the root.
//!
//! Construction is total: [`TargetProject::try_from_path`] returns a
//! `Result`; there is no public API that produces an invalid value.

use core::fmt;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::TargetProjectError;

/// A validated absolute UTF-8 path to a directory containing a
/// `Cargo.toml` file.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetProject(Utf8PathBuf);

impl TargetProject {
    /// Construct a [`TargetProject`] from a filesystem path.
    ///
    /// # Errors
    /// - [`TargetProjectError::Empty`] if `path` is empty after UTF-8
    ///   conversion.
    /// - [`TargetProjectError::NonAbsolute`] if `path` is relative.
    /// - [`TargetProjectError::NotUtf8`] if `path` contains non-UTF-8 bytes.
    /// - [`TargetProjectError::NotFound`] if the path does not exist.
    /// - [`TargetProjectError::NotADirectory`] if the path exists but is
    ///   not a directory.
    /// - [`TargetProjectError::NoCargoToml`] if `Cargo.toml` is absent.
    /// - [`TargetProjectError::CargoTomlNotFile`] if `Cargo.toml` exists
    ///   but is not a file.
    /// - [`TargetProjectError::Io`] for any other filesystem error.
    pub fn try_from_path(path: &std::path::Path) -> Result<Self, TargetProjectError> {
        let utf8_path = Utf8Path::from_path(path).ok_or(TargetProjectError::NotUtf8)?;
        validate_not_empty(utf8_path)?;
        validate_absolute(utf8_path)?;
        validate_root_directory(utf8_path)?;
        validate_manifest_file(&utf8_path.join("Cargo.toml"))?;
        Ok(Self(utf8_path.to_owned()))
    }

    /// Borrow the underlying path as a [`Utf8Path`].
    #[must_use]
    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }

    /// Path to the manifest: `{root}/Cargo.toml`.
    #[must_use]
    pub fn manifest_path(&self) -> Utf8PathBuf {
        self.0.join("Cargo.toml")
    }

    /// Borrow the underlying path as a [`std::path::Path`].
    #[must_use]
    pub fn as_std_path(&self) -> &std::path::Path {
        self.0.as_std_path()
    }
}

fn validate_not_empty(path: &Utf8Path) -> Result<(), TargetProjectError> {
    if path.as_str().is_empty() { Err(TargetProjectError::Empty) } else { Ok(()) }
}

fn validate_absolute(path: &Utf8Path) -> Result<(), TargetProjectError> {
    if path.is_absolute() { Ok(()) } else { Err(TargetProjectError::NonAbsolute(path.to_string())) }
}

fn validate_root_directory(path: &Utf8Path) -> Result<(), TargetProjectError> {
    let metadata = metadata_or(path, TargetProjectError::NotFound)?;
    if metadata.is_dir() { Ok(()) } else { Err(TargetProjectError::NotADirectory) }
}

fn validate_manifest_file(path: &Utf8Path) -> Result<(), TargetProjectError> {
    let metadata = metadata_or(path, TargetProjectError::NoCargoToml)?;
    if metadata.is_file() { Ok(()) } else { Err(TargetProjectError::CargoTomlNotFile) }
}

fn metadata_or(
    path: &Utf8Path,
    missing: TargetProjectError,
) -> Result<std::fs::Metadata, TargetProjectError> {
    std::fs::metadata(path.as_std_path()).map_err(|e| classify_io(&e, path.as_str(), missing))
}

fn classify_io(e: &std::io::Error, path: &str, missing: TargetProjectError) -> TargetProjectError {
    if e.kind() == std::io::ErrorKind::NotFound {
        missing
    } else {
        TargetProjectError::Io { path: path.to_owned(), kind: e.kind() }
    }
}

impl fmt::Display for TargetProject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl fmt::Debug for TargetProject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TargetProject").field(&self.0.as_str()).finish()
    }
}

impl AsRef<Utf8Path> for TargetProject {
    fn as_ref(&self) -> &Utf8Path {
        &self.0
    }
}

impl Serialize for TargetProject {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for TargetProject {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'_, str> as Deserialize>::deserialize(de)?;
        let p = Utf8PathBuf::from(s.into_owned());
        Self::try_from_path(p.as_std_path()).map_err(serde::de::Error::custom)
    }
}
