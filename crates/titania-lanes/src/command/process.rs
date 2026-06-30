use std::process::{Child, Command};

use super::LaneError;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(unix)]
pub(super) fn configure_process_group(cmd: &mut Command) {
    cmd.process_group(0);
}

#[cfg(not(unix))]
pub(super) fn configure_process_group(_cmd: &mut Command) {}

pub(super) fn terminate_child_tree(child: &mut Child, program: String) -> Result<(), LaneError> {
    terminate_process_group(child.id());
    child.kill().map_err(|source| LaneError::Io { program, source })
}

#[cfg(unix)]
fn terminate_process_group(child_id: u32) {
    let group = format!("-{child_id}");
    let status = Command::new("/bin/kill").arg("-TERM").arg(group).status();
    match status {
        Ok(_) | Err(_) => (),
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_child_id: u32) {}
