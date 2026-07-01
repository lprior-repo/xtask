//! Rule identifier. A namespaced, uppercase-ASCII identifier with at least
//! one underscore, e.g. `FUNC_LOOPS_FOR`, `CLIPPY_UNWRAP_USED`,
//! `ARCHITECTURE_IMPORT_CORE_FS`.
//!
//! Construction is total: [`RuleId::new`] validates and returns the value
//! or a [`RuleIdError`].

use core::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::RuleIdError;

/// A validated rule identifier string.
///
/// Once constructed, the inner string is guaranteed to be:
/// - non-empty,
/// - all uppercase ASCII (`A-Z`, `0-9`),
/// - containing at least one underscore (`_`).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RuleId(String);

impl RuleId {
    /// Maximum length of a rule identifier.
    pub const MAX_LEN: usize = 96;

    /// Construct a [`RuleId`] from any input. Lowercase letters and mixed
    /// input are rejected — call [`RuleId::normalize`] first if you have
    /// untrusted casing.
    ///
    /// # Errors
    /// - [`RuleIdError::Empty`] if `s` is empty.
    /// - [`RuleIdError::TooLong`] if `s` exceeds [`Self::MAX_LEN`].
    /// - [`RuleIdError::NoUnderscore`] if `s` has no underscore.
    /// - [`RuleIdError::NotUppercase`] if `s` contains any character that
    ///   is not uppercase ASCII (`A-Z`, `0-9`).
    pub fn new(s: &str) -> Result<Self, RuleIdError> {
        if s.is_empty() {
            return Err(RuleIdError::Empty);
        }
        if s.len() > Self::MAX_LEN {
            return Err(RuleIdError::TooLong { len: s.len(), max: Self::MAX_LEN });
        }
        let mut has_underscore = false;
        for (i, c) in s.char_indices() {
            if c == '_' {
                has_underscore = true;
                continue;
            }
            if !matches!(c, 'A'..='Z' | '0'..='9') {
                return Err(RuleIdError::NotUppercase(c, i));
            }
        }
        if !has_underscore {
            return Err(RuleIdError::NoUnderscore);
        }
        Ok(Self(s.to_owned()))
    }

    /// Normalize input to a rule identifier: uppercase ASCII, drop illegal
    /// characters, then validate. Returns the same errors as [`RuleId::new`].
    pub fn normalize(s: &str) -> Result<Self, RuleIdError> {
        let mut buf = String::with_capacity(s.len());
        for ch in s.chars() {
            if ch.is_ascii_lowercase() {
                buf.push(ch.to_ascii_uppercase());
            } else if ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_' {
                buf.push(ch);
            }
            // other chars are dropped; validation will catch empty / no-underscore.
        }
        Self::new(&buf)
    }

    /// Borrow the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Prefix before the first underscore (e.g. `FUNC` in `FUNC_LOOPS_FOR`).
    ///
    /// # Panics
    /// Cannot panic: our type invariant guarantees that `self.0`
    /// contains only uppercase ASCII, digits, and `_`. Any byte index
    /// returned by `find('_')` therefore lies on a UTF-8 character
    /// boundary, so the slice is well-defined.
    #[must_use]
    #[allow(clippy::string_slice)]
    pub fn prefix(&self) -> &str {
        match self.0.find('_') {
            Some(i) => &self.0[..i],
            None => &self.0,
        }
    }

    /// Whether this rule id has the given prefix.
    #[must_use]
    pub fn has_prefix(&self, p: &str) -> bool {
        self.prefix() == p
    }

    /// Compile-time-friendly constructor for `&'static str` literals.
    ///
    /// Returns a typed [`RuleId`] or a [`RuleIdError`]. Designed for the
    /// `RULE_*` constants in each lane: the literal is validated at startup
    /// (via [`Self::validate_many`]), and the typed value is held in a
    /// `OnceLock<RuleId>` in each bin.
    ///
    /// This is intentionally a regular `fn`, not a `const fn` -- a const
    /// constructor would need a stable `String::to_owned` in const context
    /// which Rust 2026 nightly does not yet provide. Validation logic is
    /// the same as [`Self::new`]; only the `&'static str` constraint is
    /// added.
    pub fn new_const(value: &'static str) -> Result<Self, RuleIdError> {
        Self::new(value)
    }

    /// Validate a list of `&'static str` literals as rule identifiers.
    /// Returns the first invalid literal as `Err`. Used by each lane's main
    /// to type-check its `RULE_*` constants at startup.
    ///
    /// # Errors
    /// - [`RuleIdError::Empty`] if any literal is empty.
    /// - [`RuleIdError::TooLong`] if any literal exceeds [`Self::MAX_LEN`].
    /// - [`RuleIdError::NoUnderscore`] if any literal has no underscore.
    /// - [`RuleIdError::NotUppercase`] if any literal has a non-uppercase byte.
    pub fn validate_many(ids: &[&'static str]) -> Result<(), (usize, RuleIdError)> {
        for (index, id) in ids.iter().enumerate() {
            if let Err(error) = Self::new(id) {
                return Err((index, error));
            }
        }
        Ok(())
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// `clippy::string_slice` lint flags `&self.0[..i]` for any byte index.
// Our invariant guarantees rule ids are uppercase ASCII (and `_`),
// so every byte index lies on a UTF-8 character boundary; the slice is
// sound by construction. We silence the lint at the next impl block
// and document the safety argument above.
impl fmt::Debug for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RuleId({})", self.0)
    }
}

impl FromStr for RuleId {
    type Err = RuleIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Serialize for RuleId {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RuleId {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'_, str> as Deserialize>::deserialize(de)?;
        Self::new(&s).map_err(serde::de::Error::custom)
    }
}
