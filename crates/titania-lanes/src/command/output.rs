use std::process::ExitStatus;

use super::{LaneError, OutputStream};

/// The result of a captured subprocess run.
#[derive(Debug)]
pub struct CommandOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    program: String,
}

impl CommandOutput {
    pub(super) fn new(
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        program: String,
    ) -> Self {
        Self { status, stdout, stderr, program }
    }

    #[must_use]
    pub fn success(&self) -> bool {
        self.status.success()
    }

    /// Decode stdout as UTF-8.
    pub fn stdout_str(&self) -> Result<&str, LaneError> {
        std::str::from_utf8(&self.stdout).map_err(|_| LaneError::NonUtf8Output {
            program: self.program.clone(),
            stream: OutputStream::Stdout,
        })
    }

    /// Decode stderr as UTF-8.
    pub fn stderr_str(&self) -> Result<&str, LaneError> {
        std::str::from_utf8(&self.stderr).map_err(|_| LaneError::NonUtf8Output {
            program: self.program.clone(),
            stream: OutputStream::Stderr,
        })
    }

    /// Convert a non-zero status to [`LaneError::NonZeroExit`].
    pub fn into_result(self) -> Result<Self, LaneError> {
        if self.status.success() {
            Ok(self)
        } else {
            let code = self.status.code();
            let stderr = self.stderr_str()?.to_owned();
            Err(LaneError::NonZeroExit { program: self.program, code, stderr })
        }
    }
}
