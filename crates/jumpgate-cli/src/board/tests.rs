#![expect(
    clippy::disallowed_methods,
    reason = "the tests write the scripts that stand in for board"
)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::*;

/// Writes an executable shell script with `body` and returns its path.
fn script(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("board");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

const LONG: Duration = Duration::from_secs(10);

#[test]
fn the_output_of_board_is_returned_and_the_scope_leads_the_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let board = script(dir.path(), "echo \"$@\"");
    let personal = run(
        board.as_os_str(),
        &Scope::Personal,
        &["ls", "/"],
        None,
        LONG,
    )
    .unwrap();
    assert_eq!(personal, "--personal --no-input ls /\n");
    let scope = Scope::Directory(PathBuf::from("/work/notes"));
    let other = run(board.as_os_str(), &scope, &["ls", "/"], None, LONG).unwrap();
    assert_eq!(other, "-C /work/notes --no-input ls /\n");
}

#[test]
fn the_input_reaches_the_standard_input_of_board() {
    let dir = tempfile::tempdir().unwrap();
    let board = script(dir.path(), "cat");
    let text = run(
        board.as_os_str(),
        &Scope::Personal,
        &["put", "--batch"],
        Some(b"{\"path\": \"/a\"}\n".to_vec()),
        LONG,
    )
    .unwrap();
    assert_eq!(text, "{\"path\": \"/a\"}\n");
}

#[test]
fn a_board_that_is_not_there_is_a_start_failure() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("no-board");
    assert_eq!(
        run(missing.as_os_str(), &Scope::Personal, &[], None, LONG),
        Err(BoardError::Start)
    );
}

#[test]
fn a_board_that_outlasts_its_deadline_is_timed_out() {
    let dir = tempfile::tempdir().unwrap();
    let board = script(dir.path(), "sleep 30");
    assert_eq!(
        run(
            board.as_os_str(),
            &Scope::Personal,
            &[],
            None,
            Duration::from_millis(200)
        ),
        Err(BoardError::TimedOut)
    );
}

#[test]
fn a_failure_status_is_a_refusal_that_carries_the_code_board_printed() {
    let dir = tempfile::tempdir().unwrap();
    let board = script(
        dir.path(),
        "echo 'error: board::path::not_found (nothing at /x)' >&2; exit 1",
    );
    assert_eq!(
        run(board.as_os_str(), &Scope::Personal, &[], None, LONG),
        Err(BoardError::Refused(Some(
            "board::path::not_found".to_owned()
        )))
    );
    let silent = script(dir.path(), "exit 3");
    assert_eq!(
        run(silent.as_os_str(), &Scope::Personal, &[], None, LONG),
        Err(BoardError::Refused(None))
    );
}

#[test]
fn output_that_is_not_utf8_is_an_output_failure() {
    let dir = tempfile::tempdir().unwrap();
    let board = script(dir.path(), "printf '\\377\\376'");
    assert_eq!(
        run(board.as_os_str(), &Scope::Personal, &[], None, LONG),
        Err(BoardError::Output)
    );
}

#[test]
fn a_code_is_found_in_text_and_nothing_else_is_taken_for_one() {
    assert_eq!(
        code_in("refused: board::schema::invalid: a ref").as_deref(),
        Some("board::schema::invalid")
    );
    assert_eq!(code_in("no code here"), None);
    assert_eq!(code_in("board:: alone"), None);
}
