use serde_json::json;

use super::*;
use crate::json::FromJson as _;
use crate::keyword::NeedKind;
use crate::record::Record as _;

fn need() -> NeedPath {
    "/jumpgate/needs/shelf/api-token".parse().unwrap()
}

fn consumers() -> crate::Distinct<UnitName, 1> {
    crate::Distinct::new(vec!["web".parse().unwrap()]).unwrap()
}

fn offer() -> OfferPath {
    "/jumpgate/offers/secret-store/one-password/shelf"
        .parse()
        .unwrap()
}

fn unbound(when_unbound: WhenUnbound, reason: UnboundReason) -> Unbound {
    Unbound::new(need(), NeedKind::Value, consumers(), when_unbound, reason)
}

#[test]
fn an_unbound_entry_names_the_rule_of_a_fallback_and_never_its_value() {
    let entry = unbound(
        WhenUnbound::Fallback("local-only-literal".parse().unwrap()),
        UnboundReason::FallbackOnCloud,
    );
    let written = serde_json::to_value(&entry).unwrap();
    assert_eq!(written["when_unbound"], "fallback");
    assert!(
        !written.to_string().contains("local-only-literal"),
        "{written}"
    );
    assert_eq!(entry.when_unbound(), UnboundAction::Fallback);
}

#[test]
fn only_an_omitted_need_that_nothing_offers_does_not_block() {
    assert!(!unbound(WhenUnbound::Omit, UnboundReason::NoOffer).blocks());
    assert!(unbound(WhenUnbound::Fail, UnboundReason::NoOffer).blocks());
}

#[test]
fn an_ambiguous_match_a_missing_offer_and_a_cloud_fallback_block_even_when_the_need_is_optional() {
    for reason in [
        UnboundReason::Ambiguous(vec![offer()]),
        UnboundReason::OfferMissing(offer()),
        UnboundReason::FallbackOnCloud,
    ] {
        assert!(unbound(WhenUnbound::Omit, reason).blocks());
    }
}

#[test]
fn a_need_for_a_local_service_on_a_cloud_target_reads_as_local_only_and_blocks_unless_it_is_omitted()
 {
    assert_eq!(
        serde_json::to_value(UnboundReason::LocalOnly).unwrap(),
        json!("local_only")
    );
    assert_eq!(
        UnboundReason::from_json(&json!("local_only")).unwrap(),
        UnboundReason::LocalOnly
    );
    assert!(unbound(WhenUnbound::Fail, UnboundReason::LocalOnly).blocks());
    assert!(!unbound(WhenUnbound::Omit, UnboundReason::LocalOnly).blocks());
}

#[test]
fn a_binding_to_a_fallback_has_no_offer_field() {
    let delivers =
        DeliversAs::Variables(crate::Distinct::new(vec!["API_TOKEN".parse().unwrap()]).unwrap());
    let binding = PlanBinding::new(
        need(),
        NeedKind::Secret,
        consumers(),
        None,
        Via::Fallback,
        delivers,
        Supplies::Fallback("placeholder".parse().unwrap()),
    );
    let value = serde_json::to_value(&binding).unwrap();
    assert_eq!(
        value,
        json!({
            "need": "/jumpgate/needs/shelf/api-token",
            "kind": "secret",
            "consumers": ["web"],
            "via": "fallback",
            "delivers_as": {"variables": ["API_TOKEN"]},
            "supplies": {"fallback": "placeholder"}
        })
    );
}

#[test]
fn a_binding_to_an_offer_names_it_and_carries_the_binding_record_fields() {
    let delivers =
        DeliversAs::Variables(crate::Distinct::new(vec!["API_TOKEN".parse().unwrap()]).unwrap());
    let reference = Reference::Secret("op://shelf-dev/api-token/credential".parse().unwrap());
    let binding = PlanBinding::new(
        need(),
        NeedKind::Secret,
        consumers(),
        Some(offer()),
        Via::Binding,
        delivers,
        Supplies::Reference(reference),
    )
    .with_binding_fields(Some("public-key".parse().unwrap()), None);
    let value = serde_json::to_value(&binding).unwrap();
    assert_eq!(
        value["offer"],
        "/jumpgate/offers/secret-store/one-password/shelf"
    );
    assert_eq!(value["via"], "binding");
    assert_eq!(value["projection"], "public-key");
    assert_eq!(value.get("audience"), None);
    assert_eq!(
        value["supplies"],
        json!({"reference": {"secret": "op://shelf-dev/api-token/credential"}})
    );
}

#[test]
fn an_unbound_reason_names_its_offers_in_json() {
    let value = serde_json::to_value(UnboundReason::Ambiguous(vec![offer()])).unwrap();
    assert_eq!(
        value,
        json!({"ambiguous": ["/jumpgate/offers/secret-store/one-password/shelf"]})
    );
    assert_eq!(
        serde_json::to_value(UnboundReason::FallbackOnCloud).unwrap(),
        json!("fallback_on_cloud")
    );
}

#[test]
fn a_plan_prints_as_pretty_json_that_parses_back_to_the_same_value() {
    let target = crate::Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": "local",
            "kind": "local",
            "authority": "local"
        })
        .to_string(),
    )
    .unwrap();
    let plan = Plan::new(PlanTarget::of(&target), vec![], vec![]);
    let text = plan.to_json();
    assert!(text.contains('\n'));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        serde_json::to_value(&plan).unwrap()
    );
}

#[test]
fn a_plan_starts_with_its_type_and_an_empty_adapter() {
    let target = crate::Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": "local",
            "kind": "local",
            "authority": "local"
        })
        .to_string(),
    )
    .unwrap();
    let plan = Plan::new(PlanTarget::of(&target), vec![], vec![]);
    assert_eq!(
        serde_json::to_value(&plan).unwrap(),
        json!({
            "$type": PLAN_TYPE,
            "target": {"name": "local", "kind": "local"},
            "units": [],
            "unbound": [],
            "adapter": {}
        })
    );
}

/// A plan of three units that a reader of `to_json` could have been given,
/// with every kind of binding and every reason a need is unbound.
fn written_plan() -> serde_json::Value {
    json!({
        "$type": PLAN_TYPE,
        "target": {"name": "local", "kind": "local"},
        "unbound": [
            {
                "need": "/jumpgate/needs/shelf/session-key",
                "kind": "value",
                "consumers": ["web"],
                "when_unbound": "omit",
                "reason": "no_offer"
            },
            {
                "need": "/jumpgate/needs/shelf/cache",
                "kind": "value",
                "consumers": ["web"],
                "when_unbound": "fail",
                "reason": {"offer_missing": "/jumpgate/offers/app/cache/http"}
            },
            {
                "need": "/jumpgate/needs/shelf/queue",
                "kind": "value",
                "consumers": ["web"],
                "when_unbound": "fail",
                "reason": {"ambiguous": [
                    "/jumpgate/offers/app/queue/http",
                    "/jumpgate/offers/app/queue/amqp"
                ]}
            },
            {
                "need": "/jumpgate/needs/shelf/dev-key",
                "kind": "value",
                "consumers": ["web"],
                "when_unbound": "fallback",
                "reason": "fallback_on_cloud"
            }
        ],
        "units": [
            {
                "app": "auth",
                "name": "server",
                "repo": "github.com/acme/auth",
                "kind": "process",
                "starts_after": [],
                "addresses": ["api"],
                "bindings": []
            },
            {
                "app": "shelf",
                "name": "web",
                "repo": "github.com/acme/shelf",
                "kind": "process",
                "starts_after": ["db"],
                "addresses": ["http"],
                "bindings": [
                    {
                        "need": "/jumpgate/needs/shelf/api-token",
                        "kind": "secret",
                        "consumers": ["web"],
                        "offer": "/jumpgate/offers/secret-store/one-password/api-token",
                        "via": "bind",
                        "delivers_as": {"variables": ["API_TOKEN"]},
                        "supplies": {"reference": {"secret": "op://shelf-dev/api-token/credential"}},
                        "projection": "public-key",
                        "audience": "shelf"
                    },
                    {
                        "need": "/jumpgate/needs/shelf/auth-api",
                        "kind": "app",
                        "consumers": ["web"],
                        "offer": "/jumpgate/offers/app/auth/api",
                        "via": "want",
                        "delivers_as": {"file": "config/auth.url"},
                        "supplies": {"reference": {"port": {"app": "auth", "port": "api"}}}
                    },
                    {
                        "need": "/jumpgate/needs/shelf/dev-token",
                        "kind": "value",
                        "consumers": ["web"],
                        "via": "fallback",
                        "delivers_as": {"variables": ["DEV_TOKEN"]},
                        "supplies": {"fallback": "placeholder"}
                    }
                ]
            },
            {
                "app": "shelf",
                "name": "db",
                "repo": "github.com/acme/shelf",
                "kind": "container",
                "starts_after": [],
                "addresses": [],
                "bindings": []
            }
        ],
        "adapter": {}
    })
}

#[test]
fn a_written_plan_reads_back_to_the_plan_that_wrote_it() {
    let plan = Plan::from_json(&written_plan().to_string()).unwrap();
    assert_eq!(serde_json::to_value(&plan).unwrap(), written_plan());
    assert_eq!(Plan::from_json(&plan.to_json()).unwrap(), plan);
    assert_eq!(plan.units().len(), 3);
    assert_eq!(plan.unbound().len(), 4);
}

/// The code of the refusal of `plan`, and the text of its message.
fn refusal(plan: &serde_json::Value) -> (String, String) {
    let error = Plan::from_json(&plan.to_string()).unwrap_err();
    (error.code().to_string(), error.to_string())
}

#[test]
fn a_plan_that_is_not_json_is_refused_with_the_syntax_code() {
    let error = Plan::from_json("{not json").unwrap_err();
    assert_eq!(error.code().to_string(), "jumpgate::record::syntax");
}

#[test]
fn a_plan_of_another_type_is_refused_with_the_wrong_type_code_and_its_twin_is_read() {
    let mut plan = written_plan();
    plan["$type"] = json!("jumpgate/plan/v1");
    assert_eq!(refusal(&plan).0, "jumpgate::record::wrong_type");
    plan["$type"] = json!(PLAN_TYPE);
    assert!(Plan::from_json(&plan.to_string()).is_ok());
}

#[test]
fn a_plan_with_a_field_it_does_not_have_is_refused_without_quoting_the_field() {
    let mut plan = written_plan();
    plan["s3cr3t-marker-field"] = json!("s3cr3t-marker-value");
    let (code, message) = refusal(&plan);
    assert_eq!(code, "jumpgate::record::unknown_field");
    assert!(!message.contains("s3cr3t-marker"), "{message}");
}

#[test]
fn a_plan_with_something_in_its_adapter_block_is_refused_and_an_empty_block_is_read() {
    let mut plan = written_plan();
    plan["adapter"] = json!({"platform": "s3cr3t-marker-value"});
    let (code, message) = refusal(&plan);
    assert_eq!(code, "jumpgate::record::unknown_field");
    assert!(!message.contains("s3cr3t-marker"), "{message}");
    plan["adapter"] = json!({});
    assert!(Plan::from_json(&plan.to_string()).is_ok());
}

#[test]
fn a_plan_that_lacks_a_field_is_refused_with_the_missing_field_code() {
    let mut plan = written_plan();
    plan.as_object_mut().unwrap().remove("unbound");
    let (code, message) = refusal(&plan);
    assert_eq!(code, "jumpgate::record::missing_field");
    assert!(message.contains("unbound"), "{message}");
}

#[test]
fn a_secret_reference_that_is_a_value_is_refused_and_never_quoted() {
    let mut plan = written_plan();
    plan["units"][1]["bindings"][0]["supplies"] =
        json!({"reference": {"secret": "s3cr3t-marker-value"}});
    let (code, message) = refusal(&plan);
    assert_eq!(code, "jumpgate::record::secret_reference");
    assert!(!message.contains("s3cr3t-marker"), "{message}");
    assert!(
        message.contains("units[1].bindings[0].supplies"),
        "{message}"
    );
}

#[test]
fn a_word_that_is_not_a_via_or_an_unbound_reason_is_refused_with_the_keyword_code() {
    let mut plan = written_plan();
    plan["units"][1]["bindings"][0]["via"] = json!("guess");
    assert_eq!(refusal(&plan).0, "jumpgate::record::keyword");
    let mut plan = written_plan();
    plan["unbound"][0]["reason"] = json!("guess");
    assert_eq!(refusal(&plan).0, "jumpgate::record::keyword");
    let mut plan = written_plan();
    plan["unbound"][0]["when_unbound"] = json!("guess");
    assert_eq!(refusal(&plan).0, "jumpgate::record::keyword");
}

#[test]
fn a_plan_longer_than_the_limit_is_refused_with_the_too_large_code() {
    let text = " ".repeat(MAX_PLAN_BYTES + 1);
    let error = Plan::from_json(&text).unwrap_err();
    assert_eq!(error.code().to_string(), "jumpgate::record::too_large");
}
