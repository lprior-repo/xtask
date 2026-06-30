//! `CommandIn`: single chokepoint for shelling out from a lane.
//!
//! Every subprocess a lane launches MUST go through [`CommandIn`] so
//! that the working directory, environment policy, execution budget,
//! output budget, exit-code handling, and UTF-8 decoding behavior are
//! explicit and typed.

mod output;
mod process;
mod reader;

use std::{
    borrow::Cow,
    io,
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

pub use output::CommandOutput;
use process::{configure_process_group, terminate_child_tree};
use reader::{
    ReaderHandle, drain_after_termination, duration_millis, remaining_budget, spawn_reader,
    take_pipe,
};
use smallvec::SmallVec;
use thiserror::Error;
use titania_core::TargetProject;
use wait_timeout::ChildExt;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_MAX_STDOUT: usize = 1024 * 1024;
const DEFAULT_MAX_STDERR: usize = 1024 * 1024;
const TERMINATION_GRACE: Duration = Duration::from_secs(1);

/// Which captured stream failed a command-output invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// Environment policy for a spawned command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvPolicy {
    /// Clear the process environment, then apply explicitly supplied env
    /// pairs. This is the default for deterministic target judgment.
    Clear,
    /// Inherit the parent process environment, then apply explicitly
    /// supplied env pairs.
    Inherit,
}

/// Bounded execution policy for a spawned command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandBudget {
    pub timeout: Duration,
    pub max_stdout: usize,
    pub max_stderr: usize,
}

impl Default for CommandBudget {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_stdout: DEFAULT_MAX_STDOUT,
            max_stderr: DEFAULT_MAX_STDERR,
        }
    }
}

/// Errors produced by [`CommandIn`].
#[derive(Debug, Error)]
pub enum LaneError {
    #[error("command program must not be empty")]
    EmptyProgram,
    #[error("command program must not contain NUL bytes")]
    InvalidProgram,
    #[error("I/O error running {program}: {source}")]
    Io {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("subprocess {program} exited with code {code:?}: {stderr}")]
    NonZeroExit { program: String, code: Option<i32>, stderr: String },
    #[error("subprocess {program} produced non-UTF-8 {stream:?}")]
    NonUtf8Output { program: String, stream: OutputStream },
    #[error("subprocess {program} timed out after {timeout_ms} ms")]
    Timeout { program: String, timeout_ms: u64 },
    #[error("subprocess {program} exceeded {stream:?} output limit of {limit} bytes")]
    OutputLimitExceeded { program: String, stream: OutputStream, limit: usize },
    #[error("subprocess {program} {stream:?} pipe was unavailable")]
    PipeUnavailable { program: String, stream: OutputStream },
    #[error("subprocess {program} {stream:?} reader thread failed")]
    ReaderThread { program: String, stream: OutputStream },
}

/// A typed builder for `std::process::Command` rooted at a target project.
type EnvPair<'a> = (Cow<'a, str>, Cow<'a, str>);

#[derive(Debug)]
pub struct CommandIn<'a> {
    cwd: &'a TargetProject,
    program: Cow<'a, str>,
    args: SmallVec<[Cow<'a, str>; 8]>,
    env: SmallVec<[EnvPair<'a>; 4]>,
    env_remove: SmallVec<[Cow<'a, str>; 4]>,
    env_policy: EnvPolicy,
    budget: CommandBudget,
}

impl<'a> CommandIn<'a> {
    /// Create a new `CommandIn` for `program` inside the target project.
    ///
    /// # Errors
    /// [`LaneError::EmptyProgram`] if `program` is empty;
    /// [`LaneError::InvalidProgram`] if it contains NUL bytes.
    pub fn new(cwd: &'a TargetProject, program: &'a str) -> Result<Self, LaneError> {
        validate_program(program)?;
        Ok(Self {
            cwd,
            program: Cow::Borrowed(program),
            args: SmallVec::new(),
            env: SmallVec::new(),
            env_remove: SmallVec::new(),
            env_policy: EnvPolicy::Clear,
            budget: CommandBudget::default(),
        })
    }

    /// Append a single argument. Returns `&mut self` for chaining.
    pub fn arg(&mut self, a: &'a str) -> &mut Self {
        self.args.push(Cow::Borrowed(a));
        self
    }

    /// Append multiple arguments. Returns `&mut self` for chaining.
    pub fn args(&mut self, as_: &'a [&'a str]) -> &mut Self {
        self.args.extend(as_.iter().map(|s| Cow::Borrowed(*s)));
        self
    }

    /// Set an environment variable in the spawned process.
    pub fn env(&mut self, k: &'a str, v: &'a str) -> &mut Self {
        self.env.push((Cow::Borrowed(k), Cow::Borrowed(v)));
        self
    }

    /// Remove an inherited environment variable from the spawned process.
    pub fn env_remove(&mut self, k: &'a str) -> &mut Self {
        self.env_remove.push(Cow::Borrowed(k));
        self
    }

    /// Explicitly inherit the parent environment.
    pub fn inherit_env(&mut self) -> &mut Self {
        self.env_policy = EnvPolicy::Inherit;
        self
    }

    /// Replace the default execution/output budget.
    pub fn budget(&mut self, budget: CommandBudget) -> &mut Self {
        self.budget = budget;
        self
    }

    /// Run the subprocess, capture stdout/stderr, enforce the execution
    /// budget, and reject non-zero exits.
    pub fn run(&self) -> Result<CommandOutput, LaneError> {
        self.run_capture_raw()?.into_result()
    }

    /// Alias for [`CommandIn::run`]: checked captured execution.
    pub fn run_capture(&self) -> Result<CommandOutput, LaneError> {
        self.run()
    }

    /// Run the subprocess, capture stdout/stderr, and enforce execution
    /// and output budgets without checking the exit status.
    pub fn run_capture_raw(&self) -> Result<CommandOutput, LaneError> {
        let started = Instant::now();
        let mut cmd = self.base_command();
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|source| self.io_error(source))?;
        let stdout = take_pipe(&mut child.stdout, self.program_name(), OutputStream::Stdout)?;
        let stderr = take_pipe(&mut child.stderr, self.program_name(), OutputStream::Stderr)?;
        let stdout_reader =
            spawn_reader(stdout, self.budget.max_stdout, self.program_name(), OutputStream::Stdout);
        let stderr_reader =
            spawn_reader(stderr, self.budget.max_stderr, self.program_name(), OutputStream::Stderr);
        let status = match child
            .wait_timeout(self.budget.timeout)
            .map_err(|source| self.io_error(source))?
        {
            Some(status) => status,
            None => return self.timeout_child(child, stdout_reader, stderr_reader),
        };
        let stdout = self.receive_reader(stdout_reader, started)?;
        let stderr = self.receive_reader(stderr_reader, started)?;
        Ok(CommandOutput::new(status, stdout, stderr, self.program_name()))
    }

    /// Run the subprocess with inherited stdout/stderr and enforce the
    /// execution budget without checking the exit status.
    pub fn run_status_raw(&self) -> Result<ExitStatus, LaneError> {
        let mut cmd = self.base_command();
        cmd.stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| self.io_error(source))?;
        match child.wait_timeout(self.budget.timeout).map_err(|source| self.io_error(source))? {
            Some(status) => Ok(status),
            None => {
                terminate_child_tree(&mut child, self.program_name())?;
                child.wait().map_err(|source| self.io_error(source))?;
                Err(self.timeout_error())
            }
        }
    }

    fn base_command(&self) -> Command {
        let mut cmd = Command::new(self.program.as_ref());
        configure_process_group(&mut cmd);
        cmd.current_dir(self.cwd.as_std_path());
        if self.env_policy == EnvPolicy::Clear {
            cmd.env_clear();
        }
        cmd.args(self.args.iter().map(|a| a.as_ref()));
        cmd.envs(self.env.iter().map(|(k, v)| (k.as_ref(), v.as_ref())));
        for key in &self.env_remove {
            cmd.env_remove(key.as_ref());
        }
        cmd
    }

    fn timeout_child(
        &self,
        mut child: Child,
        stdout_reader: ReaderHandle,
        stderr_reader: ReaderHandle,
    ) -> Result<CommandOutput, LaneError> {
        terminate_child_tree(&mut child, self.program_name())?;
        child.wait().map_err(|source| self.io_error(source))?;
        drain_after_termination(stdout_reader, self.timeout_error())?;
        drain_after_termination(stderr_reader, self.timeout_error())?;
        Err(self.timeout_error())
    }

    fn receive_reader(&self, reader: ReaderHandle, started: Instant) -> Result<Vec<u8>, LaneError> {
        match reader.recv_timeout(remaining_budget(started, self.budget.timeout)) {
            Ok(out) => Ok(out),
            Err(LaneError::Timeout { .. }) => Err(self.timeout_error()),
            Err(e) => Err(e),
        }
    }

    fn timeout_error(&self) -> LaneError {
        LaneError::Timeout {
            program: self.program_name(),
            timeout_ms: duration_millis(self.budget.timeout),
        }
    }

    fn io_error(&self, source: io::Error) -> LaneError {
        LaneError::Io { program: self.program_name(), source }
    }

    fn program_name(&self) -> String {
        self.program.to_string()
    }
}

fn validate_program(program: &str) -> Result<(), LaneError> {
    if program.is_empty() {
        Err(LaneError::EmptyProgram)
    } else if program.contains('\0') {
        Err(LaneError::InvalidProgram)
    } else {
        Ok(())
    }
}
