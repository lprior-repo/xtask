use std::convert::TryFrom;

pub(super) const HOT_CRATES: &[&str] = &["titania-core", "titania-lanes"];

pub(super) const COLD_MARKERS: &[&str] = &[
    "diagnostic",
    "diagnostics",
    "fixture",
    "fixtures",
    "harness",
    "kani",
    "loom",
    "proof",
    "property",
    "proptest",
    "proptests",
    "support",
    "test_util",
    "tests",
    "verification",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct FindingData {
    pub(super) rel_path: String,
    pub(super) line_no: usize,
    pub(super) class_id: &'static str,
    pub(super) text: String,
}

impl FindingData {
    pub(super) fn line_no_as_u32(&self) -> u32 {
        u32::try_from(self.line_no).unwrap_or(u32::MAX)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceRole {
    HotProduction,
    LaneBinary,
    Test,
    ColdSupport,
}
