//! Kani harnesses for pure titania-core value invariants.
//!
//! These harnesses stay behind `cfg(kani)` so normal builds keep zero Kani
//! dependency surface. They prove constructor boundaries for the new receipt
//! domain without touching filesystem-backed TargetProject behavior.

use crate::{LaneDigest, LaneName, ReceiptError, ReceiptLaneExit, RecordedTargetRoot};

#[kani::proof]
fn lane_name_rejects_empty_string() {
    let result = LaneName::new("");
    kani::assert(matches!(result, Err(ReceiptError::EmptyLaneName)), "empty lane rejected");
}

#[kani::proof]
fn lane_name_rejects_nul_byte() {
    let result = LaneName::new("fmt\0clippy");
    kani::assert(matches!(result, Err(ReceiptError::InvalidLaneName)), "nul lane rejected");
}

#[kani::proof]
fn lane_digest_rejects_passed_greater_than_scanned() {
    let scanned: u32 = kani::any();
    let passed: u32 = kani::any();
    kani::assume(passed > scanned);
    kani::cover!(passed > scanned, "passed greater than scanned reachable");
    let lane = match LaneName::new("fmt") {
        Ok(lane) => lane,
        Err(_) => return,
    };
    let result = LaneDigest::new(lane, ReceiptLaneExit::Clean, scanned, passed, 0);
    match result {
        Err(ReceiptError::PassedExceedsScanned { passed: got_passed, scanned: got_scanned }) => {
            kani::assert(got_passed == passed, "reported passed count is preserved");
            kani::assert(got_scanned == scanned, "reported scanned count is preserved");
        }
        _ => kani::assert(false, "passed count greater than scanned is rejected exactly"),
    }
}

#[kani::proof]
fn lane_digest_accepts_passed_not_greater_than_scanned() {
    let scanned: u32 = kani::any();
    let passed: u32 = kani::any();
    kani::assume(passed <= scanned);
    kani::cover!(passed == scanned, "passed equal to scanned reachable");
    kani::cover!(passed < scanned, "passed below scanned reachable");
    let lane = match LaneName::new("fmt") {
        Ok(lane) => lane,
        Err(_) => return,
    };
    let result = LaneDigest::new(lane, ReceiptLaneExit::Clean, scanned, passed, 0);
    match result {
        Ok(lane_digest) => {
            kani::assert(lane_digest.lane().as_str() == "fmt", "lane name is preserved");
            kani::assert(lane_digest.exit() == ReceiptLaneExit::Clean, "lane exit is preserved");
            kani::assert(lane_digest.scanned() == scanned, "scanned count is preserved");
            kani::assert(lane_digest.passed() == passed, "passed count is preserved");
            kani::assert(lane_digest.finding_count() == 0, "finding count is preserved");
        }
        Err(_) => kani::assert(false, "passed count below or equal to scanned is accepted"),
    }
}

#[kani::proof]
fn recorded_target_root_rejects_empty_string() {
    let result = RecordedTargetRoot::new("");
    kani::assert(
        matches!(result, Err(ReceiptError::TargetRootEmpty)),
        "empty target root rejected",
    );
}

#[kani::proof]
fn recorded_target_root_rejects_relative_path() {
    let result = RecordedTargetRoot::new("relative/project");
    kani::assert(
        matches!(result, Err(ReceiptError::TargetRootNonAbsolute(_))),
        "relative target root rejected",
    );
}

#[kani::proof]
fn recorded_target_root_rejects_nul_byte() {
    let result = RecordedTargetRoot::new("/tmp/project\0bad");
    kani::assert(
        matches!(result, Err(ReceiptError::TargetRootContainsNul)),
        "nul target root rejected",
    );
}

#[kani::proof]
fn recorded_target_root_accepts_absolute_path() {
    let result = RecordedTargetRoot::new("/tmp/project");
    match result {
        Ok(root) => {
            kani::assert(root.as_str() == "/tmp/project", "target root string is preserved");
        }
        Err(_) => kani::assert(false, "absolute target root is accepted"),
    }
}
