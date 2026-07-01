use std::{error::Error, fs, path::Path};

use titania_core::{
    Digest, DigestError, LaneDigest, LaneName, QualityReceipt, ReceiptDigests, ReceiptError,
    ReceiptLaneExit, ReceiptPeriod, TargetProject, TargetProjectError,
};

type TestResult = Result<(), Box<dyn Error>>;

fn target_project(root: &Path) -> Result<TargetProject, TargetProjectError> {
    TargetProject::try_from_path(root)
}

fn lane_digest() -> Result<LaneDigest, ReceiptError> {
    LaneDigest::new(LaneName::new("fmt")?, ReceiptLaneExit::Clean, 3, 3, 0)
}

fn receipt(target: TargetProject) -> Result<QualityReceipt, ReceiptError> {
    QualityReceipt::new(
        &target,
        ReceiptPeriod::new(10, 12)?,
        vec![lane_digest()?],
        ReceiptDigests::new(
            Digest::from_bytes(b"source"),
            Digest::from_bytes(b"lock"),
            Digest::from_bytes(b"policy"),
            Digest::from_bytes(b"toolchain"),
        ),
    )
}

#[test]
fn receipt_round_trip_preserves_all_fields() -> TestResult {
    let temp = tempfile::tempdir()?;
    fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"demo\"\n")?;
    let target = target_project(temp.path())?;
    let receipt = receipt(target)?;

    let json = serde_json::to_string(&receipt)?;
    assert!(json.contains("\"schema_version\":2"));
    assert!(json.contains("\"target_root\""));
    assert!(json.contains("\"lane_results\""));
    assert!(json.contains("\"source_digest\""));
    assert!(json.contains("\"lock_digest\""));
    assert!(json.contains("\"policy_digest\""));
    assert!(json.contains("\"toolchain_digest\""));

    let decoded: QualityReceipt = serde_json::from_str(&json)?;
    assert_eq!(decoded, receipt);
    assert_eq!(
        decoded.target_root().manifest_path().as_str(),
        receipt.target_root().manifest_path().as_str()
    );
    Ok(())
}

#[test]
fn receipt_deserialize_rejects_schema_before_target_root() -> TestResult {
    let temp = tempfile::tempdir()?;
    fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"demo\"\n")?;
    let target = target_project(temp.path())?;
    let valid = receipt(target)?;
    let mut value = serde_json::to_value(valid)?;
    value["schema_version"] = serde_json::Value::from(1_u32);

    let err = serde_json::from_value::<QualityReceipt>(value)
        .err()
        .ok_or_else(|| std::io::Error::other("schema v1 receipt was accepted"))?;
    assert!(err.to_string().contains("unsupported receipt schema version 1"));
    Ok(())
}

#[test]
fn receipt_deserialize_rejects_schema_before_missing_current_fields() -> TestResult {
    let value = serde_json::json!({
        "schema_version": 1_u32,
        "started_at": 10_u64,
        "finished_at": 12_u64
    });

    let err = serde_json::from_value::<QualityReceipt>(value)
        .err()
        .ok_or_else(|| std::io::Error::other("schema v1 receipt was accepted"))?;
    assert!(err.to_string().contains("unsupported receipt schema version 1"));
    Ok(())
}

#[test]
fn receipt_deserialize_rejects_lane_digest_passed_above_scanned() -> TestResult {
    let temp = tempfile::tempdir()?;
    fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"demo\"\n")?;
    let target = target_project(temp.path())?;
    let valid = receipt(target)?;
    let mut value = serde_json::to_value(valid)?;
    value["lane_results"][0]["scanned"] = serde_json::Value::from(1_u32);
    value["lane_results"][0]["passed"] = serde_json::Value::from(2_u32);

    let err = serde_json::from_value::<QualityReceipt>(value)
        .err()
        .ok_or_else(|| std::io::Error::other("invalid lane digest was accepted"))?;
    assert!(err.to_string().contains("lane passed count 2 exceeds scanned count 1"));
    Ok(())
}

#[test]
fn malformed_digest_is_rejected_by_digest_constructor() -> TestResult {
    let err = Digest::from_hex("not-a-64-character-lowercase-hex-digest")
        .err()
        .ok_or_else(|| std::io::Error::other("malformed digest was accepted"))?;
    assert!(matches!(err, DigestError::WrongLength(_)));
    Ok(())
}

#[test]
fn lane_digest_rejects_passed_count_above_scanned_count() -> TestResult {
    let err = LaneDigest::new(LaneName::new("clippy")?, ReceiptLaneExit::Violations, 1, 2, 1)
        .err()
        .ok_or_else(|| std::io::Error::other("invalid lane counts were accepted"))?;
    assert!(matches!(err, ReceiptError::PassedExceedsScanned { scanned: 1, passed: 2 }));
    Ok(())
}

#[test]
fn receipt_rejects_finish_time_before_start_time() -> TestResult {
    let err = ReceiptPeriod::new(12, 10)
        .err()
        .ok_or_else(|| std::io::Error::other("invalid receipt timing was accepted"))?;
    assert!(matches!(err, ReceiptError::FinishedBeforeStarted { started_at: 12, finished_at: 10 }));
    Ok(())
}
