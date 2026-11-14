#![expect(
    clippy::disallowed_methods,
    reason = "the tests write the scripts that stand in for git"
)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::*;

/// Writes an executable shell script with `body` and returns its path.
fn script(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("git");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

const LONG: Duration = Duration::from_secs(10);

#[test]
fn the_url_git_prints_is_returned_trimmed() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "echo https://github.com/acme/shelf.git");
    assert_eq!(
        run(git.as_os_str(), dir.path(), LONG).as_deref(),
        Ok("https://github.com/acme/shelf.git")
    );
}

#[test]
fn a_git_that_outlasts_its_deadline_is_timed_out() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "sleep 30");
    assert_eq!(
        run(git.as_os_str(), dir.path(), Duration::from_millis(200)),
        Err(GitError::TimedOut)
    );
}

#[test]
fn a_git_that_prints_more_than_a_url_is_an_output_error() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "head -c 100000 /dev/zero | tr '\\0' 'a'");
    assert_eq!(
        run(git.as_os_str(), dir.path(), LONG),
        Err(GitError::Output)
    );
}

#[test]
fn a_git_that_prints_text_that_is_not_utf8_is_an_output_error() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "printf '\\377\\376\\n'");
    assert_eq!(
        run(git.as_os_str(), dir.path(), LONG),
        Err(GitError::Output)
    );
}

#[test]
fn a_git_that_fails_is_a_failure_and_a_missing_git_does_not_start() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "exit 128");
    assert_eq!(
        run(git.as_os_str(), dir.path(), LONG),
        Err(GitError::Failed)
    );
    let missing = dir.path().join("none");
    assert_eq!(
        run(missing.as_os_str(), dir.path(), LONG),
        Err(GitError::Start)
    );
}
