use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::declaration::{Checkout, CheckoutError, Declarations, Repository, read};
use crate::file_path::RepoFilePath;
use crate::name::{BoardId, TargetName};
use crate::plan::Supplies;
use crate::record::{Binding, Record as _, Reference};
use crate::repo::RepoId;

struct Files(BTreeMap<&'static str, &'static str>);

impl Checkout for Files {
    fn read(&self, path: &RepoFilePath) -> Result<Option<String>, CheckoutError> {
        Ok(self.0.get(path.as_str()).map(|text| (*text).to_owned()))
    }
}

const SHELF_MXV: &str = r#"
ports = ["http", "db"]

[containers.db]
image = "postgres:17"
ports = { 5432 = "db" }

[processes.web]
argv = ["./shelf"]
ports = { PORT = "http" }
listen = "http"
depends_on = ["db"]

[wants.auth-api]
repo = "github.com/acme/auth"
provides = "api"
as = "AUTH_API_URL"
"#;

const SHELF_STARMAP: &str = r#"
[variables.API_TOKEN]
purpose = "The token shelf sends to its upstream catalogue."
type = "string"
unset = "required"
show = "withheld"
source = "secret"
bind = { one-password = "op://shelf-dev/api-token/credential" }

[variables.SESSION_KEY]
purpose = "The key shelf signs its session cookies with."
type = "string"
unset = "optional"
show = "withheld"
source = "secret"
"#;

const AUTH_MXV: &str = r#"
ports = ["api"]
routes = ["site"]

[processes.server]
argv = ["./auth-server"]
ports = { PORT = "api" }
listen = "api"
"#;

const LEDGER_MXV: &str = r#"
[containers.store]
image = "redis:8"

[processes.worker]
argv = ["./ledger-worker"]
depends_on = ["store"]

[wants.shelf-http]
repo = "github.com/acme/shelf"
provides = "http"
as = "SHELF_URL"

[wants.billing-api]
repo = "github.com/acme/billing"
provides = "api"
as = "BILLING_URL"
optional = true
"#;

fn stack() -> Declarations {
    let checkouts: Vec<(RepoId, Files)> = [
        (
            "github.com/acme/shelf",
            vec![("mxv.toml", SHELF_MXV), ("starmap.toml", SHELF_STARMAP)],
        ),
        ("github.com/acme/auth", vec![("mxv.toml", AUTH_MXV)]),
        ("github.com/acme/ledger", vec![("mxv.toml", LEDGER_MXV)]),
    ]
    .into_iter()
    .map(|(id, files)| (id.parse().unwrap(), Files(files.into_iter().collect())))
    .collect();
    let repositories: Vec<Repository<'_>> = checkouts
        .iter()
        .map(|(id, checkout)| Repository::new(id, checkout))
        .collect();
    let declarations = read(&repositories);
    assert_eq!(declarations.refusals(), &[]);
    declarations
}

fn target(kind: &str, authority: &str) -> Target {
    Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": "local",
            "kind": kind,
            "authority": authority,
        })
        .to_string(),
    )
    .unwrap()
}

fn binding(need: &str, offer: &str) -> Binding {
    Binding::from_json(
        &json!({
            "$type": "jumpgate/binding/v1",
            "need": need,
            "offer": offer,
            "target": "local",
        })
        .to_string(),
    )
    .unwrap()
}

fn board(text: &str) -> BoardId {
    text.parse().unwrap()
}

const WANT_NEED: &str = "/jumpgate/needs/ledger/shelf-http";
const SHELF_HTTP: &str = "/jumpgate/offers/app/shelf/http";
const AUTH_API: &str = "/jumpgate/offers/app/auth/api";

fn run(declarations: &Declarations, target: &Target, bindings: &[StoredBinding]) -> Resolution {
    let input = Input::new(
        target,
        declarations.needs(),
        declarations.offers(),
        declarations.units(),
        bindings,
    );
    resolve(&DefaultMatcher, &input)
}

fn unit<'a>(resolution: &'a Resolution, app: &str, name: &str) -> &'a PlanUnit {
    resolution
        .plan()
        .units()
        .iter()
        .find(|unit| unit.app().as_str() == app && unit.name().as_str() == name)
        .unwrap()
}

fn binding_of<'a>(unit: &'a PlanUnit, need: &str) -> Option<&'a PlanBinding> {
    unit.bindings()
        .iter()
        .find(|binding| binding.need().to_string() == need)
}

fn unbound_of<'a>(resolution: &'a Resolution, need: &str) -> Option<&'a Unbound> {
    resolution
        .plan()
        .unbound()
        .iter()
        .find(|entry| entry.need().to_string() == need)
}

#[test]
fn a_want_is_bound_to_the_offer_of_the_named_repository_and_provide() {
    let resolution = run(&stack(), &target("local", "local"), &[]);
    let worker = unit(&resolution, "ledger", "worker");
    let found = binding_of(worker, WANT_NEED).unwrap();
    assert_eq!(found.via(), Via::Want);
    assert_eq!(found.offer().unwrap().to_string(), SHELF_HTTP);
    assert!(matches!(
        found.supplies(),
        Supplies::Reference(Reference::Port(_))
    ));
    assert!(unbound_of(&resolution, WANT_NEED).is_none());
}

#[test]
fn a_want_is_bound_for_every_unit_of_its_app() {
    let resolution = run(&stack(), &target("local", "local"), &[]);
    for name in ["web", "db"] {
        let found = binding_of(
            unit(&resolution, "shelf", name),
            "/jumpgate/needs/shelf/auth-api",
        );
        assert_eq!(
            found.unwrap().offer().unwrap().to_string(),
            AUTH_API,
            "{name}"
        );
    }
    assert!(
        binding_of(
            unit(&resolution, "auth", "server"),
            "/jumpgate/needs/shelf/auth-api"
        )
        .is_none()
    );
}

#[test]
fn a_secret_with_a_bind_is_bound_to_the_offer_the_bind_derives() {
    let resolution = run(&stack(), &target("local", "local"), &[]);
    let web = unit(&resolution, "shelf", "web");
    let found = binding_of(web, "/jumpgate/needs/shelf/api-token").unwrap();
    assert_eq!(found.via(), Via::Bind);
    let Supplies::Reference(Reference::Secret(reference)) = found.supplies() else {
        panic!("a secret reference is supplied");
    };
    assert_eq!(reference.as_str(), "op://shelf-dev/api-token/credential");
}

#[test]
fn an_optional_need_that_nothing_binds_is_listed_and_does_not_block() {
    let resolution = run(&stack(), &target("local", "local"), &[]);
    for need in [
        "/jumpgate/needs/ledger/billing-api",
        "/jumpgate/needs/shelf/session-key",
    ] {
        let entry = unbound_of(&resolution, need).unwrap_or_else(|| panic!("{need}"));
        assert_eq!(entry.reason(), &UnboundReason::NoOffer);
        assert_eq!(entry.when_unbound(), crate::plan::UnboundAction::Omit);
        assert!(!entry.blocks());
    }
    assert!(!resolution.blocked());
}

#[test]
fn a_required_need_that_nothing_binds_blocks_the_plan() {
    let declarations = stack();
    let local = target("local", "local");
    // Nothing offers shelf's required want when auth's offers are missing.
    let offers: Vec<Offer> = declarations
        .offers()
        .iter()
        .filter(|offer| offer.provider().name().as_str() != "auth")
        .cloned()
        .collect();
    let input = Input::new(
        &local,
        declarations.needs(),
        &offers,
        declarations.units(),
        &[],
    );
    let resolution = resolve(&DefaultMatcher, &input);
    let entry = unbound_of(&resolution, "/jumpgate/needs/shelf/auth-api").unwrap();
    assert!(entry.blocks());
    assert!(resolution.blocked());
    assert!(!run(&declarations, &local, &[]).blocked());
}

#[test]
fn the_needs_that_block_the_plan_come_first_in_its_unbound_list() {
    let declarations = stack();
    let offers: Vec<Offer> = declarations
        .offers()
        .iter()
        .filter(|offer| offer.provider().name().as_str() != "auth")
        .cloned()
        .collect();
    let target = target("local", "local");
    let input = Input::new(
        &target,
        declarations.needs(),
        &offers,
        declarations.units(),
        &[],
    );
    let resolution = resolve(&DefaultMatcher, &input);
    let blocks: Vec<bool> = resolution
        .plan()
        .unbound()
        .iter()
        .map(Unbound::blocks)
        .collect();
    assert!(
        blocks.contains(&true) && blocks.contains(&false),
        "{blocks:?}"
    );
    let mut sorted = blocks.clone();
    sorted.sort_by_key(|blocks| !blocks);
    assert_eq!(blocks, sorted);
}

#[test]
fn a_binding_in_the_authority_board_wins_over_the_want() {
    let target = target("local", "personal");
    let stored = [StoredBinding::new(
        board("personal"),
        binding(
            "/jumpgate/needs/shelf/auth-api",
            "/jumpgate/offers/app/auth/site",
        ),
    )];
    let resolution = run(&stack(), &target, &stored);
    let web = unit(&resolution, "shelf", "web");
    let found = binding_of(web, "/jumpgate/needs/shelf/auth-api").unwrap();
    assert_eq!(found.via(), Via::Binding);
    assert_eq!(
        found.offer().unwrap().to_string(),
        "/jumpgate/offers/app/auth/site"
    );
}

#[test]
fn a_binding_in_another_board_is_reported_and_ignored() {
    let target = target("local", "personal");
    let stored = [StoredBinding::new(
        board("team"),
        binding(
            "/jumpgate/needs/shelf/auth-api",
            "/jumpgate/offers/app/auth/site",
        ),
    )];
    let resolution = run(&stack(), &target, &stored);
    let web = unit(&resolution, "shelf", "web");
    let found = binding_of(web, "/jumpgate/needs/shelf/auth-api").unwrap();
    assert_eq!(found.via(), Via::Want);
    assert_eq!(found.offer().unwrap().to_string(), AUTH_API);
    assert!(resolution.notes().iter().any(|note| matches!(
        note,
        Note::IgnoredBinding { board, .. } if board.as_str() == "team"
    )));
}

#[test]
fn a_binding_for_another_target_is_not_used() {
    let target = target("local", "personal");
    let other = Binding::from_json(
        &json!({
            "$type": "jumpgate/binding/v1",
            "need": "/jumpgate/needs/shelf/auth-api",
            "offer": "/jumpgate/offers/app/auth/site",
            "target": "staging",
        })
        .to_string(),
    )
    .unwrap();
    let stored = [StoredBinding::new(board("personal"), other)];
    let resolution = run(&stack(), &target, &stored);
    let found = binding_of(
        unit(&resolution, "shelf", "web"),
        "/jumpgate/needs/shelf/auth-api",
    )
    .unwrap();
    assert_eq!(found.via(), Via::Want);
}

#[test]
fn two_authority_bindings_to_different_offers_are_ambiguous_and_bind_neither() {
    let target = target("local", "personal");
    let stored = [
        StoredBinding::new(
            board("personal"),
            binding(
                "/jumpgate/needs/shelf/auth-api",
                "/jumpgate/offers/app/auth/site",
            ),
        ),
        StoredBinding::new(
            board("personal"),
            binding("/jumpgate/needs/shelf/auth-api", AUTH_API),
        ),
    ];
    let resolution = run(&stack(), &target, &stored);
    let entry = unbound_of(&resolution, "/jumpgate/needs/shelf/auth-api").unwrap();
    let UnboundReason::Ambiguous(offers) = entry.reason() else {
        panic!("{entry:?}");
    };
    let names: Vec<String> = offers.iter().map(ToString::to_string).collect();
    assert_eq!(names, [AUTH_API, "/jumpgate/offers/app/auth/site"]);
    assert!(entry.blocks());
    assert!(
        binding_of(
            unit(&resolution, "shelf", "web"),
            "/jumpgate/needs/shelf/auth-api"
        )
        .is_none()
    );
    assert!(resolution.blocked());
}

#[test]
fn two_authority_bindings_to_the_same_offer_are_one_match() {
    let target = target("local", "personal");
    let one = binding("/jumpgate/needs/shelf/auth-api", AUTH_API);
    let stored = [
        StoredBinding::new(board("personal"), one.clone()),
        StoredBinding::new(board("personal"), one),
    ];
    let resolution = run(&stack(), &target, &stored);
    assert!(unbound_of(&resolution, "/jumpgate/needs/shelf/auth-api").is_none());
}

#[test]
fn a_binding_to_an_offer_the_store_lacks_is_unbound_and_blocks() {
    let target = target("local", "personal");
    let stored = [StoredBinding::new(
        board("personal"),
        binding(
            "/jumpgate/needs/shelf/auth-api",
            "/jumpgate/offers/app/auth/gone",
        ),
    )];
    let resolution = run(&stack(), &target, &stored);
    let entry = unbound_of(&resolution, "/jumpgate/needs/shelf/auth-api").unwrap();
    assert!(
        matches!(entry.reason(), UnboundReason::OfferMissing(path) if path.to_string().ends_with("/gone"))
    );
    assert!(entry.blocks());
}

fn need_with_fallback() -> Need {
    Need::from_json(
        &json!({
            "$type": "jumpgate/need/v1",
            "app": "shelf",
            "name": "dev_key",
            "kind": "secret",
            "consumers": ["web"],
            "declared_in": {"repo": "github.com/acme/shelf", "file": "starmap.toml"},
            "delivers_as": {"variables": ["DEV_KEY"]},
            "optional": true,
            "when": [],
            "when_unbound": {"fallback": "dev-placeholder"}
        })
        .to_string(),
    )
    .unwrap()
}

fn run_with(need: &Need, target: &Target) -> Resolution {
    let declarations = stack();
    let needs = vec![need.clone()];
    let input = Input::new(
        target,
        &needs,
        declarations.offers(),
        declarations.units(),
        &[],
    );
    resolve(&DefaultMatcher, &input)
}

#[test]
fn a_need_with_only_a_fallback_gets_it_on_a_local_target_with_a_note() {
    let need = need_with_fallback();
    let resolution = run_with(&need, &target("local", "local"));
    let web = unit(&resolution, "shelf", "web");
    let found = binding_of(web, "/jumpgate/needs/shelf/dev_key").unwrap();
    assert_eq!(found.via(), Via::Fallback);
    assert_eq!(found.offer(), None);
    assert!(
        matches!(found.supplies(), Supplies::Fallback(value) if value.as_str() == "dev-placeholder")
    );
    assert!(resolution.plan().unbound().is_empty());
    assert!(
        resolution
            .notes()
            .iter()
            .any(|note| matches!(note, Note::FallbackUsed { .. }))
    );
    assert!(!resolution.blocked());
}

#[test]
fn a_need_with_only_a_fallback_is_refused_on_a_cloud_target_and_named() {
    let need = need_with_fallback();
    let resolution = run_with(&need, &target("cloud", "local"));
    let entry = unbound_of(&resolution, "/jumpgate/needs/shelf/dev_key").unwrap();
    assert_eq!(entry.reason(), &UnboundReason::FallbackOnCloud);
    assert!(entry.blocks());
    assert!(resolution.blocked());
    assert!(unit(&resolution, "shelf", "web").bindings().is_empty());
}

#[test]
fn a_need_with_a_fallback_that_an_offer_meets_is_bound_to_the_offer_on_a_cloud_target() {
    let declarations = stack();
    let need = declarations
        .needs()
        .iter()
        .find(|need| need.name().as_str() == "api-token")
        .unwrap()
        .clone()
        .with_unbound(true, WhenUnbound::Fallback("placeholder".parse().unwrap()))
        .unwrap();
    let resolution = run_with(&need, &target("cloud", "local"));
    let found = binding_of(
        unit(&resolution, "shelf", "web"),
        "/jumpgate/needs/shelf/api-token",
    )
    .unwrap();
    assert_eq!(found.via(), Via::Bind);
    assert!(resolution.plan().unbound().is_empty());
}

fn need_when(conditions: serde_json::Value) -> Need {
    Need::from_json(
        &json!({
            "$type": "jumpgate/need/v1",
            "app": "shelf",
            "name": "extra",
            "kind": "secret",
            "consumers": ["web"],
            "declared_in": {"repo": "github.com/acme/shelf", "file": "starmap.toml"},
            "delivers_as": {"variables": ["EXTRA"]},
            "optional": false,
            "when": conditions,
            "when_unbound": "fail"
        })
        .to_string(),
    )
    .unwrap()
}

#[test]
fn a_need_that_names_the_target_applies_and_one_that_names_another_does_not() {
    let target = target("local", "local");
    let applies = run_with(&need_when(json!([{"target": "local"}])), &target);
    assert!(unbound_of(&applies, "/jumpgate/needs/shelf/extra").is_some());
    let other = run_with(&need_when(json!([{"target": "staging"}])), &target);
    assert!(unbound_of(&other, "/jumpgate/needs/shelf/extra").is_none());
    assert!(
        other
            .notes()
            .iter()
            .any(|note| matches!(note, Note::NotForTarget { .. }))
    );
    let profile = run_with(&need_when(json!([{"profile": "dev"}])), &target);
    assert!(unbound_of(&profile, "/jumpgate/needs/shelf/extra").is_none());
    let always = run_with(&need_when(json!([])), &target);
    assert!(unbound_of(&always, "/jumpgate/needs/shelf/extra").is_some());
}

#[test]
fn a_consumer_with_no_unit_record_is_noted_and_the_need_is_still_bound_for_the_others() {
    let declarations = stack();
    let units: Vec<Unit> = declarations
        .units()
        .iter()
        .filter(|unit| unit.name().as_str() != "db")
        .cloned()
        .collect();
    let target = target("local", "local");
    let input = Input::new(
        &target,
        declarations.needs(),
        declarations.offers(),
        &units,
        &[],
    );
    let resolution = resolve(&DefaultMatcher, &input);
    assert!(resolution.notes().iter().any(|note| matches!(
        note,
        Note::UnknownConsumer { unit, .. } if unit.as_str() == "db"
    )));
    assert!(
        binding_of(
            unit(&resolution, "shelf", "web"),
            "/jumpgate/needs/shelf/auth-api"
        )
        .is_some()
    );
}

#[test]
fn every_unit_of_the_store_is_a_unit_of_the_plan_in_the_order_of_app_and_name() {
    let resolution = run(&stack(), &target("local", "local"), &[]);
    let names: Vec<String> = resolution
        .plan()
        .units()
        .iter()
        .map(|unit| format!("{}/{}", unit.app(), unit.name()))
        .collect();
    assert_eq!(
        names,
        [
            "auth/server",
            "ledger/store",
            "ledger/worker",
            "shelf/db",
            "shelf/web"
        ]
    );
    let web = unit(&resolution, "shelf", "web");
    assert_eq!(web.starts_after().len(), 1);
    assert_eq!(web.addresses().len(), 1);
}

#[test]
fn the_plan_is_for_the_target_and_has_an_empty_adapter_block() {
    let resolution = run(&stack(), &target("local", "local"), &[]);
    assert_eq!(
        resolution.plan().target().name(),
        &"local".parse::<TargetName>().unwrap()
    );
    let text = serde_json::to_value(resolution.plan()).unwrap();
    assert_eq!(text["adapter"], json!({}));
    assert_eq!(text["$type"], "jumpgate/plan/v1alpha1");
}

/// A matcher that binds every need to one offer.
struct Everything(OfferPath);

impl Matcher for Everything {
    fn decide(&self, _input: &Input<'_>, _need: &Need) -> Decision {
        Decision::Bound(Choice::new(self.0.clone(), Via::Want))
    }
}

#[test]
fn another_matcher_can_replace_the_default_through_the_trait() {
    let declarations = stack();
    let target = target("local", "local");
    let input = Input::new(
        &target,
        declarations.needs(),
        declarations.offers(),
        declarations.units(),
        &[],
    );
    let matcher = Everything(AUTH_API.parse().unwrap());
    let resolution = resolve(&matcher, &input);
    assert!(resolution.plan().unbound().is_empty());
    let found = binding_of(
        unit(&resolution, "shelf", "web"),
        "/jumpgate/needs/shelf/api-token",
    )
    .unwrap();
    assert_eq!(found.offer().unwrap().to_string(), AUTH_API);
}

/// A matcher that gives every need the fallback value `leaked`.
struct AlwaysFallback;

impl Matcher for AlwaysFallback {
    fn decide(&self, _input: &Input<'_>, _need: &Need) -> Decision {
        Decision::Fallback("leaked".parse().unwrap())
    }
}

#[test]
fn a_fallback_from_any_matcher_is_refused_on_a_cloud_target() {
    let declarations = stack();
    let target = target("cloud", "local");
    let input = Input::new(
        &target,
        declarations.needs(),
        declarations.offers(),
        declarations.units(),
        &[],
    );
    let resolution = resolve(&AlwaysFallback, &input);
    assert_eq!(
        resolution.plan().unbound().len(),
        declarations.needs().len()
    );
    assert!(
        resolution
            .plan()
            .unbound()
            .iter()
            .all(|entry| entry.reason() == &UnboundReason::FallbackOnCloud && entry.blocks())
    );
    assert!(
        resolution
            .plan()
            .units()
            .iter()
            .all(|unit| unit.bindings().is_empty())
    );
    assert!(resolution.blocked());
}

#[test]
fn a_fallback_from_any_matcher_is_bound_on_a_local_target() {
    let declarations = stack();
    let target = target("local", "local");
    let input = Input::new(
        &target,
        declarations.needs(),
        declarations.offers(),
        declarations.units(),
        &[],
    );
    let resolution = resolve(&AlwaysFallback, &input);
    assert!(resolution.plan().unbound().is_empty());
}

/// Reads shelf with a want of auth's unit, and auth, each in a run of its own.
fn stack_read_in_two_runs(shelf_first: bool) -> (Vec<Need>, Vec<Offer>, Vec<Unit>) {
    let shelf_mxv: &'static str = Box::leak(
        SHELF_MXV
            .replace("provides = \"api\"", "provides = \"server\"")
            .into_boxed_str(),
    );
    let read_one = |id: &str, files: Vec<(&'static str, &'static str)>| {
        let id: RepoId = id.parse().unwrap();
        let checkout = Files(files.into_iter().collect());
        let declarations = read(&[Repository::new(&id, &checkout)]);
        assert_eq!(declarations.refusals(), &[]);
        declarations
    };
    let shelf = read_one(
        "github.com/acme/shelf",
        vec![("mxv.toml", shelf_mxv), ("starmap.toml", SHELF_STARMAP)],
    );
    let auth = read_one("github.com/acme/auth", vec![("mxv.toml", AUTH_MXV)]);
    let (first, second) = if shelf_first {
        (shelf, auth)
    } else {
        (auth, shelf)
    };
    let mut needs = first.needs().to_vec();
    needs.extend_from_slice(second.needs());
    let mut offers = first.offers().to_vec();
    offers.extend_from_slice(second.offers());
    let mut units = first.units().to_vec();
    units.extend_from_slice(second.units());
    (needs, offers, units)
}

#[test]
fn a_want_binds_whichever_order_the_repositories_were_read_in() {
    for shelf_first in [true, false] {
        let (needs, offers, units) = stack_read_in_two_runs(shelf_first);
        let target = target("local", "local");
        let input = Input::new(&target, &needs, &offers, &units, &[]);
        let resolution = resolve(&DefaultMatcher, &input);
        let found = binding_of(
            unit(&resolution, "shelf", "web"),
            "/jumpgate/needs/shelf/auth-api",
        )
        .unwrap_or_else(|| {
            panic!(
                "shelf first: {shelf_first}: {:?}",
                resolution.plan().unbound()
            )
        });
        assert_eq!(
            found.offer().unwrap().to_string(),
            "/jumpgate/offers/app/auth/server"
        );
    }
}

#[test]
fn every_note_reads_as_one_sentence_that_names_its_need() {
    let need: NeedPath = "/jumpgate/needs/shelf/api-token".parse().unwrap();
    let notes = [
        Note::FallbackUsed { need: need.clone() },
        Note::IgnoredBinding {
            need: need.clone(),
            board: "team".parse().unwrap(),
        },
        Note::UnknownConsumer {
            need: need.clone(),
            unit: "worker".parse().unwrap(),
        },
        Note::NotForTarget { need },
    ];
    let texts: Vec<String> = notes.iter().map(ToString::to_string).collect();
    for text in &texts {
        assert!(
            text.starts_with("/jumpgate/needs/shelf/api-token: "),
            "{text}"
        );
        assert!(!text.contains('\n'), "{text}");
    }
    assert!(texts[1].contains("team"), "{}", texts[1]);
    assert!(texts[2].contains("worker"), "{}", texts[2]);
}

#[test]
fn a_matcher_that_chooses_a_missing_offer_leaves_the_need_unbound() {
    let declarations = stack();
    let target = target("local", "local");
    let input = Input::new(
        &target,
        declarations.needs(),
        declarations.offers(),
        declarations.units(),
        &[],
    );
    let matcher = Everything("/jumpgate/offers/app/auth/gone".parse().unwrap());
    let resolution = resolve(&matcher, &input);
    assert_eq!(
        resolution.plan().unbound().len(),
        declarations.needs().len()
    );
    assert!(resolution.blocked());
}

const BEACON_MXV: &str = include_str!("../../../../fixtures/stack/beacon/mxv.toml");
const BEACON_API_STARMAP: &str = include_str!("../../../../fixtures/stack/beacon/api/starmap.toml");
const BEACON_CLI_STARMAP: &str = include_str!("../../../../fixtures/stack/beacon/cli/starmap.toml");

fn beacon() -> Declarations {
    let checkout = Files(
        [
            ("mxv.toml", BEACON_MXV),
            ("api/starmap.toml", BEACON_API_STARMAP),
            ("cli/starmap.toml", BEACON_CLI_STARMAP),
        ]
        .into_iter()
        .collect(),
    );
    let id: RepoId = "github.com/acme/beacon".parse().unwrap();
    let declarations = read(&[Repository::new(&id, &checkout)]);
    assert_eq!(declarations.refusals(), &[]);
    declarations
}

const BEACON_STORE: &str = "/jumpgate/needs/beacon/store";
const BEACON_FILES: &str = "/jumpgate/needs/beacon/files";

#[test]
fn a_service_need_is_bound_to_its_workspace_offer_on_a_local_target() {
    let resolution = run(&beacon(), &target("local", "local"), &[]);
    assert!(!resolution.blocked());
    let api = unit(&resolution, "beacon", "api");
    for (need, offer) in [(BEACON_STORE, "store"), (BEACON_FILES, "files")] {
        let found = binding_of(api, need).unwrap();
        assert_eq!(found.via(), Via::Bind, "{need}");
        assert_eq!(
            found.offer().unwrap().to_string(),
            format!("/jumpgate/offers/workspace/mxv/{offer}")
        );
        assert!(
            matches!(found.supplies(), Supplies::Reference(Reference::Service(_))),
            "{need}"
        );
        assert!(unbound_of(&resolution, need).is_none(), "{need}");
    }
    assert!(binding_of(unit(&resolution, "beacon", "web"), BEACON_STORE).is_none());
}

#[test]
fn every_need_of_the_beacon_fixture_is_bound_on_a_local_target() {
    let resolution = run(&beacon(), &target("local", "local"), &[]);
    assert_eq!(resolution.plan().unbound(), &[]);
    let bound: usize = resolution
        .plan()
        .units()
        .iter()
        .map(|unit| unit.bindings().len())
        .sum();
    assert_eq!(bound, 4);
}

#[test]
fn a_service_need_is_unbound_as_local_only_on_a_cloud_target_and_blocks_the_plan() {
    let resolution = run(&beacon(), &target("cloud", "local"), &[]);
    assert!(resolution.blocked());
    for need in [BEACON_STORE, BEACON_FILES] {
        let entry = unbound_of(&resolution, need).unwrap();
        assert_eq!(entry.reason(), &UnboundReason::LocalOnly, "{need}");
        assert!(entry.blocks(), "{need}");
    }
    let api = unit(&resolution, "beacon", "api");
    assert!(binding_of(api, BEACON_STORE).is_none());
    assert!(
        binding_of(api, "/jumpgate/needs/beacon/api-signing-key").is_some(),
        "the secret of the same app is still bound on a cloud target"
    );
}

#[test]
fn a_binding_record_meets_a_service_need_on_a_cloud_target() {
    let target = target("cloud", "personal");
    let stored = [StoredBinding::new(
        board("personal"),
        binding(BEACON_STORE, "/jumpgate/offers/app/beacon/http"),
    )];
    let resolution = run(&beacon(), &target, &stored);
    let found = binding_of(unit(&resolution, "beacon", "api"), BEACON_STORE).unwrap();
    assert_eq!(found.via(), Via::Binding);
    assert!(unbound_of(&resolution, BEACON_STORE).is_none());
    assert_eq!(
        unbound_of(&resolution, BEACON_FILES).unwrap().reason(),
        &UnboundReason::LocalOnly
    );
}

#[test]
fn a_service_need_whose_workspace_offer_the_store_lacks_is_unbound_on_a_local_target() {
    let declarations = beacon();
    let offers: Vec<Offer> = declarations
        .offers()
        .iter()
        .filter(|offer| offer.provider().kind() != ProviderKind::Workspace)
        .cloned()
        .collect();
    let target = target("local", "local");
    let input = Input::new(
        &target,
        declarations.needs(),
        &offers,
        declarations.units(),
        &[],
    );
    let resolution = resolve(&DefaultMatcher, &input);
    assert_eq!(
        unbound_of(&resolution, BEACON_STORE).unwrap().reason(),
        &UnboundReason::NoOffer
    );
}
