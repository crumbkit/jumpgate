use serde_json::json;

use super::*;
use crate::error::Reason;

/// Returns the text of a need that every check accepts.
fn need_text() -> String {
    json!({
        "$type": "jumpgate/need/v1",
        "app": "shelf",
        "name": "api_token",
        "kind": "secret",
        "consumers": ["api"],
        "declared_in": {"repo": "github.com/acme/shelf", "file": "starmap.toml"},
        "delivers_as": {"variables": ["API_TOKEN"]},
        "optional": false,
        "when": [],
        "when_unbound": "fail"
    })
    .to_string()
}

#[test]
fn a_record_whose_text_is_json_of_its_type_is_read() {
    assert!(Need::from_json(&need_text()).is_ok());
}

#[test]
fn a_record_whose_text_is_not_json_is_refused_with_the_syntax_code() {
    let error = Need::from_json("{\"$type\": ").unwrap_err();
    assert_eq!(error.reason(), Reason::Syntax);
    assert_eq!(error.code().to_string(), "jumpgate::record::syntax");
}

#[test]
fn a_syntax_error_never_quotes_the_text() {
    let error = Need::from_json("s3cr3t-token-value").unwrap_err();
    assert_eq!(error.reason(), Reason::Syntax);
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
}

#[test]
fn a_record_of_exactly_the_size_limit_is_read() {
    let text = need_text();
    let padded = format!("{text}{}", " ".repeat(MAX_RECORD_BYTES - text.len()));
    assert_eq!(padded.len(), MAX_RECORD_BYTES);
    assert!(Need::from_json(&padded).is_ok());
}

#[test]
fn a_record_over_the_size_limit_is_refused_with_the_too_large_code_before_parsing() {
    let text = need_text();
    let padded = format!("{text}{}", " ".repeat(MAX_RECORD_BYTES + 1 - text.len()));
    let error = Need::from_json(&padded).unwrap_err();
    assert_eq!(error.reason(), Reason::TooLarge);
    assert_eq!(error.code().to_string(), "jumpgate::record::too_large");
}

#[test]
fn a_record_that_is_not_a_json_object_is_refused_with_the_shape_code() {
    let error = Need::from_json("[1, 2]").unwrap_err();
    assert_eq!(error.reason(), Reason::Shape);
    assert_eq!(error.code().to_string(), "jumpgate::record::shape");
}

#[test]
fn a_record_whose_type_names_another_record_type_is_refused_with_the_wrong_type_code() {
    let text = need_text().replace("jumpgate/need/v1", "jumpgate/offer/v1");
    let error = Need::from_json(&text).unwrap_err();
    assert_eq!(error.reason(), Reason::WrongType);
    assert_eq!(error.code().to_string(), "jumpgate::record::wrong_type");
    assert_eq!(error.field(), Some("$type"));
}

#[test]
fn a_record_whose_type_names_another_version_is_refused() {
    let text = need_text().replace("jumpgate/need/v1", "jumpgate/need/v2");
    assert_eq!(
        Need::from_json(&text).unwrap_err().reason(),
        Reason::WrongType
    );
}

#[test]
fn a_record_without_a_type_is_refused_with_the_missing_field_code() {
    let mut value: serde_json::Value = serde_json::from_str(&need_text()).unwrap();
    value.as_object_mut().unwrap().remove("$type");
    let error = Need::from_json(&value.to_string()).unwrap_err();
    assert_eq!(error.reason(), Reason::MissingField);
    assert_eq!(error.code().to_string(), "jumpgate::record::missing_field");
    assert_eq!(error.field(), Some("$type"));
}

#[test]
fn a_record_whose_type_is_not_a_string_is_refused_with_the_shape_code() {
    let text = need_text().replace("\"jumpgate/need/v1\"", "1");
    let error = Need::from_json(&text).unwrap_err();
    assert_eq!(error.reason(), Reason::Shape);
    assert_eq!(error.field(), Some("$type"));
}

#[test]
fn a_record_with_a_field_its_type_does_not_have_is_refused_with_the_unknown_field_code() {
    let mut value: serde_json::Value = serde_json::from_str(&need_text()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("value".to_owned(), json!("s3cr3t"));
    let error = Need::from_json(&value.to_string()).unwrap_err();
    assert_eq!(error.reason(), Reason::UnknownField);
    assert_eq!(error.code().to_string(), "jumpgate::record::unknown_field");
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
}

#[test]
fn every_record_type_names_its_type_kind_and_schema_file() {
    let names: Vec<(&str, &str, &str)> = RecordType::ALL
        .iter()
        .map(|record_type| {
            (
                record_type.as_str(),
                record_type.kind(),
                record_type.schema_file(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("jumpgate/need/v1", "need", "jumpgate/need/v1.schema.json"),
            (
                "jumpgate/offer/v1",
                "offer",
                "jumpgate/offer/v1.schema.json"
            ),
            (
                "jumpgate/binding/v1",
                "binding",
                "jumpgate/binding/v1.schema.json"
            ),
            (
                "jumpgate/target/v1",
                "target",
                "jumpgate/target/v1.schema.json"
            ),
            ("jumpgate/unit/v1", "unit", "jumpgate/unit/v1.schema.json"),
        ]
    );
}

#[test]
fn each_record_types_trait_constant_matches_its_type() {
    assert_eq!(Need::TYPE, RecordType::Need);
    assert_eq!(Offer::TYPE, RecordType::Offer);
    assert_eq!(Binding::TYPE, RecordType::Binding);
    assert_eq!(Target::TYPE, RecordType::Target);
    assert_eq!(Unit::TYPE, RecordType::Unit);
}

#[test]
fn every_schema_is_draft_2020_12_and_requires_its_type_constant() {
    for record_type in RecordType::ALL {
        let schema = serde_json::to_value(schema(*record_type)).unwrap();
        assert_eq!(
            schema["$schema"],
            json!("https://json-schema.org/draft/2020-12/schema")
        );
        assert_eq!(
            schema["properties"]["$type"]["const"],
            json!(record_type.as_str())
        );
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("$type"))
        );
        assert_eq!(schema["additionalProperties"], json!(false));
    }
}

#[test]
fn every_reference_field_carries_the_board_ref_format() {
    let need = serde_json::to_value(schema(RecordType::Binding)).unwrap();
    assert_eq!(need["properties"]["need"]["format"], json!("board-ref"));
    assert_eq!(need["properties"]["offer"]["format"], json!("board-ref"));
    let offer = serde_json::to_value(schema(RecordType::Offer)).unwrap();
    assert_eq!(
        offer["properties"]["derived_from"]["format"],
        json!("board-ref")
    );
}

#[test]
fn an_optional_field_is_left_out_of_required_and_allows_no_null() {
    let offer = serde_json::to_value(schema(RecordType::Offer)).unwrap();
    let required = offer["required"].as_array().unwrap();
    assert!(!required.contains(&json!("derived_from")));
    assert!(required.contains(&json!("reference")));
    assert_eq!(offer["properties"]["derived_from"]["type"], json!("string"));
    let binding = serde_json::to_value(schema(RecordType::Binding)).unwrap();
    let required = binding["required"].as_array().unwrap();
    assert!(!required.contains(&json!("projection")));
    assert!(!required.contains(&json!("audience")));
}

#[test]
fn a_schema_holds_no_reference_to_a_definition() {
    for record_type in RecordType::ALL {
        let text = serde_json::to_string(&schema(*record_type)).unwrap();
        assert!(!text.contains("\"$ref\""), "{record_type}: {text}");
        assert!(!text.contains("\"$defs\""), "{record_type}: {text}");
    }
}
