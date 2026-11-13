use serde_json::{Value, json};

use super::*;
use crate::error::Reason;

/// Returns a unit that every check accepts, as a JSON value to change.
fn unit() -> Value {
    json!({
        "$type": "jumpgate/unit/v1",
        "app": "shelf",
        "name": "api",
        "repo": "github.com/acme/shelf",
        "kind": "process",
        "starts_after": ["db", "migrate"],
        "ports": ["http", "admin"]
    })
}

/// Returns `value` with the field `key` set to `field`.
fn with(mut value: Value, key: &str, field: Value) -> Value {
    value.as_object_mut().unwrap().insert(key.to_owned(), field);
    value
}

/// Reads `value` as a unit.
fn read(value: &Value) -> Result<Unit, RecordError> {
    Unit::from_json(&value.to_string())
}

/// Returns the reason and field of the refusal of `value`.
fn refusal(value: &Value) -> (Reason, Option<String>) {
    let error = read(value).expect_err("the unit is refused");
    (error.reason(), error.field().map(str::to_owned))
}

fn at(field: &str) -> Option<String> {
    Some(field.to_owned())
}

#[test]
fn a_unit_with_every_field_set_is_read_with_its_values() {
    let unit = read(&unit()).unwrap();
    assert_eq!(unit.app().as_str(), "shelf");
    assert_eq!(unit.name().as_str(), "api");
    assert_eq!(unit.repo().as_str(), "github.com/acme/shelf");
    assert_eq!(unit.kind(), UnitKind::Process);
    let after: Vec<&str> = unit.starts_after().iter().map(UnitName::as_str).collect();
    assert_eq!(after, ["db", "migrate"]);
    let ports: Vec<&str> = unit.ports().iter().map(PortName::as_str).collect();
    assert_eq!(ports, ["http", "admin"]);
}

#[test]
fn a_unit_reads_back_the_json_it_writes_with_its_type_first() {
    let unit = read(&unit()).unwrap();
    let text = serde_json::to_string(&unit).unwrap();
    assert!(
        text.starts_with("{\"$type\":\"jumpgate/unit/v1\","),
        "{text}"
    );
    assert_eq!(Unit::from_json(&text).unwrap(), unit);
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), self::unit());
}

#[test]
fn a_unit_built_in_code_equals_the_one_read_from_json() {
    let built = Unit::new(
        "shelf".parse().unwrap(),
        "api".parse().unwrap(),
        "github.com/acme/shelf".parse().unwrap(),
        UnitKind::Process,
    )
    .unwrap()
    .with_starts_after(
        Distinct::new(vec!["db".parse().unwrap(), "migrate".parse().unwrap()]).unwrap(),
    )
    .with_ports(Distinct::new(vec!["http".parse().unwrap(), "admin".parse().unwrap()]).unwrap());
    assert_eq!(built, read(&unit()).unwrap());
}

#[test]
fn a_unit_has_its_path_under_its_app() {
    let unit = read(&unit()).unwrap();
    assert_eq!(unit.path().to_string(), "/jumpgate/units/shelf/api");
    assert_eq!(unit.record_path().to_string(), "/jumpgate/units/shelf/api");
}

#[test]
fn a_container_unit_with_no_ports_and_no_order_is_read() {
    let unit = read(&with(
        with(with(unit(), "kind", json!("container")), "ports", json!([])),
        "starts_after",
        json!([]),
    ))
    .unwrap();
    assert_eq!(unit.kind(), UnitKind::Container);
    assert!(unit.ports().is_empty());
    assert!(unit.starts_after().is_empty());
}

#[test]
fn a_unit_without_a_field_is_refused_at_that_field() {
    for key in ["app", "name", "repo", "kind", "starts_after", "ports"] {
        let mut value = unit();
        value.as_object_mut().unwrap().remove(key);
        assert_eq!(refusal(&value), (Reason::MissingField, at(key)), "{key}");
    }
}

#[test]
fn a_unit_of_an_unknown_kind_is_refused_at_kind() {
    assert_eq!(
        refusal(&with(unit(), "kind", json!("task"))),
        (Reason::Keyword, at("kind"))
    );
}

#[test]
fn a_unit_whose_name_has_an_uppercase_letter_is_refused_at_name() {
    assert_eq!(
        refusal(&with(unit(), "name", json!("Api"))),
        (Reason::Name, at("name"))
    );
}

#[test]
fn building_a_unit_whose_name_has_an_uppercase_letter_is_refused() {
    let error = Unit::new(
        "shelf".parse().unwrap(),
        "Api".parse().unwrap(),
        "github.com/acme/shelf".parse().unwrap(),
        UnitKind::Process,
    )
    .unwrap_err();
    assert_eq!(error.reason(), Reason::Name);
}

#[test]
fn a_unit_whose_repository_is_not_host_owner_name_is_refused_at_repo() {
    assert_eq!(
        refusal(&with(unit(), "repo", json!("shelf"))),
        (Reason::Repository, at("repo"))
    );
}

#[test]
fn a_unit_that_starts_after_a_unit_twice_is_refused_at_the_repeat() {
    assert_eq!(
        refusal(&with(unit(), "starts_after", json!(["db", "db"]))),
        (Reason::Duplicate, at("starts_after[1]"))
    );
}

#[test]
fn a_unit_with_a_port_that_breaks_the_port_rule_is_refused_at_its_index() {
    assert_eq!(
        refusal(&with(unit(), "ports", json!(["http", "-admin"]))),
        (Reason::Name, at("ports[1]"))
    );
}

#[test]
fn a_unit_with_a_field_its_type_does_not_have_is_refused() {
    assert_eq!(
        refusal(&with(unit(), "argv", json!(["./api"]))).0,
        Reason::UnknownField
    );
}
