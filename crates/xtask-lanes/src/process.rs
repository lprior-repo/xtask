//! Shared process execution helpers for running external tools.

use std::process::Command;
use std::time::Duration;

use xtask_core::ProcessTermination;

/// Error from running an external process.
#[derive(Debug)]
pub enum ProcessError {
    /// Command could not be spawned.
    SpawnFailed { program: String, reason: String },
    /// Waiting on the process failed.
    WaitFailed { program: String, reason: String },
}

/// Result of running an external process.
pub struct ProcessResult {
    /// How the process terminated.
    pub termination: ProcessTermination,
    /// Captured stdout (may be large for JSON output).
    pub stdout: Vec<u8>,
    /// Captured stderr.
    pub stderr: Vec<u8>,
}

impl ProcessResult {
    /// Returns true if the process exited successfully (code 0).
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self.termination, ProcessTermination::Exited { code: 0 })
    }

    /// Returns stdout as a UTF-8 string lossy.
    #[must_use]
    pub fn stdout_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    /// Returns stderr as a UTF-8 string lossy.
    #[must_use]
    pub fn stderr_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }
}

/// Run a command with a timeout, capturing stdout and stderr.
///
/// # Errors
/// Returns an error string if the command cannot be spawned or waited on.
pub fn run_command(
    program: &str,
    args: &[&str],
    cwd: Option<&std::path::Path>,
    timeout: Duration,
) -> Result<ProcessResult, ProcessError> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let mut child = cmd.spawn().map_err(|e| ProcessError::SpawnFailed {
        program: program.to_owned(),
        reason: e.to_string(),
    })?;

    match wait_with_timeout(&mut child, timeout) {
        WaitOutcome::Exited(status) => {
            let (stdout, stderr) = drain_child(&mut child);
            let termination = map_exit_status(status);
            Ok(ProcessResult {
                termination,
                stdout,
                stderr,
            })
        }
        WaitOutcome::TimedOut => {
            drop(child.kill());
            drop(child.wait());
            Ok(ProcessResult {
                termination: ProcessTermination::TimedOut,
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
        WaitOutcome::Error(e) => Err(ProcessError::WaitFailed {
            program: program.to_owned(),
            reason: e.to_string(),
        }),
    }
}

enum WaitOutcome {
    Exited(std::process::ExitStatus),
    TimedOut,
    Error(std::io::Error),
}

fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> WaitOutcome {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return WaitOutcome::Exited(status),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    return WaitOutcome::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return WaitOutcome::Error(e),
        }
    }
}

fn drain_child(child: &mut std::process::Child) -> (Vec<u8>, Vec<u8>) {
    use std::io::Read;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(ref mut out) = child.stdout {
        drop(out.read_to_end(&mut stdout));
    }
    if let Some(ref mut err) = child.stderr {
        drop(err.read_to_end(&mut stderr));
    }
    (stdout, stderr)
}

fn map_exit_status(status: std::process::ExitStatus) -> ProcessTermination {
    if let Some(code) = status.code() {
        return ProcessTermination::Exited { code };
    }
    // On Unix, no exit code means killed by signal.
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let signal = status.signal().map_or(-1, |sig| sig);
        ProcessTermination::Signaled { signal }
    }
    #[cfg(not(unix))]
    {
        ProcessTermination::Signaled { signal: -1 }
    }
}
