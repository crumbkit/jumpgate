//! The JSON Schemas of the record types refuse what the record parser
//! refuses.
//!
//! Each case changes one field of a record that both accept, and checks
//! that the parser and the schema both refuse the result. A few rules hold
//! across fields or count bytes, which a JSON Schema cannot express; each
//! of those is a gap case, which checks that the parser refuses it, that
//! the schema accepts it, and that the schema's text names the rule.

#![cfg(test)]

use jsonschema::Validator;
use jumpgate::record::schema;
use jumpgate::{Binding, Need, Offer, Record, RecordType, Target, Unit};
use serde_json::{Value, json};

/// Returns a need that the parser and the schema both accept.
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

/// Returns a need with a request that both accept.
fn need_with_request() -> Value {
    set(
        need(),
        "/requests",
        Some(json!({"want": {"repo": "github.com/acme/auth", "provides": "auth-api"}})),
    )
}

/// Returns an offer that the parser and the schema both accept.
fn offer() -> Value {
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

/// Returns an offer of an app's port that both accept.
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

/// Returns a binding that both accept.
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

/// Returns a target that both accept.
fn target() -> Value {
    json!({
        "$type": "jumpgate/target/v1",
        "name": "local",
        "kind": "local",
        "authority": "personal"
    })
}

/// Returns a unit that both accept.
fn unit() -> Value {
    json!({
        "$type": "jumpgate/unit/v1",
        "app": "shelf",
        "name": "api",
        "repo": "github.com/acme/shelf",
        "kind": "process",
        "starts_after": ["db"],
        "ports": ["http"]
    })
}

/// Returns `value` with the field at the JSON pointer `pointer` set to
/// `field`, or removed when `field` is `None`.
fn set(mut value: Value, pointer: &str, field: Option<Value>) -> Value {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    let object = value
        .pointer_mut(parent)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("{pointer} has no parent object"));
    match field {
        Some(field) => object.insert(key.to_owned(), field),
        None => object.remove(key),
    };
    value
}

/// A record type, a function that returns its base record, and a function
/// that reports whether its parser accepts a text.
type Entry = (RecordType, fn() -> Value, fn(&str) -> bool);

/// Each record type, with its base record and its parser.
const RECORDS: [Entry; 5] = [
    (RecordType::Need, need, |text| Need::from_json(text).is_ok()),
    (RecordType::Offer, offer, |text| {
        Offer::from_json(text).is_ok()
    }),
    (RecordType::Binding, binding, |text| {
        Binding::from_json(text).is_ok()
    }),
    (RecordType::Target, target, |text| {
        Target::from_json(text).is_ok()
    }),
    (RecordType::Unit, unit, |text| Unit::from_json(text).is_ok()),
];

/// Returns the entry of `record_type` in [`RECORDS`].
fn entry(record_type: RecordType) -> &'static Entry {
    RECORDS
        .iter()
        .find(|(listed, _, _)| *listed == record_type)
        .expect("every record type has an entry")
}

/// Reports whether the parser of `record_type` accepts `value`.
fn parser_accepts(record_type: RecordType, value: &Value) -> bool {
    (entry(record_type).2)(&value.to_string())
}

/// Returns the validator of the schema of `record_type`.
fn validator(record_type: RecordType) -> Validator {
    jsonschema::draft202012::new(&schema(record_type).to_value()).unwrap()
}

/// Returns the base record of `record_type`.
fn base(record_type: RecordType) -> Value {
    (entry(record_type).1)()
}

/// The record type, the field and the value of each case that both the
/// parser and the schema refuse. A value of `None` removes the field.
fn refusals() -> Vec<(RecordType, &'static str, Option<Value>)> {
    use RecordType::{Need, Offer};
    let mut cases = vec![
        (Need, "/$type", Some(json!("jumpgate/offer/v1"))),
        (Need, "/value", Some(json!("s3cr3t"))),
        (Need, "/app", Some(json!("Shelf"))),
        (Need, "/app", Some(json!("-shelf"))),
        (Need, "/app", Some(json!(""))),
        (Need, "/app", Some(json!("a".repeat(65)))),
        (Need, "/name", Some(json!("API_TOKEN"))),
        (Need, "/kind", Some(json!("password"))),
        (Need, "/consumers", Some(json!([]))),
        (Need, "/consumers", Some(json!(["-api"]))),
        (Need, "/consumers", Some(json!(["api", "api"]))),
        (Need, "/declared_in/line", Some(json!(3))),
        (Need, "/declared_in/repo", Some(json!("shelf"))),
        (
            Need,
            "/declared_in/repo",
            Some(json!("GitHub.com/acme/shelf")),
        ),
        (
            Need,
            "/declared_in/repo",
            Some(json!("localhost/acme/shelf")),
        ),
        (
            Need,
            "/declared_in/repo",
            Some(json!("github..com/acme/shelf")),
        ),
        (
            Need,
            "/declared_in/repo",
            Some(json!("github.com/acme/shelf/x")),
        ),
        (
            Need,
            "/declared_in/repo",
            Some(json!("github.com/acme/a..b")),
        ),
        (
            Need,
            "/declared_in/repo",
            Some(json!("github.com/a..b/shelf")),
        ),
        (Need, "/declared_in/repo", Some(json!("github.com/acme/.."))),
        (
            Need,
            "/declared_in/repo",
            Some(json!("github.com/.acme/shelf")),
        ),
        (
            Need,
            "/declared_in/repo",
            Some(json!(format!("{}.com/a/b", "a".repeat(250)))),
        ),
        (Need, "/delivers_as", Some(json!({}))),
        (Need, "/delivers_as", Some(json!({"env": "TOKEN"}))),
        (
            Need,
            "/delivers_as",
            Some(json!({"variables": ["A"], "file": "token"})),
        ),
        (Need, "/delivers_as", Some(json!({"variables": []}))),
        (Need, "/delivers_as", Some(json!({"variables": ["1A"]}))),
        (Need, "/delivers_as", Some(json!({"variables": ["A", "A"]}))),
        (Need, "/delivers_as", Some(json!({"file": "/"}))),
        (Need, "/delivers_as", Some(json!({"file": ""}))),
        (Need, "/delivers_as", Some(json!({"file": "/run/../token"}))),
        (Need, "/optional", Some(json!("yes"))),
        (Need, "/when", Some(json!([{"profile": "Dev"}]))),
        (Need, "/when", Some(json!([{"platform": "linux"}]))),
        (
            Need,
            "/when",
            Some(json!([{"target": "local"}, {"target": "local"}])),
        ),
        (Need, "/requests", Some(json!({"fetch": "x"}))),
        (Need, "/requests", Some(json!({}))),
        (Need, "/requests", Some(Value::Null)),
        (
            Need,
            "/requests",
            Some(json!({"want": {"repo": "auth", "provides": "api"}})),
        ),
        (
            Need,
            "/requests",
            Some(json!({"want": {"repo": "github.com/acme/auth", "provides": "Api"}})),
        ),
        (
            Need,
            "/requests",
            Some(json!({"bind": "/jumpgate/needs/a/b"})),
        ),
        (Need, "/when_unbound", Some(json!("ignore"))),
        (Need, "/when_unbound", Some(Value::Null)),
        (Need, "/when_unbound", Some(json!({"default": "x"}))),
        (Need, "/when_unbound", Some(json!({"fallback": "a\u{0}b"}))),
        (
            Need,
            "/when_unbound",
            Some(json!({"fallback": "a".repeat(4097)})),
        ),
    ];
    cases.extend(other_refusals());
    for file in [
        "/starmap.toml",
        "../starmap.toml",
        "services/../starmap.toml",
        "services/./starmap.toml",
        "services//starmap.toml",
        "services/",
        "services\\starmap.toml",
        "services/star\u{7}map.toml",
        "services/star\u{85}map.toml",
        "",
    ] {
        cases.push((Need, "/declared_in/file", Some(json!(file))));
    }
    cases.push((Need, "/declared_in/file", Some(json!("a".repeat(1025)))));
    for reference in [
        "hunter2",
        "OP://vault/item/field",
        "op://vault/item",
        "op://vault/item/section/field/extra",
        "op://./item/field",
        "op://vault/../field",
        "op://vault/item/section/..",
        "op://vault//field",
        "op://vault/item/fie\nld",
        "op://vault/item/fie\u{9f}ld",
    ] {
        cases.push((Offer, "/reference/secret", Some(json!(reference))));
    }
    for record_type in RecordType::ALL {
        let fields: Vec<String> = base(*record_type)
            .as_object()
            .unwrap()
            .keys()
            .filter(|key| !matches!(key.as_str(), "derived_from" | "projection" | "audience"))
            .map(|key| format!("/{key}"))
            .collect();
        for field in fields {
            cases.push((*record_type, field.leak(), None));
        }
    }
    cases
}

/// The cases of [`refusals`] for offers, bindings and targets.
fn other_refusals() -> Vec<(RecordType, &'static str, Option<Value>)> {
    use RecordType::{Binding, Offer, Target, Unit};
    vec![
        (Unit, "/kind", Some(json!("task"))),
        (Unit, "/name", Some(json!("api!"))),
        (Unit, "/repo", Some(json!("shelf"))),
        (Unit, "/starts_after", Some(json!(["db", "db"]))),
        (Unit, "/ports", Some(json!(["-http"]))),
        (Unit, "/command", Some(json!("./api"))),
        (Offer, "/value", Some(json!("s3cr3t"))),
        (Offer, "/provider/kind", Some(json!("vault"))),
        (Offer, "/provider/name", Some(json!("One Password"))),
        (Offer, "/kind", Some(json!("password"))),
        (Offer, "/name", Some(json!("Token"))),
        (Offer, "/reference", Some(json!({"value": "s3cr3t"}))),
        (Offer, "/grants", Some(json!(["shelf", "shelf"]))),
        (Offer, "/grants", Some(json!(["Shelf"]))),
        (Offer, "/derived_from", Some(Value::Null)),
        (
            Offer,
            "/derived_from",
            Some(json!("/jumpgate/needs/shelf/token")),
        ),
        (Offer, "/scope", Some(json!("global"))),
        (Binding, "/delivers_as", Some(json!({"variables": ["A"]}))),
        (Binding, "/value", Some(json!("s3cr3t"))),
        (
            Binding,
            "/need",
            Some(json!("/jumpgate/needs/shelf/../token")),
        ),
        (Binding, "/need", Some(json!("/jumpgate/needs/Shelf/token"))),
        (
            Binding,
            "/offer",
            Some(json!("op://shelf-dev/api-token/credential")),
        ),
        (Binding, "/target", Some(json!("Local"))),
        (Binding, "/projection", Some(Value::Null)),
        (Binding, "/audience", Some(json!("https://ledger.example"))),
        (Target, "/kind", Some(json!("staging"))),
        (Target, "/name", Some(json!("Local"))),
        (Target, "/authority", Some(json!("/repos/acme"))),
    ]
}

#[test]
fn every_base_record_is_accepted_by_the_parser_and_the_schema() {
    for (record_type, value) in RecordType::ALL
        .iter()
        .map(|record_type| (*record_type, base(*record_type)))
        .chain([
            (RecordType::Offer, port_offer()),
            (RecordType::Need, need_with_request()),
            (
                RecordType::Need,
                set(
                    need(),
                    "/requests",
                    Some(json!({"bind": "/jumpgate/offers/secret-store/one-password/token"})),
                ),
            ),
        ])
    {
        assert!(parser_accepts(record_type, &value), "{record_type}");
        assert!(validator(record_type).is_valid(&value), "{record_type}");
    }
}

#[test]
fn a_four_part_secret_reference_and_a_trailing_dot_repository_are_accepted_by_both() {
    for (record_type, value) in [
        (
            RecordType::Offer,
            set(
                offer(),
                "/reference/secret",
                Some(json!("op://Équipe/clé d'API/section/mot de passe")),
            ),
        ),
        (
            RecordType::Offer,
            set(
                offer(),
                "/reference/secret",
                Some(json!(format!("op://{}/item/field", "a".repeat(4096)))),
            ),
        ),
        (
            RecordType::Need,
            set(
                need(),
                "/declared_in/repo",
                Some(json!("github.com/acme/shelf.")),
            ),
        ),
        (
            RecordType::Need,
            set(need(), "/declared_in/file", Some(json!("...hidden/.env"))),
        ),
        (
            RecordType::Need,
            set(
                need(),
                "/delivers_as",
                Some(json!({"file": "/run/secrets/token"})),
            ),
        ),
    ] {
        assert!(parser_accepts(record_type, &value), "{value}");
        assert!(validator(record_type).is_valid(&value), "{value}");
    }
}

#[test]
fn every_value_the_parser_refuses_the_schema_refuses_too() {
    let mut accepted = Vec::new();
    for (record_type, field, value) in refusals() {
        let shown = format!("{record_type} {field} = {value:?}");
        let record = set(base(record_type), field, value);
        assert!(
            !parser_accepts(record_type, &record),
            "the parser accepts {shown}"
        );
        if validator(record_type).is_valid(&record) {
            accepted.push(shown);
        }
    }
    assert!(
        accepted.is_empty(),
        "the schema accepts what the parser refuses: {accepted:#?}"
    );
}

/// The cases that the parser refuses and a JSON Schema cannot: each holds
/// the record type, the record, the JSON pointer of the description in the
/// record's schema that must name the rule, and a phrase it holds.
fn gaps() -> Vec<(RecordType, Value, &'static str, &'static str)> {
    let wide = "é";
    vec![
        (
            RecordType::Unit,
            set(unit(), "/name", Some(json!("Api"))),
            "/properties/name/description",
            "the name holds no uppercase letter",
        ),
        (
            RecordType::Need,
            set(
                set(need(), "/optional", Some(json!(true))),
                "/when_unbound",
                Some(json!("fail")),
            ),
            "/properties/when_unbound/description",
            "an optional need never fails when unbound",
        ),
        (
            RecordType::Need,
            set(
                set(need(), "/optional", Some(json!(false))),
                "/when_unbound",
                Some(json!("omit")),
            ),
            "/properties/when_unbound/description",
            "a required need is never omitted",
        ),
        (
            RecordType::Need,
            set(need(), "/declared_in/file", Some(json!(wide.repeat(600)))),
            "/properties/declared_in/properties/file/description",
            "at most 1024 bytes",
        ),
        (
            RecordType::Need,
            set(
                need(),
                "/delivers_as",
                Some(json!({"file": wide.repeat(600)})),
            ),
            "/properties/delivers_as/oneOf/1/properties/file/description",
            "at most 1024 bytes",
        ),
        (
            RecordType::Need,
            set(
                need(),
                "/when_unbound",
                Some(json!({"fallback": wide.repeat(2100)})),
            ),
            "/properties/when_unbound/oneOf/2/properties/fallback/description",
            "at most 4096 bytes",
        ),
        (
            RecordType::Offer,
            set(
                offer(),
                "/provider",
                Some(json!({"kind": "secret-store", "name": "vault"})),
            ),
            "/properties/reference/description",
            "the reference fits the offer",
        ),
        (
            RecordType::Offer,
            set(
                port_offer(),
                "/reference",
                Some(json!({"port": {"app": "ledger", "port": "http"}})),
            ),
            "/properties/reference/description",
            "the reference fits the offer",
        ),
    ]
}

/// Returns the description at `pointer` in `schema`, with each run of
/// whitespace written as one space.
fn description(schema: &Value, pointer: &str) -> String {
    let text = schema
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("the schema has no description at {pointer}"));
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn every_rule_a_schema_cannot_express_is_named_at_its_field_in_the_schema() {
    for (record_type, record, pointer, phrase) in gaps() {
        assert!(!parser_accepts(record_type, &record), "{record}");
        assert!(
            validator(record_type).is_valid(&record),
            "the schema now refuses a gap case; move it to the refusals: {record}"
        );
        let text = description(&schema(record_type).to_value(), pointer);
        assert!(
            text.contains(phrase),
            "the {record_type} schema does not name the rule {phrase:?} at {pointer}: {text}"
        );
    }
}
