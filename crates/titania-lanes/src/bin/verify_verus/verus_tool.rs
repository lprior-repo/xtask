use std::{fs, path::Path};

use titania_core::TargetProject;
use titania_lanes::CommandIn;

use super::registry::ProofTarget;

#[must_use]
pub(crate) fn verus_on_path(target: &TargetProject) -> bool {
    let Ok(mut command) = CommandIn::new(target, "verus") else {
        return false;
    };
    command.inherit_env();
    command.arg("--version");
    command.run_capture_raw().is_ok()
}

pub(crate) fn run_verus_target(
    target: &TargetProject,
    proof_target: &ProofTarget,
    evidence_dir: &Path,
) -> Result<(), String> {
    let log_path = evidence_dir.join(format!("{}.log", safe_log_name(proof_target.path())));
    let mut command =
        CommandIn::new(target, "verus").map_err(|e| format!("failed to prepare verus: {e}"))?;
    command.inherit_env();
    command.arg(proof_target.path());
    let output = command.run_capture_raw().map_err(|e| format!("failed to run verus: {e}"))?;
    write_log(&log_path, &output.stdout, &output.stderr)?;
    if output.success() {
        Ok(())
    } else {
        Err(format!(
            "verus target {} failed with status {:?}; see {}",
            proof_target.path(),
            output.status.code(),
            log_path.display()
        ))
    }
}

fn safe_log_name(proof_target: &str) -> String {
    proof_target
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

fn write_log(path: &Path, stdout: &[u8], stderr: &[u8]) -> Result<(), String> {
    let mut body = String::from_utf8_lossy(stdout).into_owned();
    body.push_str("\n--- stderr ---\n");
    body.push_str(&String::from_utf8_lossy(stderr));
    fs::write(path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))
}
