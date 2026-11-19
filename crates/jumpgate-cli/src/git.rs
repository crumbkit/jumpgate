//! The processes the command starts to read a checkout: `git remote get-url
//! origin` and `git ls-files`. The other process the command starts is `board`, in `board.rs`.
//!
//! This module is a process owner (PROC-1) and a clock owner (BASE-5),
//! because the run's deadline is a point in time.

#![expect(
    clippy::disallowed_methods,
    reason = "this module owns the git process and the deadline of its run"
)]

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crumbkit_process::{Capture, Outcome, RunOptions, Status};

/// How long git has to print the URL of a remote.
const DEADLINE: Duration = Duration::from_secs(10);

/// The most bytes of a remote's URL that the command reads.
const URL_LIMIT: usize = 4096;

/// The most bytes of the list of a checkout's `starmap.toml` files that the
/// command reads.
const LIST_LIMIT: usize = 1024 * 1024;

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
    run("git".as_ref(), dir, DEADLINE)
}

/// Runs `program` as `git remote get-url origin` and returns the URL.
fn run(program: &OsStr, dir: &Path, deadline: Duration) -> Result<String, GitError> {
    run_git(
        program,
        dir,
        &["remote", "get-url", "origin"],
        URL_LIMIT,
        deadline,
    )
    .map(|text| text.trim().to_owned())
}

/// Returns the paths of the `starmap.toml` files that the checkout at `dir`
/// tracks, at any depth, each relative to the checkout.
///
/// The paths are relative to `dir` and only files below it are listed, so a
/// checkout that is a subdirectory of a larger repository lists its own files.
pub(crate) fn tracked_starmaps(dir: &Path) -> Result<Vec<String>, GitError> {
    list_starmaps("git".as_ref(), dir, LIST_LIMIT, DEADLINE)
}

/// Runs `program` as `git ls-files` for `dir`, so a test can stand in a
/// script for git and shrink the limit and the deadline.
fn list_starmaps(
    program: &OsStr,
    dir: &Path,
    limit: usize,
    deadline: Duration,
) -> Result<Vec<String>, GitError> {
    let text = run_git(
        program,
        dir,
        &["ls-files", "-z", "--", ":(glob)**/starmap.toml"],
        limit,
        deadline,
    )?;
    Ok(text
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Runs `program` as `git` with `args` and gives it `deadline` to finish, so
/// a test can stand in a script for git. It returns at most `limit` bytes of
/// output.
fn run_git(
    program: &OsStr,
    dir: &Path,
    args: &[&str],
    limit: usize,
    deadline: Duration,
) -> Result<String, GitError> {
    let mut command = Command::new(program);
    command
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0");
    let options = RunOptions::new()
        .stdout(Capture::whole(limit))
        .stderr(Capture::discard())
        .deadline(Instant::now() + deadline);
    let finished = crumbkit_process::run(command, &options).map_err(|_| GitError::Start)?;
    let outcome = finished.outcome();
    if outcome == Outcome::Ended(Status::Code(0)) {
        return String::from_utf8(finished.stdout().bytes().to_vec()).map_err(|_| GitError::Output);
    }
    if outcome == Outcome::TimedOut {
        return Err(GitError::TimedOut);
    }
    if matches!(outcome, Outcome::Stopped(_)) {
        return Err(GitError::Output);
    }
    Err(GitError::Failed)
}

#[cfg(test)]
mod tests;
