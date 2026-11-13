//! The one process the command starts: `git remote get-url origin`.
//!
//! This module is the command's process owner (PROC-1) and its clock owner
//! (BASE-5), because the run's deadline is a point in time.

#![expect(
    clippy::disallowed_methods,
    reason = "this module owns the git process and the deadline of its run"
)]

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crumbkit_process::{Capture, Outcome, OverLimit, RunOptions, Status};

/// How long git has to print the URL of a remote.
const DEADLINE: Duration = Duration::from_secs(10);

/// The most bytes of a remote's URL that the command reads.
const URL_LIMIT: usize = 4096;

/// Why git did not give a remote's URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitError {
    /// git did not start, for example because it is not installed.
    Start,
    /// git ended with a failure status, such as a directory that is not a
    /// checkout or a checkout with no `origin`.
    Failed,
    /// git did not finish within the deadline.
    TimedOut,
    /// git printed more than a URL, or text that is not UTF-8.
    Output,
}

/// Returns the URL of the remote `origin` of the checkout at `dir`.
///
/// git runs with no prompt, no standard input and a deadline, and prints
/// nothing to the command's output.
pub(crate) fn origin_url(dir: &Path) -> Result<String, GitError> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["remote", "get-url", "origin"])
        .env("GIT_TERMINAL_PROMPT", "0");
    let options = RunOptions::new()
        .stdout(Capture::Whole {
            limit: URL_LIMIT,
            over: OverLimit::Stop,
        })
        .stderr(Capture::Discard)
        .deadline(Instant::now() + DEADLINE);
    let finished = crumbkit_process::run(command, &options).map_err(|_| GitError::Start)?;
    let outcome = finished.outcome();
    if outcome == Outcome::Ended(Status::Code(0)) {
        return String::from_utf8(finished.stdout().bytes().to_vec())
            .map(|text| text.trim().to_owned())
            .map_err(|_| GitError::Output);
    }
    if outcome == Outcome::TimedOut {
        return Err(GitError::TimedOut);
    }
    if matches!(outcome, Outcome::Stopped(_)) {
        return Err(GitError::Output);
    }
    Err(GitError::Failed)
}
