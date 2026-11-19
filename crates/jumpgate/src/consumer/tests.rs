use serde_json::json;

use super::*;
use crate::error::Reason;
use crate::json::FromJson;

#[test]
fn a_string_is_a_unit_and_an_object_with_command_is_a_command() {
    let unit = Consumer::from_json(&json!("api")).unwrap();
    assert_eq!(unit, Consumer::Unit("api".parse().unwrap()));
    let command = Consumer::from_json(&json!({"command": "test"})).unwrap();
    assert_eq!(command, Consumer::command("test".parse().unwrap()));
}

#[test]
fn a_consumer_is_written_back_as_it_was_read() {
    for value in [json!("api"), json!({"command": "test"})] {
        let consumer = Consumer::from_json(&value).unwrap();
        assert_eq!(serde_json::to_value(&consumer).unwrap(), value);
    }
}

#[test]
fn a_consumer_that_is_neither_a_name_nor_a_command_object_is_refused() {
    for value in [
        json!(3),
        json!(["api"]),
        json!({}),
        json!({"unit": "api"}),
        json!({"command": "test", "unit": "api"}),
    ] {
        assert!(Consumer::from_json(&value).is_err(), "{value}");
    }
}

#[test]
fn a_command_name_that_breaks_the_name_rule_is_refused_at_command() {
    let error = Consumer::from_json(&json!({"command": "a b"})).unwrap_err();
    assert_eq!(error.reason(), Reason::Name);
    assert_eq!(error.field(), Some("command"));
}

#[test]
fn a_consumer_says_whether_it_is_a_unit_and_names_itself() {
    let unit = Consumer::Unit("api".parse().unwrap());
    let command = Consumer::command("api".parse().unwrap());
    assert_eq!(unit.unit().map(UnitName::as_str), Some("api"));
    assert_eq!(command.unit(), None);
    assert_ne!(unit, command);
    assert_eq!(unit.to_string(), "api");
    assert_eq!(command.to_string(), "command api");
}
