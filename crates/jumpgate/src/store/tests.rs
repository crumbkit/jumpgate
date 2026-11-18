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
use crate::plan::{MAX_PLAN_BYTES, Plan, PlanTarget};
use crate::repo::RepoId;

#[test]
fn a_plan_file_is_written_whole_with_a_final_newline_and_replaces_an_older_one() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("out").join("plan.json");
    let target: Target = Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": "local",
            "kind": "local",
            "authority": "local"
        })
        .to_string(),
    )
    .unwrap();
    let plan = Plan::new(PlanTarget::of(&target), vec![], vec![]);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "old").unwrap();
    write_plan_file(&file, &plan).unwrap();
    let text = fs::read_to_string(&file).unwrap();
    assert_eq!(text, format!("{}\n", plan.to_json()));
}

#[test]
fn a_plan_file_in_a_directory_that_is_a_file_is_an_io_error() {
    let dir = TempDir::new().unwrap();
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    let target: Target = Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": "local",
            "kind": "local",
            "authority": "local"
        })
        .to_string(),
    )
    .unwrap();
    let plan = Plan::new(PlanTarget::of(&target), vec![], vec![]);
    let error = write_plan_file(&blocker.join("plan.json"), &plan).unwrap_err();
    assert_eq!(error.reason(), Reason::Io);
}

#[test]
fn a_plan_file_is_read_as_the_text_that_was_written_and_a_missing_one_is_an_io_error() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("plan.json");
    fs::write(&file, "{\"$type\": \"x\"}\n").unwrap();
    assert_eq!(read_plan_text(&file).unwrap(), "{\"$type\": \"x\"}\n");
    let missing = dir.path().join("missing.json");
    let error = read_plan_text(&missing).unwrap_err();
    assert_eq!(error.reason(), Reason::Io);
    assert!(error.message().contains("missing.json"), "{error}");
}

#[test]
fn a_plan_file_that_is_not_text_is_a_corrupt_file_and_one_that_is_text_is_read() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("plan.json");
    fs::write(&file, [0xff, 0xfe, 0x00]).unwrap();
    let error = read_plan_text(&file).unwrap_err();
    assert_eq!(error.reason(), Reason::Corrupt);
    assert!(error.message().contains("plan.json"), "{error}");
    fs::write(&file, "text").unwrap();
    assert_eq!(read_plan_text(&file).unwrap(), "text");
}

#[test]
fn a_plan_file_longer_than_the_limit_is_read_one_byte_past_it_and_no_further() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("plan.json");
    fs::write(&file, " ".repeat(MAX_PLAN_BYTES + 100)).unwrap();
    let text = read_plan_text(&file).unwrap();
    assert_eq!(text.len(), MAX_PLAN_BYTES + 1);
    assert!(Plan::from_json(&text).is_err());
}

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
fn a_put_never_deletes_a_record_that_a_later_batch_leaves_out() {
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

#[test]
fn remove_deletes_the_records_at_the_paths_and_leaves_the_others() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    let need = RecordPath::Need("/jumpgate/needs/shelf/auth-api".parse().unwrap());
    store.remove(std::slice::from_ref(&need)).unwrap();
    assert_eq!(store.get(&need).unwrap(), None);
    assert_eq!(store.needs().unwrap(), []);
    assert_eq!(store.offers().unwrap().len(), 2);
    assert_eq!(store.units().unwrap().len(), 1);
}

#[test]
fn remove_of_a_path_with_no_record_succeeds_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    let missing = RecordPath::Need("/jumpgate/needs/shelf/missing".parse().unwrap());
    store.remove(&[missing]).unwrap();
    assert_eq!(store.needs().unwrap().len(), 1);
    store.remove(&[]).unwrap();
}

#[test]
fn remove_never_touches_the_records_of_another_boards_directory() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&[binding("local").into()]).unwrap();
    let other = DirectoryStore::open(dir.path().join("boards/team")).unwrap();
    other.put(&[binding("local").into()]).unwrap();
    let path = RecordPath::Binding("/jumpgate/bindings/local/shelf/auth-api".parse().unwrap());
    store.remove(&[path]).unwrap();
    let found = store.bindings(&name("local")).unwrap();
    let boards: Vec<&str> = found.iter().map(|found| found.board().as_str()).collect();
    assert_eq!(boards, ["team"]);
}

#[test]
fn remove_of_a_path_that_is_a_directory_is_an_io_failure() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    fs::create_dir_all(dir.path().join("jumpgate/needs/shelf/odd.json")).unwrap();
    let path = RecordPath::Need("/jumpgate/needs/shelf/odd".parse().unwrap());
    assert_eq!(store.remove(&[path]).unwrap_err().reason(), Reason::Io);
}

#[test]
fn apply_writes_the_records_and_deletes_the_paths_in_one_call() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.put(&records()).unwrap();
    let old = RecordPath::Unit("/jumpgate/units/shelf/web".parse().unwrap());
    let kept: Vec<AnyRecord> = records()
        .into_iter()
        .filter(|record| record.path() != old)
        .collect();
    store.apply(&kept, std::slice::from_ref(&old)).unwrap();
    assert_eq!(store.get(&old).unwrap(), None);
    assert_eq!(store.needs().unwrap().len(), 1);
}

#[test]
fn apply_does_not_delete_a_path_that_it_also_writes() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    let all = records();
    let paths: Vec<RecordPath> = all.iter().map(AnyRecord::path).collect();
    store.apply(&all, &paths).unwrap();
    for record in &all {
        assert_eq!(store.get(&record.path()).unwrap().as_ref(), Some(record));
    }
}

#[test]
fn apply_with_nothing_to_write_or_delete_leaves_the_directory_untouched() {
    let dir = TempDir::new().unwrap();
    let store = open(&dir);
    store.apply(&[], &[]).unwrap();
    assert!(!dir.path().join(BOARD_FILE).exists());
}

#[test]
fn a_store_error_built_outside_the_crate_has_the_reason_and_the_code_it_was_given() {
    let error = StoreError::new(Reason::Board, "the board did not answer");
    assert_eq!(error.reason(), Reason::Board);
    assert_eq!(error.code().to_string(), "jumpgate::store::board");
    assert_eq!(error.message(), "the board did not answer");
}

#[test]
fn any_record_reads_the_text_of_each_type_and_refuses_the_text_of_another() {
    let target = json!({
        "$type": "jumpgate/target/v1",
        "name": "local",
        "kind": "local",
        "authority": "local"
    })
    .to_string();
    let record = AnyRecord::from_json(RecordType::Target, &target).unwrap();
    assert_eq!(record.record_type(), RecordType::Target);
    assert_eq!(record.path().to_string(), "/jumpgate/targets/local");
    let refused = AnyRecord::from_json(RecordType::Need, &target).unwrap_err();
    assert_eq!(refused.code().name(), "wrong_type");
}
