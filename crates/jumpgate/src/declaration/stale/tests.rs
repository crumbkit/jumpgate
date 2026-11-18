#![expect(
    clippy::disallowed_methods,
    reason = "the tests keep their records in a temporary directory store"
)]

use std::collections::BTreeMap;

use tempfile::TempDir;

use super::*;
use crate::declaration::{Checkout, CheckoutError, Repository, read};
use crate::file_path::RepoFilePath;
use crate::repo::RepoId;
use crate::store::{AnyRecord, DirectoryStore};

struct Files(BTreeMap<&'static str, &'static str>);

impl Checkout for Files {
    fn read(&self, path: &RepoFilePath) -> Result<Option<String>, CheckoutError> {
        Ok(self.0.get(path.as_str()).map(|text| (*text).to_owned()))
    }
}

const SHELF: &str = "ports = [\"http\"]\n\n[processes.web]\nargv = [\"./web\"]\nlisten = \"http\"\n\n[wants.auth-api]\nrepo = \"github.com/acme/auth\"\nprovides = \"api\"\nas = \"AUTH_URL\"\n";
const SHELF_NO_WANT: &str =
    "ports = [\"http\"]\n\n[processes.web]\nargv = [\"./web\"]\nlisten = \"http\"\n";
const SHELF_RENAMED: &str =
    "ports = [\"api\"]\n\n[processes.api]\nargv = [\"./web\"]\nlisten = \"api\"\n";
const SHELF_PORT_AND_UNIT: &str =
    "ports = [\"api\"]\n\n[processes.api]\nargv = [\"./web\"]\nlisten = \"api\"\n";
const BROKEN: &str = "ports = [\"HTTP\"]\n";
const AUTH: &str =
    "ports = [\"api\"]\n\n[processes.server]\nargv = [\"./auth\"]\nlisten = \"api\"\n";

fn declarations(repos: &[(&str, &'static str)]) -> Declarations {
    let checkouts: Vec<(RepoId, Files)> = repos
        .iter()
        .map(|(id, mxv)| {
            (
                id.parse().unwrap(),
                Files(BTreeMap::from([("mxv.toml", *mxv)])),
            )
        })
        .collect();
    let repositories: Vec<Repository<'_>> = checkouts
        .iter()
        .map(|(id, checkout)| Repository::new(id, checkout))
        .collect();
    read(&repositories)
}

fn write(store: &DirectoryStore, declarations: &Declarations) {
    let mut batch: Vec<AnyRecord> = Vec::new();
    batch.extend(declarations.needs().iter().cloned().map(AnyRecord::from));
    batch.extend(declarations.offers().iter().cloned().map(AnyRecord::from));
    batch.extend(declarations.units().iter().cloned().map(AnyRecord::from));
    store.put(&batch).unwrap();
}

fn stale_paths(declarations: &Declarations, store: &DirectoryStore) -> Vec<String> {
    stale(declarations, store)
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect()
}

const SHELF_ID: &str = "github.com/acme/shelf";
const AUTH_ID: &str = "github.com/acme/auth";

#[test]
fn a_run_that_produces_every_record_of_the_store_finds_nothing_stale() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    let run = declarations(&[(SHELF_ID, SHELF), (AUTH_ID, AUTH)]);
    write(&store, &run);
    assert_eq!(stale_paths(&run, &store), Vec::<String>::new());
}

#[test]
fn the_need_of_a_removed_want_is_stale() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    write(&store, &declarations(&[(SHELF_ID, SHELF), (AUTH_ID, AUTH)]));
    let later = declarations(&[(SHELF_ID, SHELF_NO_WANT), (AUTH_ID, AUTH)]);
    assert_eq!(
        stale_paths(&later, &store),
        ["/jumpgate/needs/shelf/auth-api"]
    );
}

#[test]
fn the_offers_and_unit_of_a_renamed_port_and_process_are_stale() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    write(&store, &declarations(&[(SHELF_ID, SHELF)]));
    let later = declarations(&[(SHELF_ID, SHELF_RENAMED)]);
    let stale = stale_paths(&later, &store);
    for expected in [
        "/jumpgate/needs/shelf/auth-api",
        "/jumpgate/offers/app/shelf/http",
        "/jumpgate/offers/app/shelf/web",
        "/jumpgate/units/shelf/web",
    ] {
        assert!(stale.iter().any(|path| path == expected), "{stale:?}");
    }
    assert_eq!(stale.len(), 4, "{stale:?}");
}

#[test]
fn an_offer_renamed_by_the_bare_name_rule_leaves_the_old_offer_stale() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    let before = declarations(&[(AUTH_ID, AUTH)]);
    write(&store, &before);
    assert!(
        before
            .offers()
            .iter()
            .any(|offer| offer.name().as_str() == "api")
    );
    // The process and the port share the name api in this declaration, so
    // both offers take the kind as a suffix.
    let later = declarations(&[(AUTH_ID, SHELF_PORT_AND_UNIT)]);
    let names: Vec<_> = later
        .offers()
        .iter()
        .map(|offer| offer.name().as_str().to_owned())
        .collect();
    assert!(names.contains(&"api-port".to_owned()), "{names:?}");
    let stale = stale_paths(&later, &store);
    assert!(
        stale.contains(&"/jumpgate/offers/app/auth/api".to_owned()),
        "{stale:?}"
    );
}

#[test]
fn an_app_that_the_run_did_not_read_keeps_every_record() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    write(&store, &declarations(&[(SHELF_ID, SHELF), (AUTH_ID, AUTH)]));
    let later = declarations(&[(AUTH_ID, AUTH)]);
    assert_eq!(stale_paths(&later, &store), Vec::<String>::new());
}

#[test]
fn an_app_whose_declaration_was_refused_keeps_every_record() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    write(&store, &declarations(&[(SHELF_ID, SHELF), (AUTH_ID, AUTH)]));
    let later = declarations(&[(SHELF_ID, BROKEN), (AUTH_ID, AUTH)]);
    assert!(!later.refusals().is_empty());
    assert_eq!(stale_paths(&later, &store), Vec::<String>::new());
}

#[test]
fn the_secret_store_offers_of_an_app_are_never_stale() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    let mut records = declarations(&[(AUTH_ID, AUTH)]).offers().to_vec();
    let secret = crate::Offer::from_json(
        r#"{
            "$type": "jumpgate/offer/v1",
            "provider": {"kind": "secret-store", "name": "one-password"},
            "kind": "secret",
            "name": "shelf-dev-api-token",
            "reference": {"secret": "op://shelf-dev/api-token/credential"},
            "grants": [],
            "scope": "app-instance"
        }"#,
    )
    .unwrap();
    records.push(secret.clone());
    let batch: Vec<AnyRecord> = records.into_iter().map(AnyRecord::from).collect();
    store.put(&batch).unwrap();
    let run = declarations(&[(AUTH_ID, AUTH)]);
    assert_eq!(stale_paths(&run, &store), Vec::<String>::new());
}

#[test]
fn a_store_record_that_cannot_be_read_fails_the_search() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    let run = declarations(&[(AUTH_ID, AUTH)]);
    write(&store, &run);
    std::fs::write(dir.path().join("jumpgate/units/auth/server.json"), "{").unwrap();
    assert!(stale(&run, &store).is_err());
}

const SHELF_SERVICE: &str = "[service.store]\nkind = \"dynamodb\"\n\n[processes.web]\nargv = [\"./web\"]\nneeds = [\"store\"]\n";
const SHELF_SERVICE_UNLISTED: &str =
    "[service.store]\nkind = \"dynamodb\"\n\n[processes.web]\nargv = [\"./web\"]\n";
const AUTH_SERVICE: &str = "[service.store]\nkind = \"dynamodb\"\n\n[processes.server]\nargv = [\"./auth\"]\nneeds = [\"store\"]\n";
const AUTH_NO_SERVICE: &str = "[processes.server]\nargv = [\"./auth\"]\n";

#[test]
fn a_service_that_its_only_app_stops_listing_leaves_its_need_and_workspace_offer_stale() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    write(&store, &declarations(&[(SHELF_ID, SHELF_SERVICE)]));
    let later = declarations(&[(SHELF_ID, SHELF_SERVICE_UNLISTED)]);
    assert_eq!(
        stale_paths(&later, &store),
        [
            "/jumpgate/needs/shelf/store",
            "/jumpgate/offers/workspace/mxv/store"
        ]
    );
}

#[test]
fn a_workspace_offer_that_another_app_of_the_store_still_needs_is_not_stale() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    write(
        &store,
        &declarations(&[(SHELF_ID, SHELF_SERVICE), (AUTH_ID, AUTH_SERVICE)]),
    );
    let later = declarations(&[(SHELF_ID, SHELF_SERVICE_UNLISTED)]);
    assert_eq!(stale_paths(&later, &store), ["/jumpgate/needs/shelf/store"]);
}

#[test]
fn a_workspace_offer_that_a_run_of_two_apps_still_produces_is_not_stale() {
    let dir = TempDir::new().unwrap();
    let store = DirectoryStore::open(dir.path()).unwrap();
    write(
        &store,
        &declarations(&[(SHELF_ID, SHELF_SERVICE), (AUTH_ID, AUTH_SERVICE)]),
    );
    let later = declarations(&[(SHELF_ID, SHELF_SERVICE_UNLISTED), (AUTH_ID, AUTH_SERVICE)]);
    assert_eq!(stale_paths(&later, &store), ["/jumpgate/needs/shelf/store"]);
    let gone = declarations(&[
        (SHELF_ID, SHELF_SERVICE_UNLISTED),
        (AUTH_ID, AUTH_NO_SERVICE),
    ]);
    assert_eq!(
        stale_paths(&gone, &store),
        [
            "/jumpgate/needs/auth/store",
            "/jumpgate/needs/shelf/store",
            "/jumpgate/offers/workspace/mxv/store"
        ]
    );
}
