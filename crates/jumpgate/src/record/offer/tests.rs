use serde_json::{Value, json};

use super::*;
use crate::error::Reason;

/// Returns a secret offer that every check accepts, as a JSON value.
fn secret_offer() -> Value {
    json!({
        "$type": "jumpgate/offer/v1",
        "provider": {"kind": "secret-store", "name": "one-password"},
        "kind": "secret",
        "name": "shelf-api-token",
        "reference": {"secret": "op://shelf-dev/api-token/credential"},
        "grants": ["shelf"],
        "derived_from": "/jumpgate/offers/secret-store/one-password/shelf-signing-key",
        "scope": "workspace"
    })
}

/// Returns an offer of the port `http` of the app `auth`.
fn port_offer() -> Value {
    json!({
        "$type": "jumpgate/offer/v1",
        "provider": {"kind": "app", "name": "auth"},
        "kind": "address",
        "name": "auth-api",
        "reference": {"port": {"app": "auth", "port": "http"}},
        "grants": [],
        "scope": "app-instance"
    })
}

fn with(mut value: Value, key: &str, field: Value) -> Value {
    value.as_object_mut().unwrap().insert(key.to_owned(), field);
    value
}

fn read(value: &Value) -> Result<Offer, RecordError> {
    Offer::from_json(&value.to_string())
}

fn refusal(value: &Value) -> (Reason, Option<String>) {
    let error = read(value).expect_err("the offer is refused");
    (error.reason(), error.field().map(str::to_owned))
}

fn at(field: &str) -> Option<String> {
    Some(field.to_owned())
}

#[test]
fn a_secret_offer_is_read_with_its_values() {
    let offer = read(&secret_offer()).unwrap();
    assert_eq!(offer.provider().kind(), ProviderKind::SecretStore);
    assert_eq!(offer.provider().name().as_str(), "one-password");
    assert_eq!(offer.kind(), NeedKind::Secret);
    assert_eq!(offer.name().as_str(), "shelf-api-token");
    assert_eq!(
        offer.reference(),
        &Reference::Secret("op://shelf-dev/api-token/credential".parse().unwrap())
    );
    assert_eq!(offer.grants().len(), 1);
    assert_eq!(
        offer.derived_from().map(ToString::to_string).as_deref(),
        Some("/jumpgate/offers/secret-store/one-password/shelf-signing-key")
    );
    assert_eq!(offer.scope(), Scope::Workspace);
}

#[test]
fn an_offer_reads_back_the_json_it_writes_with_its_type_first() {
    for value in [secret_offer(), port_offer()] {
        let offer = read(&value).unwrap();
        let text = serde_json::to_string(&offer).unwrap();
        assert!(
            text.starts_with("{\"$type\":\"jumpgate/offer/v1\","),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value);
        assert_eq!(Offer::from_json(&text).unwrap(), offer);
    }
}

#[test]
fn an_offer_built_in_code_equals_the_one_read_from_json() {
    let built = Offer::new(
        Provider::new(ProviderKind::App, "auth".parse().unwrap()),
        NeedKind::Address,
        "auth-api".parse().unwrap(),
        Reference::Port(PortReference::new(
            "auth".parse().unwrap(),
            "http".parse().unwrap(),
        )),
        Scope::AppInstance,
    )
    .unwrap();
    assert_eq!(built, read(&port_offer()).unwrap());
    assert_eq!(built.derived_from(), None);
}

#[test]
fn an_offer_has_its_path_under_its_provider() {
    let offer = read(&secret_offer()).unwrap();
    assert_eq!(
        offer.path().to_string(),
        "/jumpgate/offers/secret-store/one-password/shelf-api-token"
    );
    assert_eq!(
        offer.record_path().to_string(),
        "/jumpgate/offers/secret-store/one-password/shelf-api-token"
    );
}

#[test]
fn a_route_offer_and_a_unit_offer_are_read() {
    let route = with(
        port_offer(),
        "reference",
        json!({"route": {"app": "auth", "port": "http"}}),
    );
    assert!(read(&route).is_ok());
    let unit = with(
        with(port_offer(), "kind", json!("app")),
        "reference",
        json!({"unit": {"app": "auth", "unit": "Worker"}}),
    );
    assert!(read(&unit).is_ok());
}

#[test]
fn an_offer_whose_reference_is_a_bare_string_is_refused_without_quoting_it() {
    let error = read(&with(secret_offer(), "reference", json!("s3cr3t-value"))).unwrap_err();
    assert_eq!(error.reason(), Reason::Shape);
    assert_eq!(error.field(), Some("reference"));
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
}

#[test]
fn an_offer_whose_secret_reference_is_a_value_is_refused_without_quoting_it() {
    let error = read(&with(
        secret_offer(),
        "reference",
        json!({"secret": "s3cr3t-value"}),
    ))
    .unwrap_err();
    assert_eq!(error.reason(), Reason::SecretReference);
    assert_eq!(error.field(), Some("reference.secret"));
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
}

#[test]
fn an_offer_whose_reference_has_an_unknown_key_is_refused() {
    assert_eq!(
        refusal(&with(
            secret_offer(),
            "reference",
            json!({"value": "s3cr3t"})
        )),
        (Reason::UnknownField, at("reference"))
    );
}

#[test]
fn an_offer_with_a_value_field_is_refused_because_no_field_holds_a_value() {
    assert_eq!(
        refusal(&with(secret_offer(), "value", json!("s3cr3t"))),
        (Reason::UnknownField, None)
    );
}

#[test]
fn a_port_reference_with_an_extra_field_is_refused() {
    let reference = json!({"port": {"app": "auth", "port": "http", "value": "x"}});
    assert_eq!(
        refusal(&with(port_offer(), "reference", reference)),
        (Reason::UnknownField, at("reference.port"))
    );
}

#[test]
fn a_secret_reference_from_an_app_provider_is_refused_with_the_reference_mismatch_code() {
    let offer = with(
        secret_offer(),
        "provider",
        json!({"kind": "app", "name": "auth"}),
    );
    let error = read(&offer).unwrap_err();
    assert_eq!(error.reason(), Reason::ReferenceMismatch);
    assert_eq!(
        error.code().to_string(),
        "jumpgate::record::reference_mismatch"
    );
    assert_eq!(error.field(), Some("reference"));
}

#[test]
fn a_secret_reference_on_an_offer_that_is_not_a_secret_is_refused() {
    assert_eq!(
        refusal(&with(secret_offer(), "kind", json!("value"))),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn a_port_reference_on_an_offer_that_is_not_an_address_is_refused() {
    assert_eq!(
        refusal(&with(port_offer(), "kind", json!("secret"))),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn a_port_reference_from_a_secret_store_is_refused() {
    let offer = with(
        port_offer(),
        "provider",
        json!({"kind": "secret-store", "name": "auth"}),
    );
    assert_eq!(
        refusal(&offer),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn a_port_reference_to_another_app_than_the_provider_is_refused() {
    let reference = json!({"port": {"app": "ledger", "port": "http"}});
    assert_eq!(
        refusal(&with(port_offer(), "reference", reference)),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn a_unit_reference_on_an_address_offer_is_refused() {
    let reference = json!({"unit": {"app": "auth", "unit": "api"}});
    assert_eq!(
        refusal(&with(port_offer(), "reference", reference)),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn building_an_offer_whose_reference_does_not_fit_is_refused() {
    let error = Offer::new(
        Provider::new(ProviderKind::App, "auth".parse().unwrap()),
        NeedKind::Address,
        "auth-api".parse().unwrap(),
        Reference::Secret("op://shelf-dev/api-token/credential".parse().unwrap()),
        Scope::AppInstance,
    )
    .unwrap_err();
    assert_eq!(error.reason(), Reason::ReferenceMismatch);
}

#[test]
fn an_offer_that_grants_an_app_twice_is_refused_at_the_repeat() {
    assert_eq!(
        refusal(&with(secret_offer(), "grants", json!(["shelf", "shelf"]))),
        (Reason::Duplicate, at("grants[1]"))
    );
}

#[test]
fn an_offer_without_grants_is_refused_and_one_with_empty_grants_is_read() {
    let mut value = secret_offer();
    value.as_object_mut().unwrap().remove("grants");
    assert_eq!(refusal(&value), (Reason::MissingField, at("grants")));
    assert!(read(&with(secret_offer(), "grants", json!([]))).is_ok());
}

#[test]
fn an_offer_without_derived_from_is_read_and_a_null_one_is_refused() {
    let mut value = secret_offer();
    value.as_object_mut().unwrap().remove("derived_from");
    assert_eq!(read(&value).unwrap().derived_from(), None);
    assert_eq!(
        refusal(&with(secret_offer(), "derived_from", Value::Null)),
        (Reason::Shape, at("derived_from"))
    );
}

#[test]
fn an_offer_derived_from_a_path_that_is_not_an_offer_is_refused() {
    assert_eq!(
        refusal(&with(
            secret_offer(),
            "derived_from",
            json!("/jumpgate/needs/shelf/token")
        )),
        (Reason::RecordPath, at("derived_from"))
    );
}

#[test]
fn an_offer_with_an_unknown_scope_or_provider_kind_is_refused() {
    assert_eq!(
        refusal(&with(secret_offer(), "scope", json!("global"))),
        (Reason::Keyword, at("scope"))
    );
    assert_eq!(
        refusal(&with(
            secret_offer(),
            "provider",
            json!({"kind": "vault", "name": "x"})
        )),
        (Reason::Keyword, at("provider.kind"))
    );
}

#[test]
fn an_offer_whose_provider_name_breaks_the_name_rule_is_refused() {
    assert_eq!(
        refusal(&with(
            secret_offer(),
            "provider",
            json!({"kind": "secret-store", "name": "Vault-Store"})
        )),
        (Reason::Name, at("provider.name"))
    );
}

#[test]
fn a_secret_reference_from_a_secret_store_not_named_one_password_is_refused() {
    let offer = with(
        secret_offer(),
        "provider",
        json!({"kind": "secret-store", "name": "vault"}),
    );
    let error = read(&offer).unwrap_err();
    assert_eq!(
        (error.reason(), error.field()),
        (Reason::ReferenceMismatch, Some("reference"))
    );
    assert!(error.to_string().contains("one-password"), "{error}");
    assert!(
        error.to_string().contains("no reference variant yet"),
        "{error}"
    );
}

#[test]
fn a_secret_reference_from_the_secret_store_named_one_password_is_read() {
    let offer = read(&secret_offer()).unwrap();
    assert_eq!(offer.provider().name().as_str(), "one-password");
}

#[test]
fn an_offer_from_a_provider_kind_with_no_reference_variant_says_so() {
    for kind in [
        "workspace",
        "capability",
        "cloud-environment",
        "identity-broker",
    ] {
        let offer = with(
            secret_offer(),
            "provider",
            json!({"kind": kind, "name": "one-password"}),
        );
        let error = read(&offer).unwrap_err();
        assert_eq!(error.reason(), Reason::ReferenceMismatch, "{kind}");
        let message = error.to_string();
        assert!(message.contains("no reference variant yet"), "{message}");
        assert!(message.contains(kind), "{message}");
    }
}

#[test]
fn an_offer_of_a_kind_with_no_reference_variant_says_so() {
    for kind in ["value", "data-access", "capability"] {
        let error = read(&with(secret_offer(), "kind", json!(kind))).unwrap_err();
        assert_eq!(error.reason(), Reason::ReferenceMismatch, "{kind}");
        let message = error.to_string();
        assert!(message.contains("no reference variant yet"), "{message}");
        assert!(message.contains(kind), "{message}");
    }
}

#[test]
fn an_offer_of_a_kind_that_has_a_reference_variant_does_not_say_it_has_none() {
    let error = read(&with(port_offer(), "kind", json!("secret"))).unwrap_err();
    assert_eq!(error.reason(), Reason::ReferenceMismatch);
    assert!(
        !error.to_string().contains("no reference variant yet"),
        "{error}"
    );
}
