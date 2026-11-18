//! `jumpgate check`: run the six checks on a plan file, whichever matcher
//! wrote it.
//!
//! The command reads one plan file and writes nothing. It reads no store,
//! no secret store and no environment variable, and resolves no reference.

use std::fmt;
use std::path::PathBuf;

use clap::Args;
use crumbkit::{Code, Document, Failure, Prose, Recovery};
use jumpgate::RecordError;
use jumpgate::check::{CheckResult, Outcome};
use jumpgate::plan::Plan;
use jumpgate::store::{StoreError, read_plan_text};
use schemars::JsonSchema;
use serde::Serialize;

use crate::codes;

#[cfg(test)]
mod tests;

/// The document that `jumpgate check` prints.
pub(crate) const CHECK: Document<Prose<Report, CheckFailure>> = Document::new(
    "jumpgate",
    "check",
    1,
    "The result of each of the six checks on a plan, the failed checks first.",
);

/// The arguments of `jumpgate check`.
#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// The plan file to check, as `jumpgate resolve --out` wrote it, or
    /// as another matcher or a person wrote it.
    #[arg(value_name = "PLAN")]
    pub(crate) plan: PathBuf,
}

/// Whether a check passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Status {
    /// The check found nothing wrong.
    Passed,
    /// The check found at least one failure.
    Failed,
}

/// One failure of a check. It names the unit and the need it is about and
/// quotes no value from the plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
struct FailureEntry {
    /// The failure's code, `jumpgate::check::<reason>`.
    code: String,
    /// The path of the unit the failure is about, `<app>/<unit>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    unit: Option<String>,
    /// The path of the need record the failure is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    need: Option<String>,
    /// What is wrong, in one sentence.
    message: String,
}

/// The result of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
struct CheckEntry {
    /// The check's number, from 1 to 6.
    number: u8,
    /// The check's name.
    name: String,
    /// What the check requires of a plan.
    statement: String,
    /// Whether the check passed.
    status: Status,
    /// The failures of the check, empty when it passed.
    failures: Vec<FailureEntry>,
}

impl CheckEntry {
    /// Describes `result`.
    fn of(result: &CheckResult) -> Self {
        let check = result.check();
        Self {
            number: check.number(),
            name: check.name().to_owned(),
            statement: check.statement().to_owned(),
            status: if result.passed() {
                Status::Passed
            } else {
                Status::Failed
            },
            failures: result
                .failures()
                .iter()
                .map(|failure| FailureEntry {
                    code: failure.code().to_string(),
                    unit: failure.unit().map(ToString::to_string),
                    need: failure.need().map(ToString::to_string),
                    message: failure.message().to_owned(),
                })
                .collect(),
        }
    }
}

/// What `jumpgate check` found in a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub(crate) struct Report {
    /// The plan file that was checked.
    plan: String,
    /// The name of the plan's target.
    target: String,
    /// The kind of the plan's target, `local` or `cloud`.
    kind: String,
    /// Whether every check passed.
    passed: bool,
    /// The six checks, the failed ones first, each in the order of the
    /// checks.
    checks: Vec<CheckEntry>,
}

impl Report {
    /// Builds the report of `outcome`, the checks of `plan`, which was read
    /// from `file`.
    fn new(file: &str, plan: &Plan, outcome: &Outcome) -> Self {
        let mut checks: Vec<CheckEntry> = outcome.results().iter().map(CheckEntry::of).collect();
        // A stable sort keeps the checks in their own order within each
        // status.
        checks.sort_by_key(|entry| entry.status == Status::Passed);
        Self {
            plan: file.to_owned(),
            target: plan.target().name().to_string(),
            kind: plan.target().kind().as_str().to_owned(),
            passed: outcome.passed(),
            checks,
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let failed = self
            .checks
            .iter()
            .filter(|entry| entry.status == Status::Failed)
            .count();
        writeln!(
            formatter,
            "Checked {} for target {} ({}): {} of {} checks failed.",
            self.plan,
            self.target,
            self.kind,
            failed,
            self.checks.len()
        )?;
        for (heading, status) in [("Failed", Status::Failed), ("Passed", Status::Passed)] {
            let entries: Vec<&CheckEntry> = self
                .checks
                .iter()
                .filter(|entry| entry.status == status)
                .collect();
            if entries.is_empty() {
                continue;
            }
            writeln!(formatter, "{heading}:")?;
            for entry in entries {
                writeln!(
                    formatter,
                    "  {} {}: {}",
                    entry.number, entry.name, entry.statement
                )?;
                for failure in &entry.failures {
                    writeln!(formatter, "    {}: {}", failure.code, failure.message)?;
                }
            }
        }
        Ok(())
    }
}

/// Why `jumpgate check` refused or failed.
#[derive(Debug)]
pub(crate) enum CheckFailure {
    /// The plan file cannot be read.
    Unreadable(StoreError),
    /// The plan file is not a plan. The reader refused it, so no check ran.
    Refused {
        /// The plan file.
        file: String,
        /// What the reader refused.
        error: RecordError,
    },
    /// A check found something wrong in the plan.
    Found(Box<jumpgate::check::Failure>),
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(error) => formatter.write_str(error.message()),
            Self::Refused { file, error } => write!(formatter, "{file} is not a plan: {error}"),
            Self::Found(failure) => formatter.write_str(failure.message()),
        }
    }
}

impl std::error::Error for CheckFailure {}

/// The recovery of a failure whose code has no hint of its own.
const DEFAULT_HINT: &str = "fix the plan, or resolve it again, then run jumpgate check again";

/// Returns the recovery of the failure with the library code `code`.
fn hint(code: jumpgate::Code) -> &'static str {
    match (code.area(), code.name()) {
        ("check", "secret_value") => {
            "resolve the plan again with jumpgate resolve, so a binding holds a reference and a literal appears only in a fallback binding of a local target"
        }
        ("check", "need_unbound") => {
            "declare a want or a secret that meets the need, or add a binding record for this target, or make the need optional, then run jumpgate needs and resolve again"
        }
        ("check", "offer_mismatch") => {
            "resolve the plan again, or fix the binding so it names an offer that its reference fits, and a fallback binding names none"
        }
        ("check", "unknown_provider") => {
            "run jumpgate needs for the app that provides the offer, or bind the need to an offer of the secret store one-password, then resolve again"
        }
        ("check", "secret_reach") => {
            "bind the secret only in the units of the app that declares the need, then resolve again"
        }
        ("check", "cloud_fallback") => {
            "add a binding record for this target that names a real offer, because a cloud target takes no fallback value, then resolve again"
        }
        ("check", "unit_missing") => {
            "fix the depends_on of the unit in its mxv.toml, so it names a unit of the same app, then run jumpgate needs and resolve again"
        }
        ("check", "unit_repeated") => {
            "remove the repeated unit from the plan, or resolve the plan again"
        }
        ("check", "start_cycle") => {
            "remove the cycle from the depends_on entries of the units named, in their mxv.toml, then run jumpgate needs and resolve again"
        }
        ("store", _) => "name a readable plan file, such as the one jumpgate resolve --out wrote",
        ("record", _) => {
            "fix the plan at the field the message names, or resolve the plan again with jumpgate resolve"
        }
        _ => DEFAULT_HINT,
    }
}

impl Failure for CheckFailure {
    fn code(&self) -> Code<'_> {
        match self {
            Self::Unreadable(error) => codes::of_library(error.code()),
            Self::Refused { error, .. } => codes::of_library(error.code()),
            Self::Found(failure) => codes::of_library(failure.code()),
        }
    }

    fn recovery(&self) -> Recovery {
        Recovery::hint(match self {
            Self::Unreadable(error) => hint(error.code()),
            Self::Refused { error, .. } => hint(error.code()),
            Self::Found(failure) => hint(failure.code()),
        })
    }
}

/// Reads the plan file of `args`, runs the six checks on it and reports
/// each result. A plan that the reader refuses is a failure, and no check
/// runs. Every failed check is a failure of the result, so the command exits
/// with 1.
pub(crate) fn run(args: &CheckArgs) -> Result<Prose<Report, CheckFailure>, CheckFailure> {
    let file = args.plan.display().to_string();
    let text = read_plan_text(&args.plan).map_err(CheckFailure::Unreadable)?;
    let plan = Plan::from_json(&text).map_err(|error| CheckFailure::Refused {
        file: file.clone(),
        error,
    })?;
    let outcome = jumpgate::check::run(&plan);
    let failures = outcome
        .failures()
        .cloned()
        .map(|failure| CheckFailure::Found(Box::new(failure)))
        .collect();
    Ok(Prose::with_failures(
        Report::new(&file, &plan, &outcome),
        failures,
    ))
}
