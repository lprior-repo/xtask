//! UTF-8 byte range in a source file. Half-open: `[start_byte, end_byte)`.
//! Used for deterministic patches and stable cross-platform finding
//! reproduction.
//!
//! Invariants enforced by construction:
//! - `start_byte <= end_byte`
//! - both fields are `u32`, so the maximum range is 4 GiB.

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::TextRangeError;

/// A UTF-8 byte range within a single source file.
///
/// Once constructed, [`TextRange::start`] ≤ [`TextRange::end`] is
/// guaranteed by the type system.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextRange {
    start_byte: u32,
    end_byte: u32,
}

impl TextRange {
    /// Construct a half-open byte range.
    ///
    /// # Errors
    /// [`TextRangeError::EndBeforeStart`] if `end < start`.
    pub const fn new(start_byte: u32, end_byte: u32) -> Result<Self, TextRangeError> {
        if end_byte < start_byte {
            return Err(TextRangeError::EndBeforeStart { start: start_byte, end: end_byte });
        }
        Ok(Self { start_byte, end_byte })
    }

    /// Inclusive start byte position.
    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start_byte
    }

    /// Exclusive end byte position.
    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end_byte
    }

    /// Number of bytes covered. Always non-negative: `end - start` cannot
    /// underflow because the constructor rejects `end < start`.
    #[must_use]
    pub const fn width(&self) -> u32 {
        // SAFETY: the constructor guarantees end >= start, so subtraction
        // never underflows. We use saturating_sub purely to satisfy
        // arithmetic-side-effect lint; the result equals `end - start`.
        self.end_byte.saturating_sub(self.start_byte)
    }

    /// Whether this range covers zero bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start_byte == self.end_byte
    }

    /// Whether `byte` is in `[self.start, self.end)`.
    #[must_use]
    pub const fn contains_byte(&self, byte: u32) -> bool {
        byte >= self.start_byte && byte < self.end_byte
    }

    /// Whether this range and `other` share any byte position.
    #[must_use]
    pub const fn overlaps(&self, other: &Self) -> bool {
        self.start_byte < other.end_byte && other.start_byte < self.end_byte
    }
}

impl fmt::Display for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}..{})", self.start_byte, self.end_byte)
    }
}

impl fmt::Debug for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TextRange(start={}, end={})", self.start_byte, self.end_byte)
    }
}

impl Serialize for TextRange {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("TextRange", 2)?;
        s.serialize_field("start_byte", &self.start_byte)?;
        s.serialize_field("end_byte", &self.end_byte)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for TextRange {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            start_byte: u32,
            end_byte: u32,
        }
        let r = Repr::deserialize(de)?;
        Self::new(r.start_byte, r.end_byte).map_err(serde::de::Error::custom)
    }
}
