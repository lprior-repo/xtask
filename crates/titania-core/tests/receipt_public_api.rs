use std::{error::Error, fs, path::Path};

use titania_core::{
    Digest, LaneDigest, LaneName, QualityReceipt, RECEIPT_SCHEMA_VERSION, ReceiptDigests,
    ReceiptLaneExit, ReceiptPeriod, TargetProject,
};

type TestResult = Result<(), Box<dyn Error>>;

fn target_project(root: &Path) -> Result<TargetProject, Box<dyn Error>> {
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"receipt-demo\"\n")?;
    Ok(TargetProject::try_from_path(root)?)
}

fn digest(seed: &'static [u8]) -> Digest {
    Digest::from_bytes(seed)
}

#[test]
fn quality_receipt_public_api_round_trips_target_root_and_lane_results() -> TestResult {
    let temp = tempfile::tempdir()?;
    let target = target_project(temp.path())?;
    let lane = LaneDigest::new(LaneName::new("cargo_fmt")?, ReceiptLaneExit::Clean, 4, 4, 0)?;

    let receipt = QualityReceipt::new(
        &target,
        ReceiptPeriod::new(100, 105)?,
        vec![lane],
        ReceiptDigests::new(
            digest(b"source"),
            digest(b"lock"),
            digest(b"policy"),
            digest(b"toolchain"),
        ),
    )?;

    let encoded = serde_json::to_string(&receipt)?;
    let decoded: QualityReceipt = serde_json::from_str(&encoded)?;

    assert_eq!(decoded.schema_version(), RECEIPT_SCHEMA_VERSION);
    assert_eq!(decoded.started_at(), 100);
    assert_eq!(decoded.finished_at(), 105);
    assert_eq!(decoded.lane_results().len(), 1);
    assert_eq!(decoded.lane_results()[0].lane().as_str(), "cargo_fmt");
    assert_eq!(decoded.lane_results()[0].exit(), ReceiptLaneExit::Clean);
    assert_eq!(decoded.lane_results()[0].scanned(), 4);
    assert_eq!(decoded.lane_results()[0].passed(), 4);
    assert_eq!(decoded.lane_results()[0].finding_count(), 0);
    assert!(decoded.target_root().manifest_path().as_str().ends_with("Cargo.toml"));
    assert_eq!(decoded.source_digest().as_hex().len(), 64);
    assert_eq!(decoded.lock_digest().as_hex().len(), 64);
    assert_eq!(decoded.policy_digest().as_hex().len(), 64);
    assert_eq!(decoded.toolchain_digest().as_hex().len(), 64);
    Ok(())
}

#[test]
fn quality_receipt_public_api_rejects_legacy_schema() -> TestResult {
    let temp = tempfile::tempdir()?;
    let target = target_project(temp.path())?;
    let receipt = QualityReceipt::new(
        &target,
        ReceiptPeriod::new(100, 100)?,
        vec![],
        ReceiptDigests::new(
            digest(b"source"),
            digest(b"lock"),
            digest(b"policy"),
            digest(b"toolchain"),
        ),
    )?;
    let mut value = serde_json::to_value(receipt)?;
    value["schema_version"] = serde_json::Value::from(1_u32);

    let err = serde_json::from_value::<QualityReceipt>(value)
        .err()
        .ok_or_else(|| std::io::Error::other("legacy schema was accepted"))?;
    assert!(err.to_string().contains("unsupported receipt schema version 1"));
    Ok(())
}

#[test]
fn quality_receipt_public_api_rejects_future_schema() -> TestResult {
    let temp = tempfile::tempdir()?;
    let target = target_project(temp.path())?;
    let receipt = QualityReceipt::new(
        &target,
        ReceiptPeriod::new(100, 100)?,
        vec![],
        ReceiptDigests::new(
            digest(b"source"),
            digest(b"lock"),
            digest(b"policy"),
            digest(b"toolchain"),
        ),
    )?;
    let mut value = serde_json::to_value(receipt)?;
    value["schema_version"] = serde_json::Value::from(RECEIPT_SCHEMA_VERSION + 1);

    let err = serde_json::from_value::<QualityReceipt>(value)
        .err()
        .ok_or_else(|| std::io::Error::other("future schema was accepted"))?;
    assert!(
        err.to_string().contains(&format!(
            "unsupported receipt schema version {}",
            RECEIPT_SCHEMA_VERSION + 1
        ))
    );
    Ok(())
}
