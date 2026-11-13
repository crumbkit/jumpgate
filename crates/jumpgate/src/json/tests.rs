use serde_json::json;

use super::*;
use crate::name::AppName;

#[test]
fn an_object_refuses_a_field_it_does_not_know_and_names_the_fields_it_has() {
    let value = json!({"app": "shelf", "colour": "blue"});
    let error = Object::open(&value, &["app", "name"]).err().unwrap();
    assert_eq!(error.reason(), Reason::UnknownField);
    assert!(error.to_string().contains("app, name"), "{error}");
}

#[test]
fn an_unknown_field_is_never_quoted_even_when_it_looks_like_a_field_name() {
    for key in ["colour", "ghp_s3cr3tT0ken", "sk live s3cr3t"] {
        let value = json!({"app": "shelf", key: true});
        let error = Object::open(&value, &["app"]).err().unwrap();
        assert_eq!(error.reason(), Reason::UnknownField);
        assert!(!error.to_string().contains(key), "{error}");
    }
}

#[test]
fn one_key_never_quotes_an_unknown_key_and_names_the_keys_it_takes() {
    let value = json!({"ghp_s3cr3tT0ken": "TOKEN"});
    let error = one_key(&value, &["variables", "file"], "not one key").unwrap_err();
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
    assert!(error.to_string().contains("variables, file"), "{error}");
}

#[test]
fn an_object_with_only_known_fields_opens() {
    let value = json!({"app": "shelf"});
    let object = Object::open(&value, &["app", "name"]).unwrap();
    let app: AppName = object.required("app").unwrap();
    assert_eq!(app.as_str(), "shelf");
}

#[test]
fn a_missing_required_field_is_refused_at_its_key() {
    let value = json!({});
    let object = Object::open(&value, &["app"]).unwrap();
    let error = object.required::<AppName>("app").unwrap_err();
    assert_eq!(error.reason(), Reason::MissingField);
    assert_eq!(error.field(), Some("app"));
}

#[test]
fn a_missing_optional_field_reads_as_none_and_a_null_is_refused() {
    let empty = json!({});
    let object = Object::open(&empty, &["app"]).unwrap();
    assert_eq!(object.optional::<AppName>("app").unwrap(), None);
    let null = json!({"app": null});
    let object = Object::open(&null, &["app"]).unwrap();
    assert_eq!(
        object.optional::<AppName>("app").unwrap_err().reason(),
        Reason::Shape
    );
}

#[test]
fn a_value_that_is_not_an_object_is_refused_with_the_shape_code() {
    let value = json!(["shelf"]);
    assert_eq!(
        Object::open(&value, &["app"]).err().unwrap().reason(),
        Reason::Shape
    );
}

#[test]
fn a_number_where_a_name_belongs_is_refused_without_quoting_it() {
    let value = json!({"app": 123_456_789});
    let object = Object::open(&value, &["app"]).unwrap();
    let error = object.required::<AppName>("app").unwrap_err();
    assert_eq!(error.reason(), Reason::Shape);
    assert!(!error.to_string().contains("123456789"), "{error}");
}

#[test]
fn one_key_returns_the_one_known_key_and_its_value() {
    let value = json!({"file": "token"});
    let (key, inner) = one_key(&value, &["variables", "file"], "not one key").unwrap();
    assert_eq!(key, "file");
    assert_eq!(inner, &json!("token"));
}

#[test]
fn one_key_refuses_an_object_with_two_keys_or_none() {
    let two = json!({"file": "token", "variables": []});
    assert_eq!(
        one_key(&two, &["variables", "file"], "not one key")
            .unwrap_err()
            .reason(),
        Reason::Shape
    );
    let none = json!({});
    assert_eq!(
        one_key(&none, &["variables", "file"], "not one key")
            .unwrap_err()
            .reason(),
        Reason::Shape
    );
}

#[test]
fn one_key_refuses_an_unknown_key() {
    let value = json!({"env": "TOKEN"});
    assert_eq!(
        one_key(&value, &["variables", "file"], "not one key")
            .unwrap_err()
            .reason(),
        Reason::UnknownField
    );
}

#[test]
fn one_key_refuses_a_string_without_quoting_it() {
    let value = json!("s3cr3t");
    let error = one_key(&value, &["secret"], "not one key").unwrap_err();
    assert_eq!(error.reason(), Reason::Shape);
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
}
