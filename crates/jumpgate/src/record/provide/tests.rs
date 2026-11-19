use serde_json::{Value, json};

use super::*;
use crate::error::Reason;

/// Returns a secret provide that every check accepts, as a JSON value.
fn secret_provide() -> Value {
    json!({
        "$type": "jumpgate/provide/v1",
        "provider": {"kind": "secret-store", "name": "one-password"},
        "kind": "secret",
        "name": "shelf-api-token",
        "reference": {"secret": "op://shelf-dev/api-token/credential"},
        "grants": ["shelf"],
        "derived_from": "/jumpgate/provides/secret-store/one-password/shelf-signing-key",
        "scope": "workspace"
    })
}

/// Returns a provide of the port `http` of the app `auth`.
fn port_provide() -> Value {
    json!({
        "$type": "jumpgate/provide/v1",
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

fn read(value: &Value) -> Result<Provide, RecordError> {
    Provide::from_json(&value.to_string())
}

fn refusal(value: &Value) -> (Reason, Option<String>) {
    let error = read(value).expect_err("the provide is refused");
    (error.reason(), error.field().map(str::to_owned))
}

fn at(field: &str) -> Option<String> {
    Some(field.to_owned())
}

#[test]
fn a_secret_provide_is_read_with_its_values() {
    let provide = read(&secret_provide()).unwrap();
    assert_eq!(provide.provider().kind(), ProviderKind::SecretStore);
    assert_eq!(provide.provider().name().as_str(), "one-password");
    assert_eq!(provide.kind(), NeedKind::Secret);
    assert_eq!(provide.name().as_str(), "shelf-api-token");
    assert_eq!(
        provide.reference(),
        &Reference::Secret("op://shelf-dev/api-token/credential".parse().unwrap())
    );
    assert_eq!(provide.grants().len(), 1);
    assert_eq!(
        provide.derived_from().map(ToString::to_string).as_deref(),
        Some("/jumpgate/provides/secret-store/one-password/shelf-signing-key")
    );
    assert_eq!(provide.scope(), Scope::Workspace);
}

#[test]
fn a_provide_reads_back_the_json_it_writes_with_its_type_first() {
    for value in [secret_provide(), port_provide()] {
        let provide = read(&value).unwrap();
        let text = serde_json::to_string(&provide).unwrap();
        assert!(
            text.starts_with("{\"$type\":\"jumpgate/provide/v1\","),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value);
        assert_eq!(Provide::from_json(&text).unwrap(), provide);
    }
}

#[test]
fn a_provide_built_in_code_equals_the_one_read_from_json() {
    let built = Provide::new(
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
    assert_eq!(built, read(&port_provide()).unwrap());
    assert_eq!(built.derived_from(), None);
}

#[test]
fn a_provide_has_its_path_under_its_provider() {
    let provide = read(&secret_provide()).unwrap();
    assert_eq!(
        provide.path().to_string(),
        "/jumpgate/provides/secret-store/one-password/shelf-api-token"
    );
    assert_eq!(
        provide.record_path().to_string(),
        "/jumpgate/provides/secret-store/one-password/shelf-api-token"
    );
}

#[test]
fn a_route_provide_and_a_unit_provide_are_read() {
    let route = with(
        port_provide(),
        "reference",
        json!({"route": {"app": "auth", "port": "http"}}),
    );
    assert!(read(&route).is_ok());
    let unit = with(
        with(port_provide(), "kind", json!("app")),
        "reference",
        json!({"unit": {"app": "auth", "unit": "Worker"}}),
    );
    assert!(read(&unit).is_ok());
}

#[test]
fn a_provide_whose_reference_is_a_bare_string_is_refused_without_quoting_it() {
    let error = read(&with(secret_provide(), "reference", json!("s3cr3t-value"))).unwrap_err();
    assert_eq!(error.reason(), Reason::Shape);
    assert_eq!(error.field(), Some("reference"));
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
}

#[test]
fn a_provide_whose_secret_reference_is_a_value_is_refused_without_quoting_it() {
    let error = read(&with(
        secret_provide(),
        "reference",
        json!({"secret": "s3cr3t-value"}),
    ))
    .unwrap_err();
    assert_eq!(error.reason(), Reason::SecretReference);
    assert_eq!(error.field(), Some("reference.secret"));
    assert!(!error.to_string().contains("s3cr3t"), "{error}");
}

#[test]
fn a_provide_whose_reference_has_an_unknown_key_is_refused() {
    assert_eq!(
        refusal(&with(
            secret_provide(),
            "reference",
            json!({"value": "s3cr3t"})
        )),
        (Reason::UnknownField, at("reference"))
    );
}

#[test]
fn a_provide_with_a_value_field_is_refused_because_no_field_holds_a_value() {
    assert_eq!(
        refusal(&with(secret_provide(), "value", json!("s3cr3t"))),
        (Reason::UnknownField, None)
    );
}

#[test]
fn a_port_reference_with_an_extra_field_is_refused() {
    let reference = json!({"port": {"app": "auth", "port": "http", "value": "x"}});
    assert_eq!(
        refusal(&with(port_provide(), "reference", reference)),
        (Reason::UnknownField, at("reference.port"))
    );
}

#[test]
fn a_secret_reference_from_an_app_provider_is_refused_with_the_reference_mismatch_code() {
    let provide = with(
        secret_provide(),
        "provider",
        json!({"kind": "app", "name": "auth"}),
    );
    let error = read(&provide).unwrap_err();
    assert_eq!(error.reason(), Reason::ReferenceMismatch);
    assert_eq!(
        error.code().to_string(),
        "jumpgate::record::reference_mismatch"
    );
    assert_eq!(error.field(), Some("reference"));
}

#[test]
fn a_secret_reference_on_a_provide_that_is_not_a_secret_is_refused() {
    assert_eq!(
        refusal(&with(secret_provide(), "kind", json!("value"))),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn a_port_reference_on_a_provide_that_is_not_an_address_is_refused() {
    assert_eq!(
        refusal(&with(port_provide(), "kind", json!("secret"))),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn a_port_reference_from_a_secret_store_is_refused() {
    let provide = with(
        port_provide(),
        "provider",
        json!({"kind": "secret-store", "name": "auth"}),
    );
    assert_eq!(
        refusal(&provide),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn a_port_reference_to_another_app_than_the_provider_is_refused() {
    let reference = json!({"port": {"app": "ledger", "port": "http"}});
    assert_eq!(
        refusal(&with(port_provide(), "reference", reference)),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn a_unit_reference_on_an_address_provide_is_refused() {
    let reference = json!({"unit": {"app": "auth", "unit": "api"}});
    assert_eq!(
        refusal(&with(port_provide(), "reference", reference)),
        (Reason::ReferenceMismatch, at("reference"))
    );
}

#[test]
fn building_a_provide_whose_reference_does_not_fit_is_refused() {
    let error = Provide::new(
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
fn a_provide_that_grants_an_app_twice_is_refused_at_the_repeat() {
    assert_eq!(
        refusal(&with(secret_provide(), "grants", json!(["shelf", "shelf"]))),
        (Reason::Duplicate, at("grants[1]"))
    );
}

#[test]
fn a_provide_without_grants_is_refused_and_one_with_empty_grants_is_read() {
    let mut value = secret_provide();
    value.as_object_mut().unwrap().remove("grants");
    assert_eq!(refusal(&value), (Reason::MissingField, at("grants")));
    assert!(read(&with(secret_provide(), "grants", json!([]))).is_ok());
}

#[test]
fn a_provide_without_derived_from_is_read_and_a_null_one_is_refused() {
    let mut value = secret_provide();
    value.as_object_mut().unwrap().remove("derived_from");
    assert_eq!(read(&value).unwrap().derived_from(), None);
    assert_eq!(
        refusal(&with(secret_provide(), "derived_from", Value::Null)),
        (Reason::Shape, at("derived_from"))
    );
}

#[test]
fn a_provide_derived_from_a_path_that_is_not_a_provide_is_refused() {
    assert_eq!(
        refusal(&with(
            secret_provide(),
            "derived_from",
            json!("/jumpgate/needs/shelf/token")
        )),
        (Reason::RecordPath, at("derived_from"))
    );
}

#[test]
fn a_provide_with_an_unknown_scope_or_provider_kind_is_refused() {
    assert_eq!(
        refusal(&with(secret_provide(), "scope", json!("global"))),
        (Reason::Keyword, at("scope"))
    );
    assert_eq!(
        refusal(&with(
            secret_provide(),
            "provider",
            json!({"kind": "vault", "name": "x"})
        )),
        (Reason::Keyword, at("provider.kind"))
    );
}

#[test]
fn a_provide_whose_provider_name_breaks_the_name_rule_is_refused() {
    assert_eq!(
        refusal(&with(
            secret_provide(),
            "provider",
            json!({"kind": "secret-store", "name": "Vault-Store"})
        )),
        (Reason::Name, at("provider.name"))
    );
}

#[test]
fn a_secret_reference_from_a_secret_store_not_named_one_password_is_refused() {
    let provide = with(
        secret_provide(),
        "provider",
        json!({"kind": "secret-store", "name": "vault"}),
    );
    let error = read(&provide).unwrap_err();
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
    let provide = read(&secret_provide()).unwrap();
    assert_eq!(provide.provider().name().as_str(), "one-password");
}

#[test]
fn a_provide_from_a_provider_kind_with_no_reference_variant_says_so() {
    for kind in [
        "workspace",
        "capability",
        "cloud-environment",
        "identity-broker",
    ] {
        let provide = with(
            secret_provide(),
            "provider",
            json!({"kind": kind, "name": "one-password"}),
        );
        let error = read(&provide).unwrap_err();
        assert_eq!(error.reason(), Reason::ReferenceMismatch, "{kind}");
        let message = error.to_string();
        assert!(message.contains("no reference variant yet"), "{message}");
        assert!(message.contains(kind), "{message}");
    }
}

#[test]
fn a_provide_of_a_kind_with_no_reference_variant_says_so() {
    for kind in ["value", "capability"] {
        let error = read(&with(secret_provide(), "kind", json!(kind))).unwrap_err();
        assert_eq!(error.reason(), Reason::ReferenceMismatch, "{kind}");
        let message = error.to_string();
        assert!(message.contains("no reference variant yet"), "{message}");
        assert!(message.contains(kind), "{message}");
    }
}

#[test]
fn a_provide_of_a_kind_that_has_a_reference_variant_does_not_say_it_has_none() {
    let error = read(&with(port_provide(), "kind", json!("secret"))).unwrap_err();
    assert_eq!(error.reason(), Reason::ReferenceMismatch);
    assert!(
        !error.to_string().contains("no reference variant yet"),
        "{error}"
    );
}

/// Returns a provide of the socket `deadband.sock` of the unit `deadband` of
/// the app `deadband`.
fn socket_provide() -> Value {
    json!({
        "$type": "jumpgate/provide/v1",
        "provider": {"kind": "app", "name": "deadband"},
        "kind": "socket",
        "name": "deadband-socket",
        "reference": {"socket": {
            "app": "deadband",
            "unit": "deadband",
            "path": "/work/deadband/target/deadband.sock"
        }},
        "grants": [],
        "scope": "app-instance"
    })
}

#[test]
fn a_socket_provide_is_read_and_written_back_unchanged() {
    let provide = read(&socket_provide()).unwrap();
    assert_eq!(provide.kind(), NeedKind::Socket);
    let Reference::Socket(socket) = provide.reference() else {
        panic!("the reference is a socket");
    };
    assert_eq!(socket.app().as_str(), "deadband");
    assert_eq!(socket.unit().as_str(), "deadband");
    assert_eq!(
        socket.path().as_str(),
        "/work/deadband/target/deadband.sock"
    );
    assert_eq!(serde_json::to_value(&provide).unwrap(), socket_provide());
    assert_eq!(
        provide.path().to_string(),
        "/jumpgate/provides/app/deadband/deadband-socket"
    );
}

#[test]
fn a_socket_reference_on_a_provide_that_is_not_a_socket_is_refused() {
    let value = with(socket_provide(), "kind", json!("address"));
    assert_eq!(refusal(&value).0, Reason::ReferenceMismatch);
}

#[test]
fn a_socket_provide_of_another_app_than_its_provider_is_refused() {
    let mut value = socket_provide();
    value["reference"]["socket"]["app"] = json!("other");
    assert_eq!(refusal(&value).0, Reason::ReferenceMismatch);
}

#[test]
fn a_socket_provide_from_a_secret_store_is_refused() {
    let mut value = socket_provide();
    value["provider"] = json!({"kind": "secret-store", "name": "one-password"});
    assert_eq!(refusal(&value).0, Reason::ReferenceMismatch);
}

#[test]
fn a_socket_reference_with_a_relative_path_or_a_missing_key_is_refused() {
    let mut relative = socket_provide();
    relative["reference"]["socket"]["path"] = json!("target/deadband.sock");
    assert_eq!(
        refusal(&relative),
        (Reason::FilePath, at("reference.socket.path"))
    );
    let mut lacking = socket_provide();
    lacking["reference"]["socket"]
        .as_object_mut()
        .unwrap()
        .remove("unit");
    assert_eq!(refusal(&lacking).0, Reason::MissingField);
}

/// Returns a provide of the local service `store` of the workspace `mxv`.
fn service_provide() -> Value {
    json!({
        "$type": "jumpgate/provide/v1",
        "provider": {"kind": "workspace", "name": "mxv"},
        "kind": "data-access",
        "name": "store",
        "reference": {"service": {"kind": "dynamodb", "name": "store"}},
        "grants": [],
        "scope": "workspace"
    })
}

#[test]
fn a_service_provide_of_the_workspace_mxv_is_read_with_its_kind_and_name() {
    let provide = read(&service_provide()).unwrap();
    assert_eq!(provide.provider().kind(), ProviderKind::Workspace);
    assert_eq!(provide.kind(), NeedKind::DataAccess);
    assert_eq!(
        provide.path().to_string(),
        "/jumpgate/provides/workspace/mxv/store"
    );
    let Reference::Service(service) = provide.reference() else {
        panic!("the provide holds a service reference");
    };
    assert_eq!(service.kind(), ServiceKind::Dynamodb);
    assert_eq!(service.name().as_str(), "store");
}

#[test]
fn a_service_provide_written_back_reads_to_the_same_provide() {
    let provide = read(&service_provide()).unwrap();
    let text = serde_json::to_string(&provide).unwrap();
    assert_eq!(Provide::from_json(&text).unwrap(), provide);
}

#[test]
fn a_service_reference_that_does_not_fit_its_provide_is_refused() {
    for provide in [
        with(service_provide(), "kind", json!("secret")),
        with(
            service_provide(),
            "provider",
            json!({"kind": "app", "name": "store"}),
        ),
    ] {
        assert_eq!(
            refusal(&provide),
            (Reason::ReferenceMismatch, at("reference"))
        );
    }
}

#[test]
fn a_workspace_provide_not_named_mxv_and_a_service_of_an_unknown_kind_are_refused() {
    let other = with(
        service_provide(),
        "provider",
        json!({"kind": "workspace", "name": "other"}),
    );
    let message = read(&other).unwrap_err().to_string();
    assert!(message.contains("no reference variant yet"), "{message}");
    let unknown = with(
        service_provide(),
        "reference",
        json!({"service": {"kind": "redis", "name": "store"}}),
    );
    assert_eq!(
        refusal(&unknown),
        (Reason::Keyword, at("reference.service.kind"))
    );
}

#[test]
fn a_service_reference_with_an_extra_key_is_refused() {
    let provide = with(
        service_provide(),
        "reference",
        json!({"service": {"kind": "s3", "name": "store", "region": "x"}}),
    );
    assert_eq!(
        refusal(&provide),
        (Reason::UnknownField, at("reference.service"))
    );
}
