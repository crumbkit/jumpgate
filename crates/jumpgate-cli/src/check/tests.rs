#![expect(
    clippy::disallowed_methods,
    reason = "the tests write plan files into their own temporary directory"
)]

use serde_json::{Value, json};

use super::*;
use jumpgate::store::{Reason as StoreReason, StoreError};

/// A recognisable value that a test carries through a plan's fallback.
const MARKER: &str = "s3cr3t-marker-value";

/// A local plan of one unit that receives a secret reference and passes
/// every check.
fn plan() -> Value {
    json!({
        "$type": "jumpgate/plan/v1alpha1",
        "target": {"name": "local", "kind": "local"},
        "unbound": [],
        "units": [{
            "app": "shelf",
            "name": "web",
            "repo": "github.com/acme/shelf",
            "kind": "process",
            "starts_after": [],
            "addresses": [],
            "bindings": [{
                "need": "/jumpgate/needs/shelf/api-token",
                "kind": "secret",
                "consumers": ["web"],
                "offer": "/jumpgate/offers/secret-store/one-password/api-token",
                "via": "bind",
                "delivers_as": {"variables": ["API_TOKEN"]},
                "supplies": {"reference": {"secret": "op://shelf-dev/api-token/credential"}}
            }]
        }],
        "adapter": {}
    })
}

/// Runs `jumpgate check` on a file that holds `text`.
fn run_on(text: &str) -> Result<Prose<Report, CheckFailure>, CheckFailure> {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("plan.json");
    std::fs::write(&file, text).unwrap();
    run(&CheckArgs {
        plan: file,
        plan_only: true,
        store: StoreArgs {
            store: None,
            board: None,
        },
    })
}

#[test]
fn a_plan_that_passes_has_no_failure_and_lists_the_six_checks_as_passed() {
    let result = run_on(&plan().to_string()).unwrap();
    assert!(result.failures().is_empty());
    let text = result.value().to_string();
    for name in [
        "no-secret-value",
        "needs-bound",
        "known-providers",
        "secret-reach",
        "no-cloud-fallback",
        "start-order",
    ] {
        assert!(text.contains(name), "{text}");
    }
    assert!(!text.contains("Failed:"), "{text}");
    assert!(text.contains("Passed:"), "{text}");
}

/// The plan with a literal value in a binding that no fallback made, and
/// the fallback of a cloud need, which fail checks 1 and 5.
fn failing() -> Value {
    let mut plan = plan();
    plan["target"]["kind"] = json!("cloud");
    plan["units"][0]["bindings"][0]["supplies"] = json!({"fallback": MARKER});
    plan["units"][0]["bindings"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "need": "/jumpgate/needs/shelf/dev-key",
            "kind": "value",
            "consumers": ["web"],
            "via": "fallback",
            "delivers_as": {"variables": ["DEV_KEY"]},
            "supplies": {"fallback": MARKER}
        }));
    plan
}

#[test]
fn a_failing_plan_lists_the_failed_checks_before_the_passed_ones_and_each_failure_has_its_code() {
    let result = run_on(&failing().to_string()).unwrap();
    let codes: Vec<String> = result
        .failures()
        .iter()
        .map(|failure| failure.code().as_str().to_owned())
        .collect();
    assert_eq!(
        codes,
        [
            "jumpgate::check::secret_value",
            "jumpgate::check::cloud_fallback",
            "jumpgate::check::cloud_fallback"
        ]
    );
    let text = result.value().to_string();
    let failed = text.find("Failed:").unwrap();
    let passed = text.find("Passed:").unwrap();
    assert!(failed < passed, "{text}");
    assert!(text.contains("/jumpgate/needs/shelf/api-token"), "{text}");
    assert!(text.contains("shelf/web"), "{text}");
}

#[test]
fn neither_the_report_nor_a_failure_shows_a_value_of_the_plan() {
    let result = run_on(&failing().to_string()).unwrap();
    assert!(!result.value().to_string().contains(MARKER));
    for failure in result.failures() {
        assert!(!failure.to_string().contains(MARKER));
        assert!(!format!("{failure:?}").contains(MARKER));
    }
    let json = serde_json::to_string(result.value()).unwrap();
    assert!(!json.contains(MARKER), "{json}");
}

#[test]
fn a_plan_that_does_not_parse_is_refused_with_its_record_code_and_runs_no_check() {
    let mut text = plan();
    text["$type"] = json!("jumpgate/plan/v1");
    let refused = run_on(&text.to_string()).unwrap_err();
    assert_eq!(refused.code(), codes::RECORD_WRONG_TYPE);
    let refused = run_on("{not json").unwrap_err();
    assert_eq!(refused.code(), codes::RECORD_SYNTAX);
    assert!(run_on(&plan().to_string()).is_ok());
}

#[test]
fn a_refused_plan_never_quotes_its_values() {
    let mut text = plan();
    text["units"][0]["bindings"][0]["supplies"] = json!({"reference": {"secret": MARKER}});
    let refused = run_on(&text.to_string()).unwrap_err();
    assert_eq!(refused.code(), codes::RECORD_SECRET_REFERENCE);
    assert!(!refused.to_string().contains(MARKER));
}

#[test]
fn a_plan_file_that_is_missing_is_an_io_failure_that_names_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("missing.json");
    let refused = run(&CheckArgs {
        plan: file,
        plan_only: true,
        store: StoreArgs {
            store: None,
            board: None,
        },
    })
    .unwrap_err();
    assert_eq!(refused.code(), codes::STORE_IO);
    assert!(refused.to_string().contains("missing.json"));
}

#[test]
fn every_reason_of_a_check_has_a_recovery_of_its_own() {
    for reason in jumpgate::check::Reason::ALL {
        let hint = hint(reason.code());
        assert_ne!(hint, DEFAULT_HINT, "{}", reason.code());
    }
}

/// Returns the text of the hint that `failure` offers.
fn hint_of(failure: &CheckFailure) -> String {
    let Recovery::Hint(text) = failure.recovery() else {
        panic!("{failure} offers no hint");
    };
    text
}

#[test]
fn a_failure_of_the_store_gets_a_hint_about_the_store_and_one_of_the_plan_file_does_not() {
    for reason in [
        StoreReason::Corrupt,
        StoreReason::Misplaced,
        StoreReason::BoardFile,
        StoreReason::Io,
    ] {
        let failure = CheckFailure::Store(StoreError::new(reason, "the store failed"));
        let text = hint_of(&failure);
        assert!(!text.contains("plan file"), "{reason:?}: {text}");
        assert!(text.contains("store"), "{reason:?}: {text}");
    }
    let unreadable = CheckFailure::Unreadable(StoreError::new(StoreReason::Io, "no such file"));
    assert!(hint_of(&unreadable).contains("plan file"));
}

#[test]
fn every_reason_of_a_check_maps_to_a_code_of_the_command() {
    for reason in jumpgate::check::Reason::ALL {
        let code = codes::of_library(reason.code());
        assert!(codes::ALL.contains(&code));
        assert_eq!(code.as_str(), reason.code().to_string());
    }
}
