use serde_json::{Value, json};

use super::*;
use crate::check::tests::base;
use crate::check::{Check, CheckResult, run_against};
use crate::name::{BoardId, TargetName};
use crate::path::RecordPath;
use crate::record::{Need, Offer, Record as _, Target};
use crate::store::{AnyRecord, Reason as StoreReason, StoredBinding};

/// A store that holds records in memory and fails a read when asked.
#[derive(Default)]
struct Memory {
    board: Option<BoardId>,
    needs: Vec<Need>,
    offers: Vec<Offer>,
    targets: Vec<Target>,
    unreadable: bool,
}

impl Store for Memory {
    fn board(&self) -> &BoardId {
        self.board.as_ref().unwrap()
    }

    fn put(&self, _: &[AnyRecord]) -> Result<(), StoreError> {
        Err(StoreError::new(
            StoreReason::Board,
            "the checks write nothing",
        ))
    }

    fn remove(&self, _: &[RecordPath]) -> Result<(), StoreError> {
        Err(StoreError::new(
            StoreReason::Board,
            "the checks delete nothing",
        ))
    }

    fn get(&self, _: &RecordPath) -> Result<Option<AnyRecord>, StoreError> {
        Err(StoreError::new(
            StoreReason::Board,
            "the checks list records",
        ))
    }

    fn needs(&self) -> Result<Vec<Need>, StoreError> {
        if self.unreadable {
            return Err(StoreError::new(
                StoreReason::Board,
                "the board did not answer",
            ));
        }
        Ok(self.needs.clone())
    }

    fn offers(&self) -> Result<Vec<Offer>, StoreError> {
        Ok(self.offers.clone())
    }

    fn units(&self) -> Result<Vec<crate::record::Unit>, StoreError> {
        Err(StoreError::new(
            StoreReason::Board,
            "the checks read the units from the plan",
        ))
    }

    fn targets(&self) -> Result<Vec<Target>, StoreError> {
        Ok(self.targets.clone())
    }

    fn bindings(&self, _: &TargetName) -> Result<Vec<StoredBinding>, StoreError> {
        Err(StoreError::new(
            StoreReason::Board,
            "the checks read the bindings from the plan",
        ))
    }
}

fn need(name: &str, kind: &str, consumers: &[&str], when_unbound: Value) -> Value {
    json!({
        "$type": "jumpgate/need/v1",
        "app": "shelf",
        "name": name,
        "kind": kind,
        "consumers": consumers,
        "declared_in": {"repo": "github.com/acme/shelf", "file": "mxv.toml"},
        "delivers_as": {"variables": ["VALUE"]},
        "optional": when_unbound == json!("omit"),
        "when": [],
        "when_unbound": when_unbound
    })
}

/// The needs and offers that the base plan was resolved from.
fn base_needs() -> Vec<Value> {
    vec![
        need("session-key", "value", &["web"], json!("omit")),
        need("api-token", "secret", &["web"], json!("fail")),
        need("auth-api", "app", &["web"], json!("fail")),
    ]
}

fn base_offers() -> Vec<Value> {
    vec![
        json!({
            "$type": "jumpgate/offer/v1",
            "provider": {"kind": "secret-store", "name": "one-password"},
            "kind": "secret",
            "name": "api-token",
            "reference": {"secret": "op://shelf-dev/api-token/credential"},
            "grants": ["shelf"],
            "scope": "workspace"
        }),
        json!({
            "$type": "jumpgate/offer/v1",
            "provider": {"kind": "app", "name": "auth"},
            "kind": "address",
            "name": "api",
            "reference": {"port": {"app": "auth", "port": "api"}},
            "grants": [],
            "scope": "app-instance"
        }),
    ]
}

fn target(kind: &str) -> Value {
    json!({
        "$type": "jumpgate/target/v1",
        "name": "local",
        "kind": kind,
        "authority": "local"
    })
}

fn store(needs: &[Value], offers: &[Value], targets: &[Value]) -> Memory {
    Memory {
        board: Some("local".parse().unwrap()),
        needs: needs
            .iter()
            .map(|value| Need::from_json(&value.to_string()).unwrap())
            .collect(),
        offers: offers
            .iter()
            .map(|value| Offer::from_json(&value.to_string()).unwrap())
            .collect(),
        targets: targets
            .iter()
            .map(|value| Target::from_json(&value.to_string()).unwrap())
            .collect(),
        unreadable: false,
    }
}

fn base_store() -> Memory {
    store(&base_needs(), &base_offers(), &[target("local")])
}

fn plan(value: &Value) -> Plan {
    Plan::from_json(&value.to_string()).unwrap()
}

/// The reasons of every failure that comparing `value` with `store` adds.
fn reasons(value: &Value, store: &Memory) -> Vec<Reason> {
    run_against(&plan(value), store)
        .unwrap()
        .failures()
        .map(crate::check::Failure::reason)
        .collect()
}

#[test]
fn a_plan_that_agrees_with_its_store_passes_every_check() {
    assert_eq!(reasons(&base(), &base_store()), []);
}

#[test]
fn the_failures_of_the_plan_alone_are_kept_when_the_store_is_read_too() {
    let mut value = base();
    value["units"][1]["starts_after"] = json!(["nothing"]);
    assert_eq!(reasons(&value, &base_store()), [Reason::UnitMissing]);
}

#[test]
fn a_target_that_the_store_does_not_hold_is_refused_and_one_it_holds_is_not() {
    let empty = store(&base_needs(), &base_offers(), &[]);
    assert_eq!(reasons(&base(), &empty), [Reason::TargetUnknown]);
    assert_eq!(reasons(&base(), &base_store()), []);
}

#[test]
fn a_plan_that_calls_a_cloud_target_local_is_refused_by_the_store_kind() {
    let cloud = store(&base_needs(), &base_offers(), &[target("cloud")]);
    assert_eq!(reasons(&base(), &cloud), [Reason::TargetKind]);
    let outcome = run_against(&plan(&base()), &cloud).unwrap();
    let failing: Vec<Check> = outcome
        .results()
        .iter()
        .filter(|result| !result.passed())
        .map(CheckResult::check)
        .collect();
    assert_eq!(failing, [Check::NoCloudFallback]);
}

#[test]
fn a_plan_that_states_another_kind_for_a_need_than_the_store_holds_is_refused() {
    let mut value = base();
    value["units"][1]["bindings"][0]["kind"] = json!("value");
    assert_eq!(reasons(&value, &base_store()), [Reason::NeedKind]);
    let mut unbound = base();
    unbound["unbound"][0]["kind"] = json!("secret");
    assert_eq!(reasons(&unbound, &base_store()), [Reason::NeedKind]);
}

#[test]
fn a_plan_that_states_other_consumers_than_the_store_holds_is_refused() {
    let mut needs = base_needs();
    needs[1] = need("api-token", "secret", &["web", "db"], json!("fail"));
    let wider = store(&needs, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&base(), &wider), [Reason::NeedConsumers]);
    let mut value = base();
    value["unbound"][0]["consumers"] = json!(["web", "db"]);
    assert_eq!(reasons(&value, &base_store()), [Reason::NeedConsumers]);
}

/// Returns the messages of the failures of comparing `value` with `store`.
fn messages(value: &Value, store: &Memory) -> Vec<String> {
    run_against(&plan(value), store)
        .unwrap()
        .failures()
        .map(|failure| failure.message().to_owned())
        .collect()
}

#[test]
fn a_kind_message_names_the_kind_of_the_plan_and_the_kind_of_the_record() {
    let mut value = base();
    value["units"][1]["bindings"][0]["kind"] = json!("value");
    let found = messages(&value, &base_store());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("kind value"), "{}", found[0]);
    assert!(found[0].contains("kind secret"), "{}", found[0]);

    let mut cloud = base();
    cloud["target"]["kind"] = json!("cloud");
    let found = messages(&cloud, &base_store());
    let kind = found
        .iter()
        .find(|message| message.contains("kind cloud"))
        .unwrap_or_else(|| panic!("{found:?}"));
    assert!(kind.contains("kind local"), "{kind}");
}

#[test]
fn a_consumers_message_does_not_say_which_side_lists_more() {
    let mut needs = base_needs();
    needs[1] = need("api-token", "secret", &["web"], json!("fail"));
    let mut value = base();
    value["units"][1]["bindings"][0]["consumers"] = json!(["web", "db"]);
    let narrower = store(&needs, &base_offers(), &[target("local")]);
    let found = messages(&value, &narrower);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("consumers other than its record lists"),
        "{}",
        found[0]
    );
}

#[test]
fn consumers_in_another_order_are_the_same_consumers() {
    let mut needs = base_needs();
    needs[1] = need("api-token", "secret", &["db", "web"], json!("fail"));
    let mut value = base();
    value["units"][1]["bindings"][0]["consumers"] = json!(["web", "db"]);
    let store = store(&needs, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&value, &store), []);
}

#[test]
fn a_need_that_the_store_does_not_hold_is_refused_whether_bound_or_unbound() {
    let mut without_bound = base_needs();
    without_bound.remove(1);
    let store_bound = store(&without_bound, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&base(), &store_bound), [Reason::NeedUnknown]);
    let without_unbound = base_needs().split_off(1);
    let store_unbound = store(&without_unbound, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&base(), &store_unbound), [Reason::NeedUnknown]);
}

#[test]
fn a_plan_that_says_another_thing_when_unbound_than_the_need_does_is_refused() {
    let mut needs = base_needs();
    needs[0] = need("session-key", "value", &["web"], json!("fail"));
    let failing = store(&needs, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&base(), &failing), [Reason::UnboundDiffers]);
}

#[test]
fn a_fallback_binding_for_a_need_that_does_not_fall_back_is_refused() {
    let mut value = base();
    value["unbound"] = json!([]);
    value["units"][1]["bindings"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "need": "/jumpgate/needs/shelf/session-key",
            "kind": "value",
            "consumers": ["web"],
            "via": "fallback",
            "delivers_as": {"variables": ["VALUE"]},
            "supplies": {"fallback": "dev"}
        }));
    assert_eq!(reasons(&value, &base_store()), [Reason::UnboundDiffers]);
    let mut needs = base_needs();
    needs[0] = need("session-key", "value", &["web"], json!({"fallback": "dev"}));
    let falling = store(&needs, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&value, &falling), []);
}

#[test]
fn an_unbound_secret_need_that_falls_back_is_refused_by_the_store() {
    let mut needs = base_needs();
    needs[1] = need("api-token", "secret", &["web"], json!({"fallback": "dev"}));
    let mut value = base();
    value["units"][1]["bindings"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    value["unbound"].as_array_mut().unwrap().push(json!({
        "need": "/jumpgate/needs/shelf/api-token",
        "kind": "secret",
        "consumers": ["web"],
        "when_unbound": "fallback",
        "reason": "no_offer"
    }));
    let store = store(&needs, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&value, &store), [Reason::SecretFallback]);
    let mut omitted = base_needs();
    omitted[1] = need("api-token", "secret", &["web"], json!("omit"));
    value["unbound"][1]["when_unbound"] = json!("omit");
    let store = self::store(&omitted, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&value, &store), []);
}

#[test]
fn an_offer_that_the_store_does_not_hold_is_refused_and_one_it_holds_is_not() {
    let only_one = base_offers().split_off(1);
    let store = store(&base_needs(), &only_one, &[target("local")]);
    assert_eq!(reasons(&base(), &store), [Reason::OfferUnknown]);
    assert_eq!(reasons(&base(), &base_store()), []);
}

#[test]
fn a_reference_that_differs_from_the_stored_offers_is_refused() {
    let mut offers = base_offers();
    offers[0]["reference"] = json!({"secret": "op://shelf-dev/other-token/credential"});
    let store = store(&base_needs(), &offers, &[target("local")]);
    assert_eq!(reasons(&base(), &store), [Reason::OfferReference]);
}

#[test]
fn a_need_that_applies_to_the_target_and_is_left_out_of_the_plan_is_refused() {
    let mut needs = base_needs();
    needs.push(need("extra", "value", &["web"], json!("fail")));
    let store = store(&needs, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&base(), &store), [Reason::NeedOmitted]);
}

#[test]
fn a_need_for_another_target_or_for_a_unit_the_plan_lacks_may_be_left_out() {
    let mut other_target = need("extra", "value", &["web"], json!("fail"));
    other_target["when"] = json!([{"target": "prod"}]);
    let no_unit = need("lonely", "value", &["worker"], json!("fail"));
    let mut needs = base_needs();
    needs.extend([other_target, no_unit]);
    let store = store(&needs, &base_offers(), &[target("local")]);
    assert_eq!(reasons(&base(), &store), []);
}

#[test]
fn a_store_that_cannot_be_read_is_an_error_and_no_outcome() {
    let mut store = base_store();
    store.unreadable = true;
    let error = run_against(&plan(&base()), &store).unwrap_err();
    assert_eq!(error.reason(), StoreReason::Board);
}

#[test]
fn every_store_failure_message_names_paths_and_quotes_no_value() {
    let mut value = base();
    value["units"][1]["bindings"][0]["supplies"] =
        json!({"reference": {"secret": "op://shelf-dev/leaked-token/credential"}});
    let outcome = run_against(&plan(&value), &base_store()).unwrap();
    for failure in outcome.failures() {
        assert!(
            !failure.message().contains("op://"),
            "{}",
            failure.message()
        );
    }
    assert_eq!(outcome.failures().count(), 1);
}
