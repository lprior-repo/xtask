//! 64-character lowercase hex content digest. Backed by a `String` because
//! the byte representation is 64 ASCII chars; `String` keeps the API safe
//! (no `unsafe` needed) and the allocation cost is amortized at
//! construction.
//!
//! Construction is total: [`Digest::from_hex`] returns `Result`, and
//! [`Digest::from_bytes`] is infallible because blake3's hex output is
//! contractually 64 lowercase hex characters.

use core::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::DigestError;

/// Number of hex characters in a hex-encoded 256-bit digest.
pub(crate) const DIGEST_HEX_LEN: usize = 64;

/// A validated 64-character lowercase-hex content digest.
///
/// Once constructed, the inner string is guaranteed to be exactly 64 ASCII
/// characters and every byte is in `[0-9a-f]`. The constructor returns
/// `Result`; there is no public API to produce an invalid value.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Digest(String);

impl Digest {
    /// Compute a digest from arbitrary bytes using blake3 and hex-encode
    /// the result to lowercase ASCII. Total.
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Self {
        // blake3's hex output is contractually 64 lowercase hex chars.
        Self(blake3::hash(data).to_hex().to_string())
    }

    /// Parse a 64-character lowercase-hex string into a [`Digest`].
    ///
    /// # Errors
    /// - [`DigestError::WrongLength`] if the input length is not 64.
    /// - [`DigestError::NonHexChar`] at the first non-`[0-9a-f]` byte.
    pub fn from_hex(hex: &str) -> Result<Self, DigestError> {
        let bytes = hex.as_bytes();
        if bytes.len() != DIGEST_HEX_LEN {
            return Err(DigestError::WrongLength(bytes.len()));
        }
        for (i, &b) in bytes.iter().enumerate() {
            if !is_lower_hex(b) {
                return Err(DigestError::NonHexChar(i));
            }
        }
        Ok(Self(hex.to_owned()))
    }

    /// Borrow the underlying lowercase-hex string.
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({})", self.0)
    }
}

impl FromStr for Digest {
    type Err = DigestError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl Serialize for Digest {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'_, str> as Deserialize>::deserialize(de)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

#[must_use]
const fn is_lower_hex(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f')
}
