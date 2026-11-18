//! The `board` process: the one program the command starts besides `git`.
//!
//! This module is the command's second process owner (PROC-1) and, with
//! `git.rs`, its clock owner (BASE-5), because each run has a deadline. The
//! board store runs `board` through [`Board::run`] and never reads or writes
//! a file of a board.

#![expect(
    clippy::disallowed_methods,
    reason = "this module owns the board process and the deadline of its runs"
)]

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crumbkit_process::{Capture, Input, Outcome, RunOptions, Status, Zeroizing};

/// How long one `board` run has to finish.
const DEADLINE: Duration = Duration::from_secs(60);

/// The most bytes of standard output that one run keeps.
const OUTPUT_LIMIT: usize = 64 * 1024 * 1024;

/// The most bytes of the failure output that the module reads to find the
/// code of a refusal.
const FAILURE_LIMIT: usize = 16 * 1024;

/// The prefix of every code that `board` prints for a refusal.
const CODE_PREFIX: &str = "board::";

/// Which board a `board` run reads and writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scope {
    /// The personal board, which `board --personal` finds.
    Personal,
    /// The board in this directory, which `board -C <dir>` opens.
    Directory(PathBuf),
}

/// Why a `board` run gave no output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoardError {
    /// `board` did not start, for example because it is not on `PATH`.
    Start,
    /// `board` did not finish within the deadline.
    TimedOut,
    /// `board` printed more than the module keeps, or text that is not
    /// UTF-8.
    Output,
    /// `board` ended with a failure status. It holds the code that `board`
    /// printed, such as `board::path::not_found`, when there was one.
    Refused(Option<String>),
}

/// The `board` program and the board it works on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Board {
    program: OsString,
    scope: Scope,
}

impl Board {
    /// Works on the board `scope` names with the `board` program that
    /// `PATH` finds.
    pub(crate) fn new(scope: Scope) -> Self {
        Self::with_program("board", scope)
    }

    /// Works on the board `scope` names with `program`, so a test can stand
    /// a script in for `board`.
    pub(crate) fn with_program(program: impl Into<OsString>, scope: Scope) -> Self {
        Self {
            program: program.into(),
            scope,
        }
    }

    /// Runs `board <scope> --no-input <args>` with `input` on its standard
    /// input and returns what it printed. Nothing prompts, because no
    /// question can be answered.
    pub(crate) fn run(&self, args: &[&str], input: Option<Vec<u8>>) -> Result<String, BoardError> {
        run(&self.program, &self.scope, args, input, DEADLINE)
    }
}

/// Runs `program` as `board` and gives it `deadline` to finish.
fn run(
    program: &OsStr,
    scope: &Scope,
    args: &[&str],
    input: Option<Vec<u8>>,
    deadline: Duration,
) -> Result<String, BoardError> {
    let mut command = Command::new(program);
    match scope {
        Scope::Personal => {
            command.arg("--personal");
        }
        Scope::Directory(dir) => {
            command.arg("-C").arg(dir);
        }
    }
    command.arg("--no-input").args(args);
    let options = RunOptions::new()
        .stdin(input.map_or(Input::Null, |bytes| Input::Bytes(Zeroizing::new(bytes))))
        .stdout(Capture::whole(OUTPUT_LIMIT))
        .stderr(Capture::whole(FAILURE_LIMIT))
        .deadline(Instant::now() + deadline);
    let finished = crumbkit_process::run(command, &options).map_err(|_| BoardError::Start)?;
    let outcome = finished.outcome();
    if outcome == Outcome::Ended(Status::Code(0)) {
        if finished.stdout().over_limit() {
            return Err(BoardError::Output);
        }
        return String::from_utf8(finished.stdout().bytes().to_vec())
            .map_err(|_| BoardError::Output);
    }
    if outcome == Outcome::TimedOut {
        return Err(BoardError::TimedOut);
    }
    let mut text = String::from_utf8_lossy(finished.stderr().bytes()).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(finished.stdout().bytes()));
    Err(BoardError::Refused(code_in(&text)))
}

/// Returns the first `board::...` code in `text`.
fn code_in(text: &str) -> Option<String> {
    let start = text.find(CODE_PREFIX)?;
    let code: String = text[start..]
        .chars()
        .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | ':'))
        .collect();
    (code.len() > CODE_PREFIX.len()).then_some(code)
}

#[cfg(test)]
mod tests;
