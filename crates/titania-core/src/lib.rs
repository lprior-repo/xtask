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
mod error;
mod rule_id;
mod text_range;
mod workspace_path;

pub use digest::Digest;
pub use error::{CoreError, DigestError, RuleIdError, TextRangeError, WorkspacePathError};
pub use rule_id::RuleId;
pub use text_range::TextRange;
pub use workspace_path::WorkspacePath;
