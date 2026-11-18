//! `jumpgate resolve`: match the needs in a store to its provides for one
//! target and write the plan.

use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;

use clap::Args;
use crumbkit::{Code, Document, Failure, Prose, Recovery};
use jumpgate::matcher::{DefaultMatcher, Input, resolve};
use jumpgate::plan::{Plan, Unbound, UnboundReason};
use jumpgate::store::{StoreError, write_plan_file};
use jumpgate::{Target, TargetName};
use schemars::JsonSchema;
use serde::Serialize;

use crate::codes;
use crate::store_arg::StoreArgs;

#[cfg(test)]
mod tests;

/// The document that `jumpgate resolve` prints.
pub(crate) const RESOLVE: Document<Prose<Report, ResolveFailure>> = Document::new(
    "jumpgate",
    "resolve",
    1,
    "The plan for one target, the needs left unbound and what the matcher noted.",
);

/// The arguments of `jumpgate resolve`.
#[derive(Debug, Args)]
pub(crate) struct ResolveArgs {
    /// The name of the target to resolve for, which the store holds as a
    /// `jumpgate/target/v1` record.
    #[arg(long, value_name = "TARGET")]
    pub(crate) target: TargetName,

    /// The store that holds the records, as `jumpgate needs` wrote them.
    #[command(flatten)]
    pub(crate) store: StoreArgs,

    /// The file to write the plan to, through a temporary file and a rename.
    /// Without it, the command prints the plan as its output.
    #[arg(long, value_name = "FILE")]
    pub(crate) out: Option<PathBuf>,
}

/// What `jumpgate resolve` matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub(crate) struct Report {
    /// The name of the target.
    target: String,
    /// The kind of the target, `local` or `cloud`.
    kind: String,
    /// Where the records were read from: a directory, or a board.
    store: String,
    /// The file the plan was written to, when `--out` named one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    out: Option<String>,
    /// What the matcher noted that is not a failure: a fallback it bound, a
    /// binding it ignored, a consumer without a unit, a need that does not
    /// apply to the target.
    notes: Vec<String>,
    /// The plan, with the needs that nothing binds listed first.
    plan: Plan,
}

impl Report {
    /// Builds the report of `plan`, the plan of `target` made from `store`.
    fn new(
        target: &Target,
        store: &str,
        out: Option<String>,
        plan: Plan,
        notes: Vec<String>,
    ) -> Self {
        Self {
            target: target.name().to_string(),
            kind: target.kind().as_str().to_owned(),
            store: store.to_owned(),
            out,
            notes,
            plan,
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.out.is_none() {
            return writeln!(formatter, "{}", self.plan.to_json());
        }
        writeln!(
            formatter,
            "Resolved target {} ({}) from {}.",
            self.target, self.kind, self.store
        )?;
        let (blocking, left_out): (Vec<&Unbound>, Vec<&Unbound>) =
            self.plan.unbound().iter().partition(|entry| entry.blocks());
        for (heading, entries) in [
            ("Unbound, blocking", blocking),
            ("Unbound, left out", left_out),
        ] {
            if !entries.is_empty() {
                writeln!(formatter, "{heading}:")?;
                for entry in entries {
                    writeln!(formatter, "  {}: {}", entry.need(), explain(entry))?;
                }
            }
        }
        if !self.notes.is_empty() {
            writeln!(formatter, "Notes:")?;
            for note in &self.notes {
                writeln!(formatter, "  {note}")?;
            }
        }
        let bound: BTreeSet<_> = self
            .plan
            .units()
            .iter()
            .flat_map(|unit| unit.bindings())
            .map(|binding| binding.need())
            .collect();
        let units = self
            .plan
            .units()
            .iter()
            .filter(|unit| !unit.bindings().is_empty())
            .count();
        writeln!(formatter, "Bound {} needs in {units} units.", bound.len())?;
        if let Some(out) = &self.out {
            writeln!(formatter, "Wrote the plan to {out}.")?;
        }
        Ok(())
    }
}

/// Returns what the matcher found for the unbound need `entry`, in one
/// clause that does not name the need.
fn explain(entry: &Unbound) -> String {
    match entry.reason() {
        UnboundReason::NoProvide if entry.blocks() => {
            "nothing provides it, and it fails when unbound".to_owned()
        }
        UnboundReason::NoProvide => "nothing provides it, and it is optional".to_owned(),
        UnboundReason::ProvideMissing(provide) => {
            format!("its binding names the provide {provide}, which the store does not hold")
        }
        UnboundReason::Ambiguous(provides) => {
            let provides: Vec<String> = provides.iter().map(ToString::to_string).collect();
            format!(
                "these provides meet it, and none is chosen: {}",
                provides.join(", ")
            )
        }
        UnboundReason::FallbackOnCloud => {
            "only its fallback value would meet it, and a cloud target takes no fallback".to_owned()
        }
    }
}

/// Why `jumpgate resolve` refused or failed.
#[derive(Debug)]
pub(crate) enum ResolveFailure {
    /// The store holds no target of the name given.
    TargetMissing {
        /// The name as typed.
        name: String,
    },
    /// The store refused a read, or the plan file could not be written.
    Store(StoreError),
    /// A need that must be bound has nothing to meet it.
    Unbound {
        /// The need's path.
        need: String,
    },
    /// A binding names a provide that the store does not hold.
    ProvideMissing {
        /// The need's path.
        need: String,
        /// The provide's path.
        provide: String,
    },
    /// Several provides meet the need.
    Ambiguous {
        /// The need's path.
        need: String,
        /// The paths of the provides that meet it.
        provides: Vec<String>,
    },
    /// Only a fallback value meets the need, and the target is cloud.
    FallbackOnCloud {
        /// The need's path.
        need: String,
    },
}

impl ResolveFailure {
    /// Returns the failure of the unbound need `entry`, or nothing when the
    /// need does not stop the plan.
    fn of(entry: &Unbound) -> Option<Self> {
        let need = entry.need().to_string();
        entry.blocks().then(|| match entry.reason() {
            UnboundReason::NoProvide => Self::Unbound { need },
            UnboundReason::ProvideMissing(provide) => Self::ProvideMissing {
                need,
                provide: provide.to_string(),
            },
            UnboundReason::Ambiguous(provides) => Self::Ambiguous {
                need,
                provides: provides.iter().map(ToString::to_string).collect(),
            },
            UnboundReason::FallbackOnCloud => Self::FallbackOnCloud { need },
        })
    }
}

impl fmt::Display for ResolveFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMissing { name } => {
                write!(formatter, "the store holds no target named {name}")
            }
            Self::Store(error) => formatter.write_str(error.message()),
            Self::Unbound { need } => {
                write!(
                    formatter,
                    "{need} is unbound: nothing provides it and it has no fallback"
                )
            }
            Self::ProvideMissing { need, provide } => write!(
                formatter,
                "{need} is bound to {provide}, which the store does not hold"
            ),
            Self::Ambiguous { need, provides } => write!(
                formatter,
                "{need} is ambiguous: {} all meet it",
                provides.join(", ")
            ),
            Self::FallbackOnCloud { need } => write!(
                formatter,
                "{need} is met only by its fallback value, and a cloud target takes no fallback"
            ),
        }
    }
}

impl std::error::Error for ResolveFailure {}

impl Failure for ResolveFailure {
    fn code(&self) -> Code<'_> {
        match self {
            Self::TargetMissing { .. } => codes::TARGET_MISSING,
            Self::Store(error) => codes::of_library(error.code()),
            Self::Unbound { .. } => codes::NEED_UNBOUND,
            Self::ProvideMissing { .. } => codes::NEED_PROVIDE_MISSING,
            Self::Ambiguous { .. } => codes::NEED_AMBIGUOUS,
            Self::FallbackOnCloud { .. } => codes::NEED_FALLBACK_ON_CLOUD,
        }
    }

    fn recovery(&self) -> Recovery {
        Recovery::hint(match self {
            Self::TargetMissing { .. } => {
                "write a jumpgate/target/v1 record at /jumpgate/targets/<name> in the store, or pass the name of one that it holds"
            }
            Self::Store(_) => {
                "fix the record or the board the message names, then run the command again"
            }
            Self::Unbound { .. } => {
                "declare a need or a secret that meets the need, or add a binding record for this target, then run jumpgate needs and resolve again"
            }
            Self::ProvideMissing { .. } => {
                "run jumpgate needs for the repository that provides the provide, or fix the binding record"
            }
            Self::Ambiguous { .. } => {
                "add a binding record for this target that names one of the provides"
            }
            Self::FallbackOnCloud { .. } => {
                "add a binding record for this target, because a cloud target takes no fallback value"
            }
        })
    }
}

/// Matches every need of the store for the target of `args` and writes the
/// plan.
///
/// The plan is written whole, to `--out` when it is given, also when a need
/// blocks it. Each blocking need is a failure of the result, so the command
/// exits with 1.
pub(crate) fn run(args: &ResolveArgs) -> Result<Prose<Report, ResolveFailure>, ResolveFailure> {
    let chosen = args.store.open().map_err(ResolveFailure::Store)?;
    let store = chosen.store();
    let targets = store.targets().map_err(ResolveFailure::Store)?;
    let target = targets
        .into_iter()
        .find(|target| target.name() == &args.target)
        .ok_or_else(|| ResolveFailure::TargetMissing {
            name: args.target.to_string(),
        })?;
    let needs = store.needs().map_err(ResolveFailure::Store)?;
    let provides = store.provides().map_err(ResolveFailure::Store)?;
    let units = store.units().map_err(ResolveFailure::Store)?;
    let bindings = store
        .bindings(target.name())
        .map_err(ResolveFailure::Store)?;
    let input = Input::new(&target, &needs, &provides, &units, &bindings);
    let resolution = resolve(&DefaultMatcher, &input);
    let out = match &args.out {
        Some(file) => {
            write_plan_file(file, resolution.plan()).map_err(ResolveFailure::Store)?;
            Some(file.display().to_string())
        }
        None => None,
    };
    let failures = resolution
        .plan()
        .unbound()
        .iter()
        .filter_map(ResolveFailure::of)
        .collect();
    let report = Report::new(
        &target,
        chosen.place(),
        out,
        resolution.plan().clone(),
        resolution.notes().iter().map(ToString::to_string).collect(),
    );
    Ok(Prose::with_failures(report, failures))
}
