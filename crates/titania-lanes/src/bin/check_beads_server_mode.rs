//! Checks `.beads/metadata.json` mode without assuming every target project
//! must use the same Beads backend topology.
//!
//! The original velvet-ballistics lane required server-mode Dolt everywhere.
//! Titania itself currently uses embedded Dolt, so this lane now parses the
//! metadata into typed policy values and rejects malformed/contradictory
//! states while treating embedded mode as an explicit supported outcome.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::fs;

use serde_json::Value;
use titania_lanes::{Finding, LaneExit, LaneReport, exit};

const METADATA_PATH: &str = ".beads/metadata.json";
const EMBEDDED_MARKER: &str = ".beads/embeddeddolt";

const RULE_BACKEND: &str = "BEADS-BACKEND-001";
const RULE_DOLT_MODE: &str = "BEADS-MODE-001";
const RULE_DOLT_PORT: &str = "BEADS-DOLT-PORT-001";
const RULE_EMBEDDED_MARKER: &str = "BEADS-EMBEDDED-MARKER-001";
const RULE_METADATA_MISSING: &str = "BEADS-METADATA-MISSING-001";
const RULE_METADATA_PARSE: &str = "BEADS-METADATA-PARSE-001";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Dolt,
    Other,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoltMode {
    Server,
    Embedded,
    Other,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BeadsMetadata {
    backend: Backend,
    mode: DoltMode,
    pins_server_port: bool,
}

impl BeadsMetadata {
    fn parse(text: &str) -> Result<Self, serde_json::Error> {
        let value = serde_json::from_str::<Value>(text)?;
        Ok(Self {
            backend: backend_from(value_text(&value, "backend")),
            mode: mode_from(value_text(&value, "dolt_mode")),
            pins_server_port: value.get("dolt_server_port").is_some(),
        })
    }

    fn check(self, report: &mut LaneReport) {
        check_backend(self.backend, report);
        check_mode(self.mode, report);
        check_port_pin(self.pins_server_port, report);
    }
}

fn value_text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn backend_from(value: Option<&str>) -> Backend {
    match value {
        Some("dolt") => Backend::Dolt,
        Some(_) => Backend::Other,
        None => Backend::Missing,
    }
}

fn mode_from(value: Option<&str>) -> DoltMode {
    match value {
        Some("server") => DoltMode::Server,
        Some("embedded") => DoltMode::Embedded,
        Some(_) => DoltMode::Other,
        None => DoltMode::Missing,
    }
}

fn check_backend(backend: Backend, report: &mut LaneReport) {
    if backend != Backend::Dolt {
        report.push(Finding::new(
            RULE_BACKEND,
            METADATA_PATH,
            0,
            ".beads/metadata.json must declare backend \"dolt\"",
        ));
    }
}

fn check_mode(mode: DoltMode, report: &mut LaneReport) {
    match mode {
        DoltMode::Server | DoltMode::Embedded => {}
        DoltMode::Missing => report.push(Finding::new(
            RULE_DOLT_MODE,
            METADATA_PATH,
            0,
            ".beads/metadata.json must declare dolt_mode",
        )),
        DoltMode::Other => report.push(Finding::new(
            RULE_DOLT_MODE,
            METADATA_PATH,
            0,
            ".beads/metadata.json contains unsupported dolt_mode",
        )),
    }
}

fn check_port_pin(pins_server_port: bool, report: &mut LaneReport) {
    if pins_server_port {
        report.push(Finding::new(
            RULE_DOLT_PORT,
            METADATA_PATH,
            0,
            "do not pin dolt_server_port in metadata; bd owns runtime routing",
        ));
    }
}

fn check_embedded_marker(mode: DoltMode, report: &mut LaneReport) {
    if mode == DoltMode::Server && fs::metadata(EMBEDDED_MARKER).is_ok() {
        report.push(Finding::new(
            RULE_EMBEDDED_MARKER,
            EMBEDDED_MARKER,
            0,
            ".beads/embeddeddolt conflicts with server-mode metadata",
        ));
    }
}

fn check_metadata(text: &str, report: &mut LaneReport) -> Option<BeadsMetadata> {
    report.record_scan();
    match BeadsMetadata::parse(text) {
        Ok(metadata) => {
            metadata.check(report);
            Some(metadata)
        }
        Err(error) => {
            report.push(Finding::new(
                RULE_METADATA_PARSE,
                METADATA_PATH,
                0,
                format!(".beads/metadata.json is not valid JSON: {error}"),
            ));
            None
        }
    }
}

fn main() -> std::process::ExitCode {
    let mut report = LaneReport::new();

    let metadata = match fs::read_to_string(METADATA_PATH) {
        Ok(text) => text,
        Err(error) => {
            report.push(Finding::new(
                RULE_METADATA_MISSING,
                METADATA_PATH,
                0,
                format!(".beads/metadata.json is missing: {error}"),
            ));
            eprint!("{}", report.render());
            return exit(LaneExit::Violations);
        }
    };

    if let Some(parsed) = check_metadata(&metadata, &mut report) {
        check_embedded_marker(parsed.mode, &mut report);
    }

    eprint!("{}", report.render());
    if report.is_clean() {
        eprintln!("beads metadata mode check passed");
        exit(LaneExit::Clean)
    } else {
        exit(LaneExit::Violations)
    }
}
