use serde_json::{Value, json};

use super::*;
use crate::plan::{PLAN_TYPE, Plan};

/// A recognisable value that a test carries through a plan's fallback.
const MARKER: &str = "s3cr3t-marker-value";

/// A local plan of two apps that passes every check: `auth` serves a port,
/// and `shelf` has a web process that starts after its database, receives
/// a secret reference and the address of `auth`, and leaves one optional
/// need unbound.
fn base() -> Value {
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
                        "supplies": {"reference": {"secret": "op://shelf-dev/api-token/credential"}}
                    },
                    {
                        "need": "/jumpgate/needs/shelf/auth-api",
                        "kind": "app",
                        "consumers": ["web"],
                        "offer": "/jumpgate/offers/app/auth/api",
                        "via": "want",
                        "delivers_as": {"variables": ["AUTH_API_URL"]},
                        "supplies": {"reference": {"port": {"app": "auth", "port": "api"}}}
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

/// Reads `plan`, which a test built from [`base`], as a plan.
fn read(plan: &Value) -> Plan {
    Plan::from_json(&plan.to_string()).unwrap()
}

/// Runs the checks on `plan`.
fn outcome(plan: &Value) -> Outcome {
    run(&read(plan))
}

/// The reasons of the failures of `check` on `plan`.
fn reasons(plan: &Value, check: Check) -> Vec<Reason> {
    outcome(plan)
        .results()
        .iter()
        .filter(|result| result.check() == check)
        .flat_map(|result| result.failures())
        .map(Failure::reason)
        .collect()
}

/// The checks that fail on `plan`.
fn failed(plan: &Value) -> Vec<Check> {
    outcome(plan)
        .results()
        .iter()
        .filter(|result| !result.passed())
        .map(CheckResult::check)
        .collect()
}

/// The binding at `index` of the unit at `unit` of `plan`.
fn binding(plan: &mut Value, unit: usize, index: usize) -> &mut Value {
    &mut plan["units"][unit]["bindings"][index]
}

#[test]
fn the_base_plan_passes_every_check_and_the_results_come_in_the_order_of_the_checks() {
    let outcome = outcome(&base());
    assert!(outcome.passed());
    assert_eq!(outcome.failures().count(), 0);
    let checks: Vec<Check> = outcome.results().iter().map(CheckResult::check).collect();
    assert_eq!(checks, Check::ALL);
    let numbers: Vec<u8> = Check::ALL.iter().map(|check| check.number()).collect();
    assert_eq!(numbers, [1, 2, 3, 4, 5, 6]);
}

#[test]
fn every_reason_has_a_distinct_code_under_the_check_area_and_belongs_to_one_check() {
    let mut codes: Vec<String> = Reason::ALL
        .iter()
        .map(|reason| reason.code().to_string())
        .collect();
    assert!(
        codes
            .iter()
            .all(|code| code.starts_with("jumpgate::check::"))
    );
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), Reason::ALL.len());
    for check in Check::ALL {
        assert!(
            Reason::ALL.iter().any(|reason| reason.check() == *check),
            "{check:?} has no reason"
        );
    }
}

// Check 1: the plan holds no secret value.

#[test]
fn a_literal_value_in_a_binding_that_no_fallback_made_fails_check_1_and_never_shows_the_value() {
    let mut plan = base();
    *binding(&mut plan, 1, 1) = json!({
        "need": "/jumpgate/needs/shelf/auth-api",
        "kind": "app",
        "consumers": ["web"],
        "offer": "/jumpgate/offers/app/auth/api",
        "via": "want",
        "delivers_as": {"variables": ["AUTH_API_URL"]},
        "supplies": {"fallback": MARKER}
    });
    let outcome = outcome(&plan);
    let found: Vec<&Failure> = outcome
        .failures()
        .filter(|failure| failure.reason() == Reason::SecretValue)
        .collect();
    assert_eq!(found.len(), 1);
    let failure = found[0];
    assert_eq!(failure.code().to_string(), "jumpgate::check::secret_value");
    assert_eq!(failure.unit().unwrap().to_string(), "shelf/web");
    assert_eq!(
        failure.need().unwrap().to_string(),
        "/jumpgate/needs/shelf/auth-api"
    );
    assert!(!failure.to_string().contains(MARKER));
    assert!(!format!("{outcome:?}").contains(MARKER));
    assert!(failed(&plan).contains(&Check::SecretValue));
}

#[test]
fn the_same_literal_in_a_binding_that_a_fallback_made_on_a_local_plan_passes_check_1() {
    let mut plan = base();
    *binding(&mut plan, 1, 1) = json!({
        "need": "/jumpgate/needs/shelf/auth-api",
        "kind": "app",
        "consumers": ["web"],
        "via": "fallback",
        "delivers_as": {"variables": ["AUTH_API_URL"]},
        "supplies": {"fallback": MARKER}
    });
    assert_eq!(failed(&plan), []);
}

// Check 2: every need is bound or has a stated `when_unbound`.

/// The base plan with `entry` as its one unbound need.
fn unbound(when_unbound: &str, reason: &Value) -> Value {
    let mut plan = base();
    plan["unbound"] = json!([{
        "need": "/jumpgate/needs/shelf/session-key",
        "kind": "value",
        "consumers": ["web"],
        "when_unbound": when_unbound,
        "reason": reason
    }]);
    plan
}

#[test]
fn a_need_that_fails_when_unbound_fails_check_2_and_an_omitted_one_passes() {
    let plan = unbound("fail", &json!("no_offer"));
    let found = outcome(&plan);
    let failure = found
        .failures()
        .find(|failure| failure.reason() == Reason::NeedUnbound)
        .unwrap();
    assert_eq!(failure.code().to_string(), "jumpgate::check::need_unbound");
    assert_eq!(
        failure.need().unwrap().to_string(),
        "/jumpgate/needs/shelf/session-key"
    );
    assert_eq!(failure.unit(), None);
    assert_eq!(failed(&plan), [Check::NeedsBound]);
    assert_eq!(failed(&unbound("omit", &json!("no_offer"))), []);
}

#[test]
fn a_need_with_a_stated_fallback_and_no_offer_passes_check_2_on_a_local_plan() {
    assert_eq!(failed(&unbound("fallback", &json!("no_offer"))), []);
}

#[test]
fn a_binding_to_a_missing_offer_and_an_ambiguous_match_fail_check_2_even_for_an_optional_need() {
    let offer = "/jumpgate/offers/app/auth/api";
    for reason in [
        json!({"offer_missing": offer}),
        json!({"ambiguous": [offer]}),
    ] {
        assert_eq!(
            reasons(&unbound("omit", &reason), Check::NeedsBound),
            [Reason::NeedUnbound]
        );
    }
}

// Check 3: every binding names an offer with a known provider.

#[test]
fn an_offer_of_an_app_that_has_no_unit_in_the_plan_fails_check_3_and_one_that_has_passes() {
    let mut plan = base();
    let wrong = binding(&mut plan, 1, 1);
    wrong["offer"] = json!("/jumpgate/offers/app/ghost/api");
    wrong["supplies"] = json!({"reference": {"port": {"app": "ghost", "port": "api"}}});
    assert_eq!(
        reasons(&plan, Check::KnownProviders),
        [Reason::UnknownProvider]
    );
    let right = binding(&mut plan, 1, 1);
    right["offer"] = json!("/jumpgate/offers/app/auth/api");
    right["supplies"] = json!({"reference": {"port": {"app": "auth", "port": "api"}}});
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_secret_store_other_than_one_password_and_a_provider_kind_with_no_reference_fail_check_3() {
    for offer in [
        "/jumpgate/offers/secret-store/vault/api-token",
        "/jumpgate/offers/workspace/one-password/api-token",
    ] {
        let mut plan = base();
        binding(&mut plan, 1, 0)["offer"] = json!(offer);
        assert_eq!(
            reasons(&plan, Check::KnownProviders),
            [Reason::UnknownProvider],
            "{offer}"
        );
    }
    let mut plan = base();
    binding(&mut plan, 1, 0)["offer"] =
        json!("/jumpgate/offers/secret-store/one-password/other-token");
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_reference_that_does_not_fit_its_offer_fails_check_3_and_one_that_fits_passes() {
    let mut plan = base();
    binding(&mut plan, 1, 0)["offer"] = json!("/jumpgate/offers/app/auth/api");
    assert_eq!(
        reasons(&plan, Check::KnownProviders),
        [Reason::OfferMismatch]
    );
    binding(&mut plan, 1, 0)["offer"] =
        json!("/jumpgate/offers/secret-store/one-password/api-token");
    assert_eq!(failed(&plan), []);
    let other = binding(&mut plan, 1, 1);
    other["supplies"] = json!({"reference": {"port": {"app": "shelf", "port": "http"}}});
    assert_eq!(
        reasons(&plan, Check::KnownProviders),
        [Reason::OfferMismatch]
    );
}

#[test]
fn a_binding_that_a_matcher_made_but_names_no_offer_fails_check_3_and_a_fallback_may_name_none() {
    let mut plan = base();
    binding(&mut plan, 1, 1)
        .as_object_mut()
        .unwrap()
        .remove("offer");
    assert_eq!(
        reasons(&plan, Check::KnownProviders),
        [Reason::OfferMismatch]
    );
    *binding(&mut plan, 1, 1) = json!({
        "need": "/jumpgate/needs/shelf/auth-api",
        "kind": "app",
        "consumers": ["web"],
        "via": "fallback",
        "delivers_as": {"variables": ["AUTH_API_URL"]},
        "supplies": {"fallback": MARKER}
    });
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_fallback_binding_that_names_an_offer_or_supplies_a_reference_fails_check_3() {
    let mut plan = base();
    binding(&mut plan, 1, 1)["via"] = json!("fallback");
    assert_eq!(
        reasons(&plan, Check::KnownProviders),
        [Reason::OfferMismatch]
    );
    binding(&mut plan, 1, 1)
        .as_object_mut()
        .unwrap()
        .remove("offer");
    assert_eq!(
        reasons(&plan, Check::KnownProviders),
        [Reason::OfferMismatch]
    );
}

// Check 4: a secret reaches only the units that declare it.

#[test]
fn a_secret_that_reaches_a_unit_of_another_app_fails_check_4_and_one_for_its_own_app_passes() {
    let mut plan = base();
    let moved = binding(&mut plan, 1, 0).clone();
    plan["units"][0]["bindings"] = json!([moved]);
    plan["units"][1]["bindings"] = json!([]);
    let outcome = outcome(&plan);
    let failure = outcome
        .failures()
        .find(|failure| failure.reason() == Reason::SecretReach)
        .unwrap();
    assert_eq!(failure.code().to_string(), "jumpgate::check::secret_reach");
    assert_eq!(failure.unit().unwrap().to_string(), "auth/server");
    assert_eq!(
        failure.need().unwrap().to_string(),
        "/jumpgate/needs/shelf/api-token"
    );
    assert_eq!(failed(&plan), [Check::SecretReach]);
    plan["units"][0]["bindings"][0]["need"] = json!("/jumpgate/needs/auth/api-token");
    plan["units"][0]["bindings"][0]["consumers"] = json!(["server"]);
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_fallback_value_that_reaches_a_unit_of_another_app_fails_check_4_without_showing_it() {
    let mut plan = base();
    plan["units"][0]["bindings"] = json!([{
        "need": "/jumpgate/needs/shelf/dev-key",
        "kind": "value",
        "consumers": ["web"],
        "via": "fallback",
        "delivers_as": {"variables": ["DEV_KEY"]},
        "supplies": {"fallback": MARKER}
    }]);
    let found = outcome(&plan);
    assert_eq!(reasons(&plan, Check::SecretReach), [Reason::SecretReach]);
    assert!(!format!("{found:?}").contains(MARKER));
    plan["units"][0]["bindings"][0]["need"] = json!("/jumpgate/needs/auth/dev-key");
    plan["units"][0]["bindings"][0]["consumers"] = json!(["server"]);
    assert_eq!(failed(&plan), []);
}

#[test]
fn an_address_that_a_unit_of_another_app_receives_is_not_a_secret_and_passes_check_4() {
    let mut plan = base();
    let address = binding(&mut plan, 1, 1).clone();
    plan["units"][0]["bindings"] = json!([address]);
    assert_eq!(failed(&plan), []);
}

// Check 5: a plan for a cloud target has no need met by a fallback.

/// The base plan for a target of `kind`, whose web unit receives the
/// fallback of the need `auth-api`.
fn fallback_plan(kind: &str) -> Value {
    let mut plan = base();
    plan["target"] = json!({"name": "staging", "kind": kind});
    *binding(&mut plan, 1, 1) = json!({
        "need": "/jumpgate/needs/shelf/auth-api",
        "kind": "app",
        "consumers": ["web"],
        "via": "fallback",
        "delivers_as": {"variables": ["AUTH_API_URL"]},
        "supplies": {"fallback": MARKER}
    });
    plan
}

#[test]
fn a_fallback_binding_in_a_cloud_plan_fails_check_5_and_the_same_plan_for_a_local_target_passes() {
    let plan = fallback_plan("cloud");
    let found = outcome(&plan);
    let failure = found
        .failures()
        .find(|failure| failure.reason() == Reason::CloudFallback)
        .unwrap();
    assert_eq!(
        failure.code().to_string(),
        "jumpgate::check::cloud_fallback"
    );
    assert_eq!(failure.unit().unwrap().to_string(), "shelf/web");
    assert_eq!(
        failure.need().unwrap().to_string(),
        "/jumpgate/needs/shelf/auth-api"
    );
    assert_eq!(failed(&plan), [Check::NoCloudFallback]);
    assert!(!format!("{found:?}").contains(MARKER));
    assert_eq!(failed(&fallback_plan("local")), []);
}

#[test]
fn a_need_that_only_a_fallback_would_meet_fails_check_5_on_a_cloud_plan_and_not_check_2() {
    for (when_unbound, reason) in [
        ("fallback", json!("fallback_on_cloud")),
        ("fallback", json!("no_offer")),
    ] {
        let mut plan = unbound(when_unbound, &reason);
        plan["target"]["kind"] = json!("cloud");
        assert_eq!(failed(&plan), [Check::NoCloudFallback], "{reason}");
        assert_eq!(
            reasons(&plan, Check::NoCloudFallback),
            [Reason::CloudFallback]
        );
    }
    let mut plan = unbound("omit", &json!("no_offer"));
    plan["target"]["kind"] = json!("cloud");
    assert_eq!(failed(&plan), []);
}

// Check 6: every unit a unit starts after exists, and the order has no cycle.

#[test]
fn a_unit_that_starts_after_a_unit_that_does_not_exist_fails_check_6_and_one_that_exists_passes() {
    let mut plan = base();
    plan["units"][1]["starts_after"] = json!(["ghost"]);
    let found = outcome(&plan);
    let failure = found
        .failures()
        .find(|failure| failure.reason() == Reason::UnitMissing)
        .unwrap();
    assert_eq!(failure.code().to_string(), "jumpgate::check::unit_missing");
    assert_eq!(failure.unit().unwrap().to_string(), "shelf/web");
    assert!(failure.to_string().contains("ghost"));
    assert_eq!(failed(&plan), [Check::StartOrder]);
    plan["units"][1]["starts_after"] = json!(["db"]);
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_unit_starts_only_after_units_of_its_own_app() {
    let mut plan = base();
    plan["units"][0]["starts_after"] = json!(["db"]);
    assert_eq!(reasons(&plan, Check::StartOrder), [Reason::UnitMissing]);
}

#[test]
fn two_units_that_start_after_each_other_fail_check_6_and_a_unit_after_them_is_not_named() {
    let mut plan = base();
    plan["units"][2]["starts_after"] = json!(["web"]);
    plan["units"][0]["starts_after"] = json!([]);
    let mut extra = plan["units"][1].clone();
    extra["name"] = json!("worker");
    extra["bindings"] = json!([]);
    extra["starts_after"] = json!(["web"]);
    plan["units"].as_array_mut().unwrap().push(extra);
    let found = outcome(&plan);
    let named: Vec<String> = found
        .failures()
        .filter(|failure| failure.reason() == Reason::StartCycle)
        .map(|failure| failure.unit().unwrap().to_string())
        .collect();
    assert_eq!(named, ["shelf/web", "shelf/db"]);
    assert_eq!(
        found.failures().next().unwrap().code().to_string(),
        "jumpgate::check::start_cycle"
    );
    plan["units"][2]["starts_after"] = json!([]);
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_unit_that_starts_after_itself_fails_check_6() {
    let mut plan = base();
    plan["units"][2]["starts_after"] = json!(["db"]);
    assert_eq!(reasons(&plan, Check::StartOrder), [Reason::StartCycle]);
    plan["units"][2]["starts_after"] = json!([]);
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_unit_listed_twice_fails_check_6_and_listed_once_passes() {
    let mut plan = base();
    let copy = plan["units"][2].clone();
    plan["units"].as_array_mut().unwrap().push(copy);
    assert_eq!(reasons(&plan, Check::StartOrder), [Reason::UnitRepeated]);
    plan["units"].as_array_mut().unwrap().pop();
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_plan_with_no_units_and_nothing_unbound_passes() {
    let mut plan = base();
    plan["units"] = json!([]);
    plan["unbound"] = json!([]);
    assert_eq!(failed(&plan), []);
}

// Closing the approximations: the plan states each need's kind and consumers.

/// The base plan with a local fallback binding of the need `dev-key` of
/// kind `kind` in `shelf/web`.
fn dev_key(kind: &str) -> Value {
    let mut plan = base();
    plan["units"][1]["bindings"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "need": "/jumpgate/needs/shelf/dev-key",
            "kind": kind,
            "consumers": ["web"],
            "via": "fallback",
            "delivers_as": {"variables": ["DEV_KEY"]},
            "supplies": {"fallback": MARKER}
        }));
    plan
}

#[test]
fn a_fallback_value_on_a_secret_need_fails_check_1_on_a_local_plan_and_on_a_setting_passes() {
    let secret = dev_key("secret");
    assert_eq!(reasons(&secret, Check::SecretValue), [Reason::SecretValue]);
    assert_eq!(failed(&secret), [Check::SecretValue]);
    assert!(!format!("{:?}", outcome(&secret)).contains(MARKER));
    assert_eq!(failed(&dev_key("value")), []);
}

#[test]
fn a_secret_reference_delivered_to_a_unit_that_does_not_declare_the_need_fails_check_4() {
    let mut plan = base();
    // `shelf/db` is a unit of the declaring app that is not a consumer.
    let secret = plan["units"][1]["bindings"][0].clone();
    plan["units"][2]["bindings"] = json!([secret]);
    assert_eq!(reasons(&plan, Check::SecretReach), [Reason::SecretReach]);
    assert_eq!(failed(&plan), [Check::SecretReach]);
    plan["units"][2]["bindings"][0]["consumers"] = json!(["web", "db"]);
    plan["units"][1]["bindings"][0]["consumers"] = json!(["web", "db"]);
    assert_eq!(failed(&plan), []);
}

#[test]
fn a_need_that_fails_when_unbound_fails_check_2_for_a_fallback_on_cloud_on_a_local_plan() {
    let plan = unbound("fail", &json!("fallback_on_cloud"));
    assert_eq!(reasons(&plan, Check::NeedsBound), [Reason::NeedUnbound]);
    assert_eq!(failed(&plan), [Check::NeedsBound]);
    let twin = unbound("omit", &json!("fallback_on_cloud"));
    assert_eq!(failed(&twin), []);
}

#[test]
fn a_unit_between_two_cycles_is_not_named_by_check_6() {
    let mut plan = base();
    let template = plan["units"][2].clone();
    let after = |name: &str, starts_after: &[&str]| {
        let mut unit = template.clone();
        unit["name"] = json!(name);
        unit["starts_after"] = json!(starts_after);
        unit
    };
    plan["units"] = json!([
        after("a", &["b"]),
        after("b", &["a"]),
        after("c", &["b"]),
        after("d", &["c", "e"]),
        after("e", &["d"]),
    ]);
    plan["units"][2]["app"] = json!("shelf");
    plan["unbound"] = json!([]);
    let named: Vec<String> = outcome(&plan)
        .failures()
        .filter(|failure| failure.reason() == Reason::StartCycle)
        .map(|failure| failure.unit().unwrap().to_string())
        .collect();
    assert_eq!(named, ["shelf/a", "shelf/b", "shelf/d", "shelf/e"]);
    plan["units"][4]["starts_after"] = json!([]);
    plan["units"][1]["starts_after"] = json!([]);
    assert_eq!(failed(&plan), []);
}
