use jumpgate::plan::{PlanTarget, Unbound, UnboundReason};
use jumpgate::{NeedPath, OfferPath, Record as _, Target, WhenUnbound};
use serde_json::json;

use super::*;

fn target() -> Target {
    Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": "local",
            "kind": "local",
            "authority": "local"
        })
        .to_string(),
    )
    .unwrap()
}

fn need(name: &str) -> NeedPath {
    format!("/jumpgate/needs/shelf/{name}").parse().unwrap()
}

fn offer(name: &str) -> OfferPath {
    format!("/jumpgate/offers/app/auth/{name}").parse().unwrap()
}

fn plan(unbound: Vec<Unbound>) -> Plan {
    Plan::new(PlanTarget::of(&target()), vec![], unbound)
}

#[test]
fn a_blocking_need_is_a_failure_with_the_code_of_its_reason() {
    let cases = [
        (
            Unbound::new(need("a"), WhenUnbound::Fail, UnboundReason::NoOffer),
            codes::NEED_UNBOUND,
        ),
        (
            Unbound::new(
                need("a"),
                WhenUnbound::Omit,
                UnboundReason::OfferMissing(offer("api")),
            ),
            codes::NEED_OFFER_MISSING,
        ),
        (
            Unbound::new(
                need("a"),
                WhenUnbound::Fail,
                UnboundReason::Ambiguous(vec![offer("api"), offer("site")]),
            ),
            codes::NEED_AMBIGUOUS,
        ),
        (
            Unbound::new(need("a"), WhenUnbound::Omit, UnboundReason::FallbackOnCloud),
            codes::NEED_FALLBACK_ON_CLOUD,
        ),
    ];
    for (unbound, code) in cases {
        let failure = ResolveFailure::of(&unbound).unwrap();
        assert_eq!(failure.code(), code);
        assert!(failure.to_string().contains("/jumpgate/needs/shelf/a"));
    }
}

#[test]
fn an_optional_need_that_nothing_offers_is_no_failure() {
    let unbound = Unbound::new(need("a"), WhenUnbound::Omit, UnboundReason::NoOffer);
    assert!(ResolveFailure::of(&unbound).is_none());
}

#[test]
fn an_ambiguous_failure_names_every_offer() {
    let unbound = Unbound::new(
        need("a"),
        WhenUnbound::Fail,
        UnboundReason::Ambiguous(vec![offer("api"), offer("site")]),
    );
    let message = ResolveFailure::of(&unbound).unwrap().to_string();
    assert!(
        message.contains("/jumpgate/offers/app/auth/api"),
        "{message}"
    );
    assert!(
        message.contains("/jumpgate/offers/app/auth/site"),
        "{message}"
    );
}

#[test]
fn every_other_failure_has_a_declared_code_and_a_next_step() {
    let failures = [
        ResolveFailure::StoreRequired,
        ResolveFailure::TargetMissing {
            name: "local".to_owned(),
        },
    ];
    for failure in &failures {
        assert!(codes::ALL.contains(&failure.code()), "{failure}");
        assert!(matches!(failure.recovery(), Recovery::Hint(_)));
    }
}

#[test]
fn the_report_to_a_file_lists_the_problems_then_the_notes_then_the_file() {
    let report = Report::new(
        &target(),
        "records",
        Some("plan.json".to_owned()),
        plan(vec![
            Unbound::new(need("a"), WhenUnbound::Fail, UnboundReason::NoOffer),
            Unbound::new(need("b"), WhenUnbound::Omit, UnboundReason::NoOffer),
        ]),
        vec!["a note".to_owned()],
    );
    assert_eq!(
        report.to_string(),
        "Resolved target local (local) from records.\n\
         Unbound, blocking:\n  /jumpgate/needs/shelf/a: nothing offers it, and it fails when unbound\n\
         Unbound, left out:\n  /jumpgate/needs/shelf/b: nothing offers it, and it is optional\n\
         Notes:\n  a note\n\
         Bound 0 needs in 0 units.\n\
         Wrote the plan to plan.json.\n"
    );
}

#[test]
fn the_report_without_a_file_is_the_plan_alone() {
    let plan = plan(vec![]);
    let report = Report::new(&target(), "records", None, plan.clone(), vec![]);
    assert_eq!(report.to_string(), format!("{}\n", plan.to_json()));
}
