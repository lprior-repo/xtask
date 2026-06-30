use std::{error::Error, io::ErrorKind, time::Duration};

use tempfile::tempdir;
use titania_core::TargetProject;
use titania_lanes::{CommandBudget, CommandIn, LaneError, OutputStream};

type TestResult = Result<(), Box<dyn Error>>;
type FixtureTarget = (tempfile::TempDir, TargetProject);

fn test_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message.into()))
}

fn fixture_target() -> Result<FixtureTarget, Box<dyn Error>> {
    let tmp = tempdir()?;
    std::fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
    let target = TargetProject::try_from_path(tmp.path())?;
    Ok((tmp, target))
}

fn expect_new_error(result: Result<CommandIn<'_>, LaneError>) -> Result<LaneError, Box<dyn Error>> {
    match result {
        Ok(_) => Err(test_error("expected CommandIn::new to fail")),
        Err(err) => Ok(err),
    }
}

fn expect_run_error<T>(result: Result<T, LaneError>) -> Result<LaneError, Box<dyn Error>> {
    match result {
        Ok(_) => Err(test_error("expected command run to fail")),
        Err(err) => Ok(err),
    }
}

#[test]
fn command_public_api_rejects_invalid_program_names() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let empty = expect_new_error(CommandIn::new(&target, ""))?;
    assert!(matches!(empty, LaneError::EmptyProgram));

    let nul = expect_new_error(CommandIn::new(&target, "bad\0program"))?;
    assert!(matches!(nul, LaneError::InvalidProgram));
    Ok(())
}

#[test]
fn command_public_api_args_env_and_cwd_are_subprocess_visible() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let mut command = CommandIn::new(&target, "/bin/sh")?;
    command.args(&[
        "-c",
        "printf '%s:%s:%s' \"$1\" \"$TITANIA_VALUE\" \"$(pwd)\"",
        "ignored",
        "arg",
    ]);
    command.env("TITANIA_VALUE", "env");

    let out = command.run()?;
    let expected = format!("arg:env:{}", target.as_std_path().display());
    assert_eq!(out.stdout_str()?, expected);
    Ok(())
}

#[test]
fn command_public_api_default_env_clear_and_inherit_env_are_observable() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let clear = CommandIn::new(&target, "/usr/bin/env")?.run()?;
    assert_eq!(clear.stdout_str()?, "");

    let mut inherited = CommandIn::new(&target, "/bin/sh")?;
    inherited.inherit_env().arg("-c").arg("printf '%s' \"${PATH:+present}\"");
    let out = inherited.run()?;
    assert_eq!(out.stdout_str()?, "present");
    Ok(())
}

#[test]
fn command_public_api_nonzero_exit_carries_code_and_stderr() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let mut command = CommandIn::new(&target, "/bin/sh")?;
    command.arg("-c").arg("printf err >&2; exit 7");

    let err = expect_run_error(command.run())?;
    match err {
        LaneError::NonZeroExit { program, code, stderr } => {
            assert_eq!(program, "/bin/sh");
            assert_eq!(code, Some(7));
            assert_eq!(stderr, "err");
        }
        other => return Err(test_error(format!("expected NonZeroExit, got {other:?}"))),
    }
    Ok(())
}

#[test]
fn command_public_api_run_capture_is_checked_like_run() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let err = expect_run_error(CommandIn::new(&target, "/bin/false")?.run_capture())?;
    match err {
        LaneError::NonZeroExit { program, code, stderr } => {
            assert_eq!(program, "/bin/false");
            assert_eq!(code, Some(1));
            assert_eq!(stderr, "");
        }
        other => return Err(test_error(format!("expected NonZeroExit, got {other:?}"))),
    }
    Ok(())
}

#[test]
fn command_public_api_spawn_io_error_carries_program_and_kind() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let err = expect_run_error(CommandIn::new(&target, "/does/not/exist")?.run())?;
    match err {
        LaneError::Io { program, source } => {
            assert_eq!(program, "/does/not/exist");
            assert_eq!(source.kind(), ErrorKind::NotFound);
        }
        other => return Err(test_error(format!("expected Io, got {other:?}"))),
    }
    Ok(())
}

#[test]
fn command_public_api_non_utf8_errors_carry_program_and_stream() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let mut stdout = CommandIn::new(&target, "/bin/sh")?;
    stdout.arg("-c").arg("printf '\\377'");
    let stdout_raw = stdout.run_capture_raw()?;
    let stdout_err = expect_run_error(stdout_raw.stdout_str())?;
    match stdout_err {
        LaneError::NonUtf8Output { program, stream } => {
            assert_eq!(program, "/bin/sh");
            assert_eq!(stream, OutputStream::Stdout);
        }
        other => return Err(test_error(format!("expected stdout NonUtf8Output, got {other:?}"))),
    }

    let mut stderr = CommandIn::new(&target, "/bin/sh")?;
    stderr.arg("-c").arg("printf '\\377' >&2; exit 1");
    let stderr_err = expect_run_error(stderr.run())?;
    match stderr_err {
        LaneError::NonUtf8Output { program, stream } => {
            assert_eq!(program, "/bin/sh");
            assert_eq!(stream, OutputStream::Stderr);
        }
        other => return Err(test_error(format!("expected stderr NonUtf8Output, got {other:?}"))),
    }
    Ok(())
}

#[test]
fn command_public_api_timeout_and_output_limits_carry_exact_fields() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let mut timeout = CommandIn::new(&target, "/bin/sh")?;
    timeout.arg("-c").arg("sleep 2");
    timeout.budget(CommandBudget {
        timeout: Duration::from_millis(20),
        max_stdout: 1024,
        max_stderr: 1024,
    });
    let timeout_err = expect_run_error(timeout.run())?;
    match timeout_err {
        LaneError::Timeout { program, timeout_ms } => {
            assert_eq!(program, "/bin/sh");
            assert_eq!(timeout_ms, 20);
        }
        other => return Err(test_error(format!("expected Timeout, got {other:?}"))),
    }

    let mut stdout = CommandIn::new(&target, "/bin/sh")?;
    stdout.arg("-c").arg("printf 1234567890");
    stdout.budget(CommandBudget {
        timeout: Duration::from_secs(1),
        max_stdout: 4,
        max_stderr: 1024,
    });
    let stdout_err = expect_run_error(stdout.run())?;
    match stdout_err {
        LaneError::OutputLimitExceeded { program, stream, limit } => {
            assert_eq!(program, "/bin/sh");
            assert_eq!(stream, OutputStream::Stdout);
            assert_eq!(limit, 4);
        }
        other => {
            return Err(test_error(format!("expected stdout OutputLimitExceeded, got {other:?}")));
        }
    }

    let mut stderr = CommandIn::new(&target, "/bin/sh")?;
    stderr.arg("-c").arg("printf 1234567890 >&2; exit 1");
    stderr.budget(CommandBudget {
        timeout: Duration::from_secs(1),
        max_stdout: 1024,
        max_stderr: 4,
    });
    let stderr_err = expect_run_error(stderr.run())?;
    match stderr_err {
        LaneError::OutputLimitExceeded { program, stream, limit } => {
            assert_eq!(program, "/bin/sh");
            assert_eq!(stream, OutputStream::Stderr);
            assert_eq!(limit, 4);
        }
        other => {
            return Err(test_error(format!("expected stderr OutputLimitExceeded, got {other:?}")));
        }
    }
    Ok(())
}

#[test]
fn command_public_api_env_remove_scrubs_explicit_env() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let mut command = CommandIn::new(&target, "/bin/sh")?;
    command
        .inherit_env()
        .env("TITANIA_DELETE_ME", "present")
        .env_remove("TITANIA_DELETE_ME")
        .arg("-c")
        .arg("printf '%s' \"${TITANIA_DELETE_ME:-gone}\"");

    let out = command.run()?;
    assert_eq!(out.stdout_str()?, "gone");
    Ok(())
}

#[test]
fn command_public_api_run_status_raw_preserves_exit_code() -> TestResult {
    let (_tmp, target) = fixture_target()?;
    let mut command = CommandIn::new(&target, "/bin/sh")?;
    command.arg("-c").arg("exit 4");

    let status = command.run_status_raw()?;
    assert_eq!(status.code(), Some(4));
    Ok(())
}
