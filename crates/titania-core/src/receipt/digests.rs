use crate::Digest;

/// Named digest bundle for one quality receipt.
///
/// Keeping the four receipt digests in a named value object prevents callers
/// from passing source, lock, policy, and toolchain digests as an ambiguous
/// positional quartet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReceiptDigests {
    source: Digest,
    lock: Digest,
    policy: Digest,
    toolchain: Digest,
}

impl ReceiptDigests {
    /// Construct the digest bundle stored in a [`crate::QualityReceipt`].
    #[must_use]
    pub const fn new(source: Digest, lock: Digest, policy: Digest, toolchain: Digest) -> Self {
        Self { source, lock, policy, toolchain }
    }

    /// Source tree digest.
    #[must_use]
    pub const fn source(&self) -> &Digest {
        &self.source
    }

    /// Cargo.lock digest.
    #[must_use]
    pub const fn lock(&self) -> &Digest {
        &self.lock
    }

    /// Policy/config digest.
    #[must_use]
    pub const fn policy(&self) -> &Digest {
        &self.policy
    }

    /// Toolchain digest.
    #[must_use]
    pub const fn toolchain(&self) -> &Digest {
        &self.toolchain
    }

    pub(crate) fn into_parts(self) -> (Digest, Digest, Digest, Digest) {
        (self.source, self.lock, self.policy, self.toolchain)
    }
}
