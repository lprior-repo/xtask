use core::fmt;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{TargetProject, error::ReceiptError};

/// Lexical target-root path recorded in an archived quality receipt.
///
/// Unlike [`TargetProject`], this type performs no filesystem I/O while
/// deserializing. Receipts must remain readable after the judged project is
/// moved, unmounted, or deleted.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordedTargetRoot(Utf8PathBuf);

impl RecordedTargetRoot {
    /// Construct a recorded target root from a UTF-8 path string.
    ///
    /// # Errors
    /// - [`ReceiptError::TargetRootEmpty`] if the path is empty.
    /// - [`ReceiptError::TargetRootNonAbsolute`] if the path is relative.
    /// - [`ReceiptError::TargetRootContainsNul`] if the path contains NUL.
    pub fn new(path: impl Into<Utf8PathBuf>) -> Result<Self, ReceiptError> {
        let path = path.into();
        validate_recorded_root(&path)?;
        Ok(Self(path))
    }

    pub(crate) fn from_target_project(target: &TargetProject) -> Self {
        Self(target.as_path().to_owned())
    }

    /// Borrow the recorded path.
    #[must_use]
    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }

    /// Borrow the recorded path as UTF-8.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Recorded manifest path: `{target_root}/Cargo.toml`.
    #[must_use]
    pub fn manifest_path(&self) -> Utf8PathBuf {
        self.0.join("Cargo.toml")
    }
}

fn validate_recorded_root(path: &Utf8Path) -> Result<(), ReceiptError> {
    if path.as_str().is_empty() {
        return Err(ReceiptError::TargetRootEmpty);
    }
    if !path.is_absolute() {
        return Err(ReceiptError::TargetRootNonAbsolute(path.to_string()));
    }
    if path.as_str().as_bytes().contains(&b'\0') {
        return Err(ReceiptError::TargetRootContainsNul);
    }
    Ok(())
}

impl fmt::Display for RecordedTargetRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl fmt::Debug for RecordedTargetRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RecordedTargetRoot").field(&self.0.as_str()).finish()
    }
}

impl Serialize for RecordedTargetRoot {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for RecordedTargetRoot {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'_, str> as Deserialize>::deserialize(de)?;
        Self::new(Utf8PathBuf::from(s.into_owned())).map_err(serde::de::Error::custom)
    }
}
