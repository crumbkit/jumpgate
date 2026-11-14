#![expect(
    clippy::disallowed_methods,
    reason = "the tests build and damage the store's files directly"
)]

use std::fs;

use serde_json::json;
use tempfile::TempDir;

use super::*;
use crate::declaration::{Checkout, CheckoutError, Repository, read};
use crate::file_path::RepoFilePath;
use crate::repo::RepoId;

/// A checkout of one `mxv.toml`.
struct OneFile(&'static str);

impl Checkout for OneFile {
    fn read(&self, path: &RepoFilePath) -> Result<Option<String>, CheckoutError> {
        Ok((path.as_str() == "mxv.toml").then(|| self.0.to_owned()))
    }
}

const MXV: &str = "ports = [\"http\"]\n\n[processes.web]\nargv = [\"./web\"]\nlisten = \"http\"\n\n[wants.auth-api]\nrepo = \"github.com/acme/auth\"\nprovides = \"api\"\nas = \"AUTH_URL\"\n";

/// The records the reader makes of a small app.
fn records() -> Vec<AnyRecord> {
    let id: RepoId = "github.com/acme/shelf".parse().unwrap();
    let checkout = OneFile(MXV);
    let declarations = read(&[Repository::new(&id, &checkout)]);
    assert_eq!(declarations.refusals(), &[]);
    let mut records: Vec<AnyRecord> = Vec::new();
    records.extend(declarations.needs().iter().cloned().map(AnyRecord::from));
    records.extend(declarations.offers().iter().cloned().map(AnyRecord::from));
    records.extend(declarations.units().iter().cloned().map(AnyRecord::from));
    records
}

fn binding(target: &str) -> Binding {
    Binding::from_json(
        &json!({
            "$type": "jumpgate/binding/v1",
            "need": "/jumpgate/needs/shelf/auth-api",
            "offer": "/jumpgate/offers/app/auth/api",
            "target": target,
        })
        .to_string(),
    )
    .unwrap()
}

fn target(name: &str, authority: &str) -> Target {
    Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": name,
            "kind": "local",
            "authority": authority,
        })
        .to_string(),
    )
    .unwrap()
}

fn open(dir: &TempDir) -> DirectoryStore {
    DirectoryStore::open(dir.path()).unwrap()
}

fn name(text: &str) -> TargetName {
    text.parse().unwrap()
}

#[test]
fn a_record_is_written_to_the_file_of_its_path_and_read_back() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    assert!(
        dir.path()
            .join("jumpgate/needs/shelf/auth-api.json")
            .is_file()
    );
    assert!(
        dir.path()
            .join("jumpgate/offers/app/shelf/http.json")
            .is_file()
    );
    assert!(dir.path().join("jumpgate/units/shelf/web.json").is_file());
    for record in records() {
        assert_eq!(store.get(&record.path()).unwrap(), Some(record));
    }
}

#[test]
fn a_path_with_no_record_reads_as_none() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    let path = "/jumpgate/needs/shelf/auth-api".parse().unwrap();
    assert_eq!(store.get(&RecordPath::Need(path)).unwrap(), None);
    store.put(&records()).unwrap();
    let other = "/jumpgate/needs/shelf/missing".parse().unwrap();
    assert_eq!(store.get(&RecordPath::Need(other)).unwrap(), None);
}

#[test]
fn each_listing_returns_the_records_of_its_type_ordered_by_path() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    let paths = |records: Vec<String>| records;
    assert_eq!(
        paths(
            store
                .needs()
                .unwrap()
                .iter()
                .map(|need| need.path().to_string())
                .collect()
        ),
        ["/jumpgate/needs/shelf/auth-api"]
    );
    assert_eq!(
        paths(
            store
                .offers()
                .unwrap()
                .iter()
                .map(|offer| offer.path().to_string())
                .collect()
        ),
        [
            "/jumpgate/offers/app/shelf/http",
            "/jumpgate/offers/app/shelf/web"
        ]
    );
    assert_eq!(
        paths(
            store
                .units()
                .unwrap()
                .iter()
                .map(|unit| unit.path().to_string())
                .collect()
        ),
        ["/jumpgate/units/shelf/web"]
    );
    assert_eq!(store.targets().unwrap(), []);
}

#[test]
fn an_empty_store_lists_nothing() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    assert_eq!(store.needs().unwrap(), []);
    assert_eq!(store.offers().unwrap(), []);
    assert_eq!(store.units().unwrap(), []);
    assert_eq!(store.bindings(&name("local")).unwrap(), []);
}

#[test]
fn writing_a_record_again_replaces_it_and_leaves_no_temporary_file() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&[target("local", "personal").into()]).unwrap();
    store.put(&[target("local", "team").into()]).unwrap();
    let targets = store.targets().unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].authority().as_str(), "team");
    let leftovers: Vec<_> = fs::read_dir(dir.path().join("jumpgate/targets"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(leftovers, ["local.json"]);
}

#[test]
fn the_store_never_deletes_a_record_that_a_later_batch_leaves_out() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    store.put(&[target("local", "personal").into()]).unwrap();
    assert_eq!(store.needs().unwrap().len(), 1);
}

#[test]
fn a_temporary_file_of_an_interrupted_write_is_not_a_record() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    fs::write(dir.path().join("jumpgate/needs/shelf/half.json.tmp"), "{").unwrap();
    assert_eq!(store.needs().unwrap().len(), 1);
}

#[test]
fn the_board_is_local_until_the_first_write_creates_the_board_file() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    assert_eq!(store.board().as_str(), "local");
    assert!(!dir.path().join(BOARD_FILE).exists());
    store.put(&records()).unwrap();
    let text = fs::read_to_string(dir.path().join(BOARD_FILE)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value, json!({"board": "local"}));
}

#[test]
fn the_board_is_read_from_the_board_file_and_a_write_keeps_it() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(BOARD_FILE), r#"{"board": "personal"}"#).unwrap();
    let store = open(&dir);
    assert_eq!(store.board().as_str(), "personal");
    store.put(&records()).unwrap();
    assert_eq!(open(&dir).board().as_str(), "personal");
}

#[test]
fn a_board_file_that_is_not_a_board_id_is_refused_and_a_valid_one_is_read() {
    for text in [
        "{",
        "[]",
        r#"{"board": 3}"#,
        r#"{"board": "Personal"}"#,
        r#"{"board": "personal", "extra": 1}"#,
        "{}",
    ] {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(BOARD_FILE), text).unwrap();
        let error = DirectoryStore::open(dir.path()).unwrap_err();
        assert_eq!(error.reason(), Reason::BoardFile, "{text}");
        assert_eq!(error.code().to_string(), "jumpgate::store::board_file");
    }
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(BOARD_FILE), r#"{"board": "personal"}"#).unwrap();
    assert!(DirectoryStore::open(dir.path()).is_ok());
}

#[test]
fn a_binding_of_the_stores_own_board_comes_back_with_that_board() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(BOARD_FILE), r#"{"board": "personal"}"#).unwrap();
    let store = open(&dir);
    store.put(&[binding("local").into()]).unwrap();
    let found = store.bindings(&name("local")).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].board().as_str(), "personal");
    assert_eq!(found[0].binding(), &binding("local"));
}

#[test]
fn bindings_are_listed_for_the_target_asked_and_no_other() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store
        .put(&[binding("local").into(), binding("cloud").into()])
        .unwrap();
    assert_eq!(store.bindings(&name("local")).unwrap().len(), 1);
    assert_eq!(store.bindings(&name("cloud")).unwrap().len(), 1);
    assert_eq!(store.bindings(&name("staging")).unwrap(), []);
}

#[test]
fn a_binding_in_another_boards_directory_comes_back_with_that_board() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&[binding("local").into()]).unwrap();
    let other = DirectoryStore::open(dir.path().join("boards/team")).unwrap();
    other.put(&[binding("local").into()]).unwrap();
    let found = store.bindings(&name("local")).unwrap();
    let boards: Vec<&str> = found.iter().map(|found| found.board().as_str()).collect();
    assert_eq!(boards, ["local", "team"]);
}

#[test]
fn a_directory_under_boards_that_is_not_a_board_id_is_refused() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    fs::create_dir_all(dir.path().join("boards/Team")).unwrap();
    let error = store.bindings(&name("local")).unwrap_err();
    assert_eq!(error.reason(), Reason::BoardFile);
    fs::remove_dir(dir.path().join("boards/Team")).unwrap();
    fs::create_dir_all(dir.path().join("boards/team")).unwrap();
    assert_eq!(store.bindings(&name("local")).unwrap(), []);
}

#[test]
fn a_file_that_is_not_a_record_is_refused_without_quoting_it() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    fs::write(
        dir.path().join("jumpgate/needs/shelf/auth-api.json"),
        "s3cr3t-text",
    )
    .unwrap();
    let error = store.needs().unwrap_err();
    assert_eq!(error.reason(), Reason::Corrupt);
    assert_eq!(error.code().to_string(), "jumpgate::store::corrupt");
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
    let path = RecordPath::Need("/jumpgate/needs/shelf/auth-api".parse().unwrap());
    assert_eq!(store.get(&path).unwrap_err().reason(), Reason::Corrupt);
}

#[test]
fn a_record_in_a_file_of_another_path_is_refused() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    let from = dir.path().join("jumpgate/needs/shelf/auth-api.json");
    let to = dir.path().join("jumpgate/needs/shelf/elsewhere.json");
    fs::rename(from, to).unwrap();
    assert_eq!(store.needs().unwrap_err().reason(), Reason::Misplaced);
    let from = dir.path().join("jumpgate/needs/shelf/elsewhere.json");
    let back = dir.path().join("jumpgate/needs/shelf/auth-api.json");
    fs::rename(from, back).unwrap();
    assert_eq!(store.needs().unwrap().len(), 1);
}

#[test]
fn a_directory_that_cannot_be_written_is_an_io_failure() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("jumpgate"),
        "a file where a directory belongs",
    )
    .unwrap();
    let store = open(&dir);
    let error = store.put(&records()).unwrap_err();
    assert_eq!(error.reason(), Reason::Io);
    assert_eq!(error.code().to_string(), "jumpgate::store::io");
}

#[test]
fn every_reason_has_a_store_code() {
    for reason in Reason::ALL {
        assert!(reason.code().to_string().starts_with("jumpgate::store::"));
    }
}
