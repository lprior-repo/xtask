/// Current receipt JSON schema version produced by titania-core.
pub const RECEIPT_SCHEMA_VERSION: u32 = 2;

/// Return true when a receipt schema version is supported by this build.
#[must_use]
pub(crate) const fn is_supported_receipt_schema_version(schema_version: u32) -> bool {
    schema_version == RECEIPT_SCHEMA_VERSION
}
