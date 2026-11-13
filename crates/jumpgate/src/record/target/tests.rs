use serde_json::{Value, json};

use super::*;
use crate::error::Reason;

fn target() -> Value {
    json!({
        "$type": "jumpgate/target/v1",
        "name": "local",
        "kind": "local",
        "authority": "personal"
    })
}

fn with(mut value: Value, key: &str, field: Value) -> Value {
    value.as_object_mut().unwrap().insert(key.to_owned(), field);
    value
}

fn refusal(value: &Value) -> (Reason, Option<String>) {
    let error = Target::from_json(&value.to_string()).expect_err("the target is refused");
    (error.reason(), error.field().map(str::to_owned))
}

fn at(field: &str) -> Option<String> {
    Some(field.to_owned())
}

#[test]
fn a_target_is_read_and_reads_back_the_json_it_writes() {
    let target = Target::from_json(&target().to_string()).unwrap();
    assert_eq!(target.name().as_str(), "local");
    assert_eq!(target.kind(), TargetKind::Local);
    assert_eq!(target.authority().as_str(), "personal");
    let text = serde_json::to_string(&target).unwrap();
    assert!(
        text.starts_with("{\"$type\":\"jumpgate/target/v1\","),
        "{text}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap(),
        self::target()
    );
}

#[test]
fn a_target_built_in_code_equals_the_one_read_from_json() {
    let built = Target::new(
        "local".parse().unwrap(),
        TargetKind::Local,
        "personal".parse().unwrap(),
    );
    assert_eq!(built, Target::from_json(&target().to_string()).unwrap());
}

#[test]
fn a_cloud_target_is_read() {
    let cloud = with(
        with(target(), "kind", json!("cloud")),
        "name",
        json!("staging"),
    );
    let target = Target::from_json(&cloud.to_string()).unwrap();
    assert_eq!(target.kind(), TargetKind::Cloud);
}

#[test]
fn a_target_has_its_path_under_its_name() {
    let target = Target::from_json(&target().to_string()).unwrap();
    assert_eq!(target.path().to_string(), "/jumpgate/targets/local");
    assert_eq!(target.record_path().to_string(), "/jumpgate/targets/local");
}

#[test]
fn a_target_of_an_unknown_kind_is_refused() {
    assert_eq!(
        refusal(&with(target(), "kind", json!("staging"))),
        (Reason::Keyword, at("kind"))
    );
}

#[test]
fn a_target_whose_authority_breaks_the_name_rule_is_refused() {
    assert_eq!(
        refusal(&with(target(), "authority", json!("/repos/acme"))),
        (Reason::Name, at("authority"))
    );
}

#[test]
fn a_target_without_an_authority_is_refused() {
    let mut value = target();
    value.as_object_mut().unwrap().remove("authority");
    assert_eq!(refusal(&value), (Reason::MissingField, at("authority")));
}
