use serde_json::{Value, json};

use super::*;
use crate::error::Reason;

fn binding() -> Value {
    json!({
        "$type": "jumpgate/binding/v1",
        "need": "/jumpgate/needs/shelf/api_token",
        "offer": "/jumpgate/offers/secret-store/one-password/shelf-api-token",
        "target": "local",
        "projection": "public-key",
        "audience": "ledger"
    })
}

fn with(mut value: Value, key: &str, field: Value) -> Value {
    value.as_object_mut().unwrap().insert(key.to_owned(), field);
    value
}

fn without(mut value: Value, key: &str) -> Value {
    value.as_object_mut().unwrap().remove(key);
    value
}

fn read(value: &Value) -> Result<Binding, RecordError> {
    Binding::from_json(&value.to_string())
}

fn refusal(value: &Value) -> (Reason, Option<String>) {
    let error = read(value).expect_err("the binding is refused");
    (error.reason(), error.field().map(str::to_owned))
}

fn at(field: &str) -> Option<String> {
    Some(field.to_owned())
}

#[test]
fn a_binding_is_read_with_its_values() {
    let binding = read(&binding()).unwrap();
    assert_eq!(
        binding.need().to_string(),
        "/jumpgate/needs/shelf/api_token"
    );
    assert_eq!(
        binding.offer().to_string(),
        "/jumpgate/offers/secret-store/one-password/shelf-api-token"
    );
    assert_eq!(binding.target().as_str(), "local");
    assert_eq!(
        binding.projection().map(Projection::as_str),
        Some("public-key")
    );
    assert_eq!(binding.audience().map(Audience::as_str), Some("ledger"));
}

#[test]
fn a_binding_reads_back_the_json_it_writes_with_its_type_first() {
    for value in [
        binding(),
        without(without(binding(), "projection"), "audience"),
    ] {
        let binding = read(&value).unwrap();
        let text = serde_json::to_string(&binding).unwrap();
        assert!(
            text.starts_with("{\"$type\":\"jumpgate/binding/v1\","),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value);
        assert_eq!(Binding::from_json(&text).unwrap(), binding);
    }
}

#[test]
fn a_binding_built_in_code_equals_the_one_read_from_json() {
    let built = Binding::new(
        "/jumpgate/needs/shelf/api_token".parse().unwrap(),
        "/jumpgate/offers/secret-store/one-password/shelf-api-token"
            .parse()
            .unwrap(),
        "local".parse().unwrap(),
    )
    .with_projection("public-key".parse().unwrap())
    .with_audience("ledger".parse().unwrap());
    assert_eq!(built, read(&binding()).unwrap());
}

#[test]
fn a_binding_has_its_path_under_its_target_and_its_needs_app() {
    let binding = read(&binding()).unwrap();
    assert_eq!(
        binding.path().to_string(),
        "/jumpgate/bindings/local/shelf/api_token"
    );
    assert_eq!(
        binding.record_path().to_string(),
        "/jumpgate/bindings/local/shelf/api_token"
    );
}

#[test]
fn a_binding_whose_need_is_not_a_need_path_is_refused_with_the_record_path_code() {
    assert_eq!(
        refusal(&with(
            binding(),
            "need",
            json!("/jumpgate/needs/shelf/../token")
        )),
        (Reason::RecordPath, at("need"))
    );
}

#[test]
fn a_binding_whose_offer_is_not_an_offer_path_is_refused() {
    assert_eq!(
        refusal(&with(
            binding(),
            "offer",
            json!("op://shelf-dev/api-token/credential")
        )),
        (Reason::RecordPath, at("offer"))
    );
}

#[test]
fn a_binding_with_a_value_field_is_refused_because_no_field_holds_a_value() {
    assert_eq!(
        refusal(&with(binding(), "value", json!("s3cr3t"))),
        (Reason::UnknownField, None)
    );
}

#[test]
fn a_binding_whose_target_breaks_the_name_rule_is_refused() {
    assert_eq!(
        refusal(&with(binding(), "target", json!("Local"))),
        (Reason::Name, at("target"))
    );
}

#[test]
fn a_binding_with_a_null_projection_is_refused_and_one_without_is_read() {
    assert_eq!(
        refusal(&with(binding(), "projection", Value::Null)),
        (Reason::Shape, at("projection"))
    );
    assert_eq!(
        read(&without(binding(), "projection"))
            .unwrap()
            .projection(),
        None
    );
}

#[test]
fn a_binding_whose_audience_breaks_the_name_rule_is_refused() {
    assert_eq!(
        refusal(&with(
            binding(),
            "audience",
            json!("https://ledger.example")
        )),
        (Reason::Name, at("audience"))
    );
}

#[test]
fn a_binding_that_says_how_its_value_is_delivered_is_refused_because_the_need_says_it() {
    assert_eq!(
        refusal(&with(
            binding(),
            "delivers_as",
            json!({"variables": ["API_TOKEN"]})
        )),
        (Reason::UnknownField, None)
    );
}

#[test]
fn a_binding_with_only_need_offer_target_projection_and_audience_is_read() {
    let binding = read(&binding()).unwrap();
    let written = serde_json::to_value(&binding).unwrap();
    let mut keys: Vec<&str> = written
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["$type", "audience", "need", "offer", "projection", "target"]
    );
}
