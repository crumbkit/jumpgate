use serde_json::json;

use super::*;
use crate::record::Record as _;

fn need() -> NeedPath {
    "/jumpgate/needs/shelf/api-token".parse().unwrap()
}

fn offer() -> OfferPath {
    "/jumpgate/offers/secret-store/one-password/shelf"
        .parse()
        .unwrap()
}

fn unbound(when_unbound: WhenUnbound, reason: UnboundReason) -> Unbound {
    Unbound::new(need(), when_unbound, reason)
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
fn a_binding_to_a_fallback_has_no_offer_field() {
    let delivers =
        DeliversAs::Variables(crate::Distinct::new(vec!["API_TOKEN".parse().unwrap()]).unwrap());
    let binding = PlanBinding::new(
        need(),
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
