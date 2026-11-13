use serde_json::{Value, json};

use super::*;
use crate::error::Reason;

/// Returns a need that every check accepts, as a JSON value to change.
fn need() -> Value {
    json!({
        "$type": "jumpgate/need/v1",
        "app": "shelf",
        "name": "api_token",
        "kind": "secret",
        "consumers": ["api", "worker"],
        "declared_in": {"repo": "github.com/acme/shelf", "file": "services/api/starmap.toml"},
        "delivers_as": {"variables": ["API_TOKEN", "LEGACY_TOKEN"]},
        "optional": true,
        "when": [{"profile": "dev"}, {"target": "local"}],
        "when_unbound": {"fallback": "local-placeholder"}
    })
}

/// Returns `value` with the field `key` set to `field`.
fn with(mut value: Value, key: &str, field: Value) -> Value {
    value.as_object_mut().unwrap().insert(key.to_owned(), field);
    value
}

/// Reads `value` as a need.
fn read(value: &Value) -> Result<Need, RecordError> {
    Need::from_json(&value.to_string())
}

/// Returns the reason and field of the refusal of `value`.
fn refusal(value: &Value) -> (Reason, Option<String>) {
    let error = read(value).expect_err("the need is refused");
    (error.reason(), error.field().map(str::to_owned))
}

fn at(field: &str) -> Option<String> {
    Some(field.to_owned())
}

#[test]
fn a_need_with_every_field_set_is_read_with_its_values() {
    let need = read(&need()).unwrap();
    assert_eq!(need.app().as_str(), "shelf");
    assert_eq!(need.name().as_str(), "api_token");
    assert_eq!(need.kind(), NeedKind::Secret);
    let consumers: Vec<&str> = need.consumers().iter().map(UnitName::as_str).collect();
    assert_eq!(consumers, ["api", "worker"]);
    assert_eq!(need.declared_in().repo().as_str(), "github.com/acme/shelf");
    assert_eq!(
        need.declared_in().file().as_str(),
        "services/api/starmap.toml"
    );
    assert!(need.optional());
    assert_eq!(need.when().len(), 2);
    assert_eq!(
        need.when_unbound(),
        &WhenUnbound::Fallback("local-placeholder".parse().unwrap())
    );
}

#[test]
fn a_need_reads_back_the_json_it_writes_with_its_type_first() {
    let need = read(&need()).unwrap();
    let text = serde_json::to_string(&need).unwrap();
    assert!(
        text.starts_with("{\"$type\":\"jumpgate/need/v1\","),
        "{text}"
    );
    assert_eq!(Need::from_json(&text).unwrap(), need);
    let written: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(written, self::need());
}

#[test]
fn a_need_built_in_code_equals_the_one_read_from_json() {
    let built = Need::new(
        "shelf".parse().unwrap(),
        "api_token".parse().unwrap(),
        NeedKind::Secret,
        Distinct::new(vec!["api".parse().unwrap(), "worker".parse().unwrap()]).unwrap(),
        DeclaredIn::new(
            "github.com/acme/shelf".parse().unwrap(),
            "services/api/starmap.toml".parse().unwrap(),
        ),
        DeliversAs::Variables(
            Distinct::new(vec![
                "API_TOKEN".parse().unwrap(),
                "LEGACY_TOKEN".parse().unwrap(),
            ])
            .unwrap(),
        ),
    )
    .with_unbound(
        true,
        WhenUnbound::Fallback("local-placeholder".parse().unwrap()),
    )
    .unwrap()
    .with_when(
        Distinct::new(vec![
            Condition::Profile("dev".parse().unwrap()),
            Condition::Target("local".parse().unwrap()),
        ])
        .unwrap(),
    );
    assert_eq!(built, read(&need()).unwrap());
}

/// Returns a need built in code with the one consumer `api`.
fn built_need() -> Need {
    Need::new(
        "shelf".parse().unwrap(),
        "auth-api".parse().unwrap(),
        NeedKind::Address,
        Distinct::new(vec!["api".parse().unwrap()]).unwrap(),
        DeclaredIn::new(
            "github.com/acme/shelf".parse().unwrap(),
            "mxv.toml".parse().unwrap(),
        ),
        DeliversAs::File("/run/auth-api".parse().unwrap()),
    )
}

#[test]
fn a_new_need_has_its_consumers_is_required_always_and_fails_when_unbound() {
    let need = built_need();
    assert_eq!(need.consumers().len(), 1);
    assert!(!need.optional());
    assert!(need.when().is_empty());
    assert_eq!(need.when_unbound(), &WhenUnbound::Fail);
}

#[test]
fn a_need_has_its_path_under_its_app() {
    let need = read(&need()).unwrap();
    assert_eq!(need.path().to_string(), "/jumpgate/needs/shelf/api_token");
    assert_eq!(
        need.record_path().to_string(),
        "/jumpgate/needs/shelf/api_token"
    );
}

#[test]
fn a_need_without_a_field_is_refused_at_that_field() {
    for key in [
        "app",
        "name",
        "kind",
        "consumers",
        "declared_in",
        "delivers_as",
        "optional",
        "when",
        "when_unbound",
    ] {
        let mut value = need();
        value.as_object_mut().unwrap().remove(key);
        assert_eq!(refusal(&value), (Reason::MissingField, at(key)), "{key}");
    }
}

#[test]
fn a_need_whose_app_breaks_the_name_rule_is_refused_at_app() {
    assert_eq!(
        refusal(&with(need(), "app", json!("Shelf"))),
        (Reason::Name, at("app"))
    );
}

#[test]
fn a_need_whose_name_breaks_the_name_rule_is_refused_at_name() {
    assert_eq!(
        refusal(&with(need(), "name", json!("API_TOKEN"))),
        (Reason::Name, at("name"))
    );
}

#[test]
fn a_need_of_an_unknown_kind_is_refused_at_kind() {
    assert_eq!(
        refusal(&with(need(), "kind", json!("password"))),
        (Reason::Keyword, at("kind"))
    );
}

#[test]
fn a_need_of_each_kind_is_read() {
    for kind in [
        "secret",
        "address",
        "value",
        "data-access",
        "capability",
        "app",
    ] {
        assert!(read(&with(need(), "kind", json!(kind))).is_ok(), "{kind}");
    }
}

#[test]
fn a_need_with_a_consumer_that_breaks_the_unit_rule_is_refused_at_its_index() {
    assert_eq!(
        refusal(&with(need(), "consumers", json!(["api", "-worker"]))),
        (Reason::Name, at("consumers[1]"))
    );
}

#[test]
fn a_need_that_names_a_consumer_twice_is_refused_at_the_repeat() {
    assert_eq!(
        refusal(&with(need(), "consumers", json!(["api", "api"]))),
        (Reason::Duplicate, at("consumers[1]"))
    );
}

#[test]
fn a_need_with_no_consumers_is_refused_with_the_empty_code() {
    assert_eq!(
        refusal(&with(need(), "consumers", json!([]))),
        (Reason::Empty, at("consumers"))
    );
}

#[test]
fn a_need_with_one_consumer_is_read() {
    let need = read(&with(need(), "consumers", json!(["api"]))).unwrap();
    assert_eq!(need.consumers().len(), 1);
}

#[test]
fn a_need_whose_repository_is_not_host_owner_name_is_refused_at_declared_in_repo() {
    let declared_in = json!({"repo": "shelf", "file": "starmap.toml"});
    assert_eq!(
        refusal(&with(need(), "declared_in", declared_in)),
        (Reason::Repository, at("declared_in.repo"))
    );
}

#[test]
fn a_need_declared_in_a_file_outside_its_repository_is_refused_at_declared_in_file() {
    let declared_in = json!({"repo": "github.com/acme/shelf", "file": "../starmap.toml"});
    assert_eq!(
        refusal(&with(need(), "declared_in", declared_in)),
        (Reason::FilePath, at("declared_in.file"))
    );
}

#[test]
fn a_need_whose_declared_in_has_an_extra_field_is_refused() {
    let declared_in = json!({"repo": "github.com/acme/shelf", "file": "starmap.toml", "line": 3});
    assert_eq!(
        refusal(&with(need(), "declared_in", declared_in)),
        (Reason::UnknownField, at("declared_in"))
    );
}

#[test]
fn a_need_delivered_as_a_file_is_read() {
    let need = read(&with(
        need(),
        "delivers_as",
        json!({"file": "/run/secrets/api-token"}),
    ))
    .unwrap();
    assert_eq!(
        need.delivers_as(),
        &DeliversAs::File("/run/secrets/api-token".parse().unwrap())
    );
}

#[test]
fn a_need_delivered_as_no_variables_is_refused_with_the_empty_code() {
    assert_eq!(
        refusal(&with(need(), "delivers_as", json!({"variables": []}))),
        (Reason::Empty, at("delivers_as.variables"))
    );
}

#[test]
fn a_need_delivered_as_a_variable_that_breaks_the_variable_rule_is_refused_at_its_index() {
    assert_eq!(
        refusal(&with(
            need(),
            "delivers_as",
            json!({"variables": ["API-TOKEN"]})
        )),
        (Reason::Variable, at("delivers_as.variables[0]"))
    );
}

#[test]
fn a_need_delivered_as_both_variables_and_a_file_is_refused_with_the_shape_code() {
    let delivers_as = json!({"variables": ["API_TOKEN"], "file": "token"});
    assert_eq!(
        refusal(&with(need(), "delivers_as", delivers_as)),
        (Reason::Shape, at("delivers_as"))
    );
}

#[test]
fn a_need_delivered_as_a_file_path_with_a_dot_dot_segment_is_refused() {
    assert_eq!(
        refusal(&with(need(), "delivers_as", json!({"file": "../token"}))),
        (Reason::FilePath, at("delivers_as.file"))
    );
}

#[test]
fn a_need_whose_optional_is_not_a_boolean_is_refused_at_optional() {
    assert_eq!(
        refusal(&with(need(), "optional", json!("yes"))),
        (Reason::Shape, at("optional"))
    );
}

#[test]
fn a_need_with_a_condition_of_an_unknown_kind_is_refused_at_its_index() {
    assert_eq!(
        refusal(&with(need(), "when", json!([{"platform": "linux"}]))),
        (Reason::UnknownField, at("when[0]"))
    );
}

#[test]
fn a_need_with_a_profile_that_breaks_the_name_rule_is_refused_inside_the_condition() {
    assert_eq!(
        refusal(&with(need(), "when", json!([{"profile": "Dev"}]))),
        (Reason::Name, at("when[0].profile"))
    );
}

#[test]
fn a_need_that_repeats_a_condition_is_refused() {
    assert_eq!(
        refusal(&with(
            need(),
            "when",
            json!([{"target": "local"}, {"target": "local"}])
        )),
        (Reason::Duplicate, at("when[1]"))
    );
}

/// Returns the fixture need with `optional` and `when_unbound` set.
fn unbound(optional: bool, when_unbound: Value) -> Value {
    with(
        with(need(), "optional", json!(optional)),
        "when_unbound",
        when_unbound,
    )
}

#[test]
fn a_required_need_that_fails_and_an_optional_one_that_is_omitted_are_read() {
    let fail = read(&unbound(false, json!("fail"))).unwrap();
    assert_eq!(fail.when_unbound(), &WhenUnbound::Fail);
    let omit = read(&unbound(true, json!("omit"))).unwrap();
    assert_eq!(omit.when_unbound(), &WhenUnbound::Omit);
}

#[test]
fn a_required_need_that_is_omitted_when_unbound_is_refused() {
    let error = read(&unbound(false, json!("omit"))).unwrap_err();
    assert_eq!(
        (error.reason(), error.field()),
        (Reason::OptionalConflict, Some("when_unbound"))
    );
    assert_eq!(
        error.code().to_string(),
        "jumpgate::record::optional_conflict"
    );
}

#[test]
fn an_optional_need_that_fails_when_unbound_is_refused() {
    assert_eq!(
        refusal(&unbound(true, json!("fail"))),
        (Reason::OptionalConflict, at("when_unbound"))
    );
}

#[test]
fn a_need_with_a_fallback_is_read_whether_it_is_optional_or_required() {
    for optional in [false, true] {
        assert!(
            read(&unbound(optional, json!({"fallback": "x"}))).is_ok(),
            "{optional}"
        );
    }
}

#[test]
fn building_a_need_whose_optional_flag_contradicts_when_unbound_is_refused() {
    for (optional, when_unbound) in [(false, WhenUnbound::Omit), (true, WhenUnbound::Fail)] {
        let error = built_need()
            .with_unbound(optional, when_unbound)
            .unwrap_err();
        assert_eq!(error.reason(), Reason::OptionalConflict);
    }
}

#[test]
fn building_a_need_whose_optional_flag_agrees_with_when_unbound_is_accepted() {
    let need = built_need().with_unbound(true, WhenUnbound::Omit).unwrap();
    assert!(need.optional());
    assert_eq!(need.when_unbound(), &WhenUnbound::Omit);
}

#[test]
fn a_need_with_an_unknown_when_unbound_word_is_refused_with_the_keyword_code() {
    assert_eq!(
        refusal(&with(need(), "when_unbound", json!("ignore"))),
        (Reason::Keyword, at("when_unbound"))
    );
}

#[test]
fn a_need_whose_fallback_holds_a_nul_character_is_refused_inside_when_unbound() {
    assert_eq!(
        refusal(&with(
            need(),
            "when_unbound",
            json!({"fallback": "a\u{0}b"})
        )),
        (Reason::Fallback, at("when_unbound.fallback"))
    );
}

#[test]
fn a_need_whose_when_unbound_is_a_number_or_a_fallback_number_is_refused_with_the_shape_code() {
    assert_eq!(
        refusal(&with(need(), "when_unbound", json!(3))),
        (Reason::Shape, at("when_unbound"))
    );
    assert_eq!(
        refusal(&with(need(), "when_unbound", json!({"fallback": 3}))),
        (Reason::Shape, at("when_unbound.fallback"))
    );
}

#[test]
fn a_need_whose_when_unbound_object_has_another_key_is_refused() {
    assert_eq!(
        refusal(&with(need(), "when_unbound", json!({"default": "x"}))),
        (Reason::UnknownField, at("when_unbound"))
    );
}
