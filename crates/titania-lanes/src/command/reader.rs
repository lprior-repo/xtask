use std::{
    io::Read,
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use super::{LaneError, OutputStream, TERMINATION_GRACE};

pub(super) struct ReaderHandle {
    thread: thread::JoinHandle<()>,
    rx: Receiver<Result<Vec<u8>, LaneError>>,
    program: String,
    stream: OutputStream,
}

impl ReaderHandle {
    pub(super) fn recv_timeout(self, timeout: Duration) -> Result<Vec<u8>, LaneError> {
        match self.rx.recv_timeout(timeout) {
            Ok(result) => {
                join_finished_reader(self.thread, self.program, self.stream)?;
                result
            }
            Err(RecvTimeoutError::Timeout) => Err(LaneError::Timeout {
                program: self.program,
                timeout_ms: duration_millis(timeout),
            }),
            Err(RecvTimeoutError::Disconnected) => {
                join_finished_reader(self.thread, self.program.clone(), self.stream)?;
                Err(LaneError::ReaderThread { program: self.program, stream: self.stream })
            }
        }
    }
}

pub(super) fn take_pipe<T>(
    pipe: &mut Option<T>,
    program: String,
    stream: OutputStream,
) -> Result<T, LaneError> {
    pipe.take().ok_or(LaneError::PipeUnavailable { program, stream })
}

pub(super) fn spawn_reader<R>(
    pipe: R,
    limit: usize,
    program: String,
    stream: OutputStream,
) -> ReaderHandle
where
    R: Read + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let reader_program = program.clone();
    let thread = thread::spawn(move || {
        let result = read_limited(pipe, limit, reader_program, stream);
        match tx.send(result) {
            Ok(()) | Err(_) => (),
        }
    });
    ReaderHandle { thread, rx, program, stream }
}

pub(super) fn drain_after_termination(
    reader: ReaderHandle,
    timeout_error: LaneError,
) -> Result<(), LaneError> {
    match reader.recv_timeout(TERMINATION_GRACE) {
        Ok(_) => Ok(()),
        Err(LaneError::Timeout { .. }) => Err(timeout_error),
        Err(e) => Err(e),
    }
}

pub(super) fn remaining_budget(started: Instant, budget: Duration) -> Duration {
    match budget.checked_sub(started.elapsed()) {
        Some(remaining) => remaining,
        None => Duration::ZERO,
    }
}

pub(super) fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).map_or(u64::MAX, |ms| ms)
}

fn read_limited<R: Read>(
    pipe: R,
    limit: usize,
    program: String,
    stream: OutputStream,
) -> Result<Vec<u8>, LaneError> {
    let read_limit = u64::try_from(limit.saturating_add(1))
        .map_err(|_| LaneError::OutputLimitExceeded { program: program.clone(), stream, limit })?;
    let mut limited = pipe.take(read_limit);
    let mut out = Vec::new();
    limited
        .read_to_end(&mut out)
        .map_err(|source| LaneError::Io { program: program.clone(), source })?;
    if out.len() > limit {
        Err(LaneError::OutputLimitExceeded { program, stream, limit })
    } else {
        Ok(out)
    }
}

fn join_finished_reader(
    thread: thread::JoinHandle<()>,
    program: String,
    stream: OutputStream,
) -> Result<(), LaneError> {
    match thread.join() {
        Ok(()) => Ok(()),
        Err(_panic) => Err(LaneError::ReaderThread { program, stream }),
    }
}
