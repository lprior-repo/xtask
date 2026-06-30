//! Pure domain types for titania-check. Zero IO, zero async, zero unsafe.
//!
//! Each public type has a smart constructor that returns a `Result`. Once
//! constructed, all invariants are type-enforced: there is no way to produce
//! an invalid value of these types without going through the constructor.
//!
//! See `crates/titania-core/src/*.rs` for the primitive definitions and
//! `crates/titania-core/tests/*.rs` for the property- and behavior-tests.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::string_slice)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::arithmetic_side_effects)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::as_conversions)]
#![forbid(unsafe_code)]

mod digest;
mod discover;
mod error;
#[cfg(kani)]
mod kani;
mod receipt;
mod rule_id;
mod target_project;
mod text_range;
mod workspace_path;

pub use digest::Digest;
pub use discover::discover_target;
pub use error::{
    CoreError, DigestError, ReceiptError, RuleIdError, TargetProjectError, TextRangeError,
    WorkspacePathError,
};
pub use receipt::{
    LaneDigest, LaneName, QualityReceipt, RECEIPT_SCHEMA_VERSION, ReceiptDigests, ReceiptLaneExit,
    ReceiptPeriod, RecordedTargetRoot,
};
pub use rule_id::RuleId;
pub use target_project::TargetProject;
pub use text_range::TextRange;
pub use workspace_path::WorkspacePath;
