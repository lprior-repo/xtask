//! Quality receipts for target-project gate runs.
//!
//! A receipt is the stable, serializable evidence envelope that says which
//! project was judged and which digests/results were observed. Construction
//! keeps invalid lane summaries and unsupported schemas out of the domain.

use serde::{Deserialize, Deserializer, Serialize};

use crate::{Digest, TargetProject, error::ReceiptError};
mod digests;
mod lane_name;
mod schema;
mod serde_support;
mod target_root;

pub use digests::ReceiptDigests;
pub use lane_name::LaneName;
pub use schema::RECEIPT_SCHEMA_VERSION;
pub use target_root::RecordedTargetRoot;

/// Receipt-local subprocess outcome.
///
/// This mirrors the lane exit-code contract without making `titania-core`
/// depend on the `titania-lanes` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptLaneExit {
    Clean,
    Violations,
    Usage,
    Failure,
}

/// Per-lane digest summary embedded in a [`QualityReceipt`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct LaneDigest {
    lane: LaneName,
    exit: ReceiptLaneExit,
    scanned: u32,
    passed: u32,
    finding_count: u32,
}

#[derive(Deserialize)]
struct LaneDigestWire {
    lane: LaneName,
    exit: ReceiptLaneExit,
    scanned: u32,
    passed: u32,
    finding_count: u32,
}

impl<'de> Deserialize<'de> for LaneDigest {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let wire = LaneDigestWire::deserialize(de)?;
        Self::new(wire.lane, wire.exit, wire.scanned, wire.passed, wire.finding_count)
            .map_err(serde::de::Error::custom)
    }
}

impl LaneDigest {
    /// Construct a validated per-lane receipt summary.
    ///
    /// # Errors
    /// - [`ReceiptError::PassedExceedsScanned`] if `passed > scanned`.
    pub fn new(
        lane: LaneName,
        exit: ReceiptLaneExit,
        scanned: u32,
        passed: u32,
        finding_count: u32,
    ) -> Result<Self, ReceiptError> {
        if passed > scanned {
            return Err(ReceiptError::PassedExceedsScanned { passed, scanned });
        }
        Ok(Self { lane, exit, scanned, passed, finding_count })
    }

    /// Lane name.
    #[must_use]
    pub const fn lane(&self) -> &LaneName {
        &self.lane
    }

    /// Lane exit outcome.
    #[must_use]
    pub const fn exit(&self) -> ReceiptLaneExit {
        self.exit
    }

    /// Files/items scanned by the lane.
    #[must_use]
    pub const fn scanned(&self) -> u32 {
        self.scanned
    }

    /// Files/items accepted by the lane.
    #[must_use]
    pub const fn passed(&self) -> u32 {
        self.passed
    }

    /// Findings emitted by the lane.
    #[must_use]
    pub const fn finding_count(&self) -> u32 {
        self.finding_count
    }
}

/// Validated start and finish timestamps for a receipt run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiptPeriod {
    started_at: u64,
    finished_at: u64,
}

impl ReceiptPeriod {
    /// Construct receipt timing from Unix-second timestamps.
    ///
    /// # Errors
    /// - [`ReceiptError::FinishedBeforeStarted`] if `finished_at < started_at`.
    pub const fn new(started_at: u64, finished_at: u64) -> Result<Self, ReceiptError> {
        if finished_at < started_at {
            return Err(ReceiptError::FinishedBeforeStarted { started_at, finished_at });
        }
        Ok(Self { started_at, finished_at })
    }

    /// Run start time, in Unix seconds.
    #[must_use]
    pub const fn started_at(&self) -> u64 {
        self.started_at
    }

    /// Run finish time, in Unix seconds.
    #[must_use]
    pub const fn finished_at(&self) -> u64 {
        self.finished_at
    }
}

/// Stable quality receipt envelope for one target-project run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QualityReceipt {
    schema_version: u32,
    target_root: RecordedTargetRoot,
    started_at: u64,
    finished_at: u64,
    lane_results: Vec<LaneDigest>,
    source_digest: Digest,
    lock_digest: Digest,
    policy_digest: Digest,
    toolchain_digest: Digest,
}

impl QualityReceipt {
    /// Construct a receipt produced by the current schema.
    ///
    /// # Errors
    /// - [`ReceiptError::FinishedBeforeStarted`] if `finished_at < started_at`.
    pub fn new(
        target_root: &TargetProject,
        period: ReceiptPeriod,
        lane_results: Vec<LaneDigest>,
        digests: ReceiptDigests,
    ) -> Result<Self, ReceiptError> {
        Self::from_parts(
            RECEIPT_SCHEMA_VERSION,
            RecordedTargetRoot::from_target_project(target_root),
            period,
            lane_results,
            digests,
        )
    }

    fn from_parts(
        schema_version: u32,
        target_root: RecordedTargetRoot,
        period: ReceiptPeriod,
        lane_results: Vec<LaneDigest>,
        digests: ReceiptDigests,
    ) -> Result<Self, ReceiptError> {
        if schema_version != RECEIPT_SCHEMA_VERSION {
            return Err(ReceiptError::UnsupportedSchemaVersion(schema_version));
        }
        let ReceiptPeriod { started_at, finished_at } = period;
        let (source_digest, lock_digest, policy_digest, toolchain_digest) = digests.into_parts();
        Ok(Self {
            schema_version,
            target_root,
            started_at,
            finished_at,
            lane_results,
            source_digest,
            lock_digest,
            policy_digest,
            toolchain_digest,
        })
    }

    /// Receipt schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Target project that was judged.
    #[must_use]
    pub const fn target_root(&self) -> &RecordedTargetRoot {
        &self.target_root
    }

    /// Run start time, in Unix seconds.
    #[must_use]
    pub const fn started_at(&self) -> u64 {
        self.started_at
    }

    /// Run finish time, in Unix seconds.
    #[must_use]
    pub const fn finished_at(&self) -> u64 {
        self.finished_at
    }

    /// Per-lane summaries.
    #[must_use]
    pub fn lane_results(&self) -> &[LaneDigest] {
        &self.lane_results
    }

    /// Source digest.
    #[must_use]
    pub const fn source_digest(&self) -> &Digest {
        &self.source_digest
    }

    /// Cargo.lock digest.
    #[must_use]
    pub const fn lock_digest(&self) -> &Digest {
        &self.lock_digest
    }

    /// Policy digest.
    #[must_use]
    pub const fn policy_digest(&self) -> &Digest {
        &self.policy_digest
    }

    /// Toolchain digest.
    #[must_use]
    pub const fn toolchain_digest(&self) -> &Digest {
        &self.toolchain_digest
    }
}
