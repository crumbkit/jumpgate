//! Tests of the board store against a real board in a temporary directory.
//!
//! Each test makes a git repository with `git init`, makes it a board with
//! `board init`, and runs the store against it. The tests never touch the
//! personal board. A machine with no `board` program on `PATH` skips them
//! with a message. CI has no `board` program, so it skips them, and they run
//! on a machine that has one.

#![expect(
    clippy::disallowed_methods,
    reason = "the tests start git and board to make the temporary board they run against"
)]

use std::path::Path;
use std::process::{Command, Stdio};

use jumpgate::{Binding, Need, Offer, Target};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;
use crate::board::Scope;

/// Returns true when a `board` program runs.
fn board_installed() -> bool {
    Command::new("board")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Runs `program` with `args` in `dir` and fails the test when it fails.
fn run(dir: &Path, program: &str, args: &[&str]) {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{program} {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Makes a temporary board and returns it with a store on it, or `None`
/// with a message when `board` is not installed.
fn temporary_board() -> Option<(TempDir, BoardStore)> {
    if !board_installed() {
        skipped();
        return None;
    }
    let dir = tempfile::Builder::new()
        .prefix("jumpgate-")
        .tempdir()
        .unwrap();
    run(dir.path(), "git", &["init", "--quiet"]);
    run(dir.path(), "git", &["config", "user.name", "jumpgate test"]);
    run(
        dir.path(),
        "git",
        &["config", "user.email", "test@example.com"],
    );
    run(dir.path(), "board", &["init", "--no-input"]);
    let board = Board::new(Scope::Directory(dir.path().to_path_buf()));
    let id = "temporary".parse().unwrap();
    Some((dir, BoardStore::new(board, id)))
}

/// Counts the commits of the board in `dir`.
fn commits(dir: &Path) -> usize {
    let output = Command::new("git")
        .args(["rev-list", "--count", "--all"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn need(name: &str) -> Need {
    Need::from_json(
        &json!({
            "$type": "jumpgate/need/v1",
            "app": "shelf",
            "name": name,
            "kind": "secret",
            "consumers": ["web"],
            "declared_in": {"repo": "github.com/acme/shelf", "file": "starmap.toml"},
            "delivers_as": {"variables": ["API_TOKEN"]},
            "optional": false,
            "when": [],
            "when_unbound": "fail"
        })
        .to_string(),
    )
    .unwrap()
}

fn offer() -> Offer {
    Offer::from_json(
        &json!({
            "$type": "jumpgate/offer/v1",
            "provider": {"kind": "secret-store", "name": "one-password"},
            "kind": "secret",
            "name": "api-token",
            "reference": {"secret": "op://example-vault/api-token/credential"},
            "grants": ["shelf"],
            "scope": "workspace"
        })
        .to_string(),
    )
    .unwrap()
}

fn target(name: &str) -> Target {
    Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": name,
            "kind": "local",
            "authority": "temporary"
        })
        .to_string(),
    )
    .unwrap()
}

fn binding(target: &str) -> Binding {
    let text: Value = json!({
        "$type": "jumpgate/binding/v1",
        "need": "/jumpgate/needs/shelf/api-token",
        "offer": "/jumpgate/offers/secret-store/one-password/api-token",
        "target": target
    });
    Binding::from_json(&text.to_string()).unwrap()
}

#[test]
fn a_board_with_no_records_holds_none_of_any_type() {
    let Some((_dir, store)) = temporary_board() else {
        return;
    };
    assert!(store.needs().unwrap().is_empty());
    assert!(store.offers().unwrap().is_empty());
    assert!(store.units().unwrap().is_empty());
    assert!(store.targets().unwrap().is_empty());
    assert!(
        store
            .bindings(&"local".parse().unwrap())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn records_put_in_a_board_are_read_back_at_their_paths() {
    let Some((_dir, store)) = temporary_board() else {
        return;
    };
    let records: Vec<AnyRecord> = vec![
        need("api-token").into(),
        need("session-key").into(),
        offer().into(),
        target("local").into(),
    ];
    store.put(&records).unwrap();
    let needs = store.needs().unwrap();
    let paths: Vec<String> = needs.iter().map(|n| n.record_path().to_string()).collect();
    assert_eq!(
        paths,
        [
            "/jumpgate/needs/shelf/api-token",
            "/jumpgate/needs/shelf/session-key"
        ]
    );
    assert_eq!(needs[0], need("api-token"));
    assert_eq!(store.offers().unwrap(), [offer()]);
    assert_eq!(store.targets().unwrap(), [target("local")]);
    let one = store.get(&need("api-token").record_path()).unwrap();
    assert_eq!(one, Some(AnyRecord::from(need("api-token"))));
    assert_eq!(store.get(&need("other").record_path()).unwrap(), None);
}

#[test]
fn the_first_write_registers_the_five_types_in_the_board() {
    let Some((dir, store)) = temporary_board() else {
        return;
    };
    store.put(&[target("local").into()]).unwrap();
    let rows = store
        .rows("select logical from entries where kind = 'schema' and logical like '/.board/types/jumpgate/%' order by logical")
        .unwrap();
    let schemas: Vec<&str> = rows.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(
        schemas,
        [
            "/.board/types/jumpgate/binding/v1.schema",
            "/.board/types/jumpgate/need/v1.schema",
            "/.board/types/jumpgate/offer/v1.schema",
            "/.board/types/jumpgate/target/v1.schema",
            "/.board/types/jumpgate/unit/v1.schema",
        ]
    );
    // A second write adds one commit and registers nothing twice.
    let before = commits(dir.path());
    store.put(&[need("api-token").into()]).unwrap();
    assert_eq!(commits(dir.path()), before + 1);
}

#[test]
fn a_record_put_again_replaces_the_one_at_its_path() {
    let Some((_dir, store)) = temporary_board() else {
        return;
    };
    store.put(&[need("api-token").into()]).unwrap();
    let mut changed: Value =
        serde_json::from_str(&AnyRecord::from(need("api-token")).to_json().unwrap()).unwrap();
    changed["consumers"] = json!(["web", "worker"]);
    let replaced = Need::from_json(&changed.to_string()).unwrap();
    store.put(&[replaced.clone().into()]).unwrap();
    assert_eq!(store.needs().unwrap(), [replaced]);
}

#[test]
fn apply_writes_and_deletes_in_one_commit_and_skips_a_path_the_board_lacks() {
    let Some((dir, store)) = temporary_board() else {
        return;
    };
    store
        .put(&[need("api-token").into(), need("old").into()])
        .unwrap();
    let before = commits(dir.path());
    let gone = [
        need("old").record_path(),
        need("never-written").record_path(),
        need("api-token").record_path(),
    ];
    store
        .apply(&[need("api-token").into(), need("fresh").into()], &gone)
        .unwrap();
    assert_eq!(commits(dir.path()), before + 1);
    let paths: Vec<String> = store
        .needs()
        .unwrap()
        .iter()
        .map(|n| n.record_path().to_string())
        .collect();
    assert_eq!(
        paths,
        [
            "/jumpgate/needs/shelf/api-token",
            "/jumpgate/needs/shelf/fresh"
        ]
    );
}

#[test]
fn an_apply_with_nothing_to_write_or_delete_makes_no_commit() {
    let Some((dir, store)) = temporary_board() else {
        return;
    };
    store.put(&[need("api-token").into()]).unwrap();
    let before = commits(dir.path());
    store
        .apply(&[], &[need("never-written").record_path()])
        .unwrap();
    store.apply(&[], &[]).unwrap();
    assert_eq!(commits(dir.path()), before);
}

#[test]
fn bindings_are_read_for_one_target_and_carry_the_board_id_of_the_store() {
    let Some((_dir, store)) = temporary_board() else {
        return;
    };
    store
        .put(&[binding("local").into(), binding("local-two").into()])
        .unwrap();
    let found = store.bindings(&"local".parse().unwrap()).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].board().as_str(), "temporary");
    assert_eq!(found[0].binding(), &binding("local"));
}

#[test]
fn a_directory_that_is_no_board_is_a_board_refusal_with_the_code_board_printed() {
    if !board_installed() {
        skipped();
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let store = BoardStore::new(
        Board::new(Scope::Directory(dir.path().to_path_buf())),
        "temporary".parse().unwrap(),
    );
    let error = store.needs().unwrap_err();
    assert_eq!(error.reason(), Reason::Board);
    assert!(error.message().contains("board::"), "{}", error.message());
}

#[test]
fn a_board_program_that_is_missing_is_a_board_error_that_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("no-board");
    let store = BoardStore::new(
        Board::with_program(missing, Scope::Personal),
        "personal".parse().unwrap(),
    );
    let error = store.needs().unwrap_err();
    assert_eq!(error.reason(), Reason::Board);
    assert!(
        error.message().contains("not installed"),
        "{}",
        error.message()
    );
}

/// Reports that a test is skipped because `board` is not installed.
#[expect(
    clippy::disallowed_macros,
    reason = "the skip message is the test's own report"
)]
fn skipped() {
    eprintln!("skipped: no board program on PATH, so there is no board to test against");
}
