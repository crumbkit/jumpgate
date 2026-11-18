//! The fixture stack of three invented apps goes from declarations to a
//! plan, and the plan agrees with its committed JSON Schema.

#![cfg(test)]

use std::collections::BTreeMap;

use jsonschema::Validator;
use jumpgate::declaration::{Checkout, CheckoutError, Declarations, Repository, read};
use jumpgate::matcher::{DefaultMatcher, Input, resolve};
use jumpgate::{Record, RepoFilePath, RepoId, Target};
use serde_json::{Value, json};

struct Files(BTreeMap<&'static str, &'static str>);

impl Checkout for Files {
    fn read(&self, path: &RepoFilePath) -> Result<Option<String>, CheckoutError> {
        Ok(self.0.get(path.as_str()).map(|text| (*text).to_owned()))
    }
}

fn declarations() -> Declarations {
    let checkouts: Vec<(RepoId, Files)> = [
        (
            "github.com/acme/shelf",
            vec![
                (
                    "mxv.toml",
                    include_str!("../../../fixtures/stack/shelf/mxv.toml"),
                ),
                (
                    "starmap.toml",
                    include_str!("../../../fixtures/stack/shelf/starmap.toml"),
                ),
            ],
        ),
        (
            "github.com/acme/auth",
            vec![(
                "mxv.toml",
                include_str!("../../../fixtures/stack/auth/mxv.toml"),
            )],
        ),
        (
            "github.com/acme/ledger",
            vec![(
                "mxv.toml",
                include_str!("../../../fixtures/stack/ledger/mxv.toml"),
            )],
        ),
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

fn plan_for(kind: &str) -> Value {
    let declarations = declarations();
    let target = Target::from_json(
        &json!({
            "$type": "jumpgate/target/v1",
            "name": "local",
            "kind": kind,
            "authority": "local",
        })
        .to_string(),
    )
    .unwrap();
    let input = Input::new(
        &target,
        declarations.needs(),
        declarations.offers(),
        declarations.units(),
        &[],
    );
    serde_json::to_value(resolve(&DefaultMatcher, &input).plan()).unwrap()
}

fn validator() -> Validator {
    let schema = serde_json::to_value(jumpgate::plan::schema()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

#[test]
fn the_plan_of_the_fixture_stack_agrees_with_the_plan_schema() {
    let validator = validator();
    for kind in ["local", "cloud"] {
        let plan = plan_for(kind);
        let errors: Vec<String> = validator
            .iter_errors(&plan)
            .map(|error| error.to_string())
            .collect();
        assert!(errors.is_empty(), "{kind}: {errors:?}");
    }
}

#[test]
fn the_plan_schema_refuses_a_field_the_plan_does_not_have() {
    let validator = validator();
    let mut plan = plan_for("local");
    assert!(validator.is_valid(&plan));
    plan["secret"] = json!("s3cr3t");
    assert!(!validator.is_valid(&plan));
    let mut plan = plan_for("local");
    plan["adapter"] = json!({"anything": 1});
    assert!(!validator.is_valid(&plan));
    let mut plan = plan_for("local");
    plan["$type"] = json!("jumpgate/plan/v1");
    assert!(!validator.is_valid(&plan));
}

#[test]
fn the_plan_of_the_fixture_stack_holds_references_and_no_secret_value() {
    let text = plan_for("local").to_string();
    assert!(
        text.contains("op://shelf-dev/api-token/credential"),
        "{text}"
    );
    assert!(text.contains("\"adapter\":{}"), "{text}");
    assert!(!text.contains("s3cr3t"), "{text}");
}

#[test]
fn the_plan_lists_the_unbound_optional_needs_for_a_local_and_a_cloud_target_alike() {
    for kind in ["local", "cloud"] {
        let plan = plan_for(kind);
        let unbound: Vec<&str> = plan["unbound"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["need"].as_str().unwrap())
            .collect();
        assert_eq!(
            unbound,
            [
                "/jumpgate/needs/ledger/billing-api",
                "/jumpgate/needs/shelf/session-key"
            ],
            "{kind}"
        );
        assert_eq!(plan["target"]["kind"], kind);
    }
}
