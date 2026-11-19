//! The six rules, one function for each check. Each reads the plan and
//! builds failures from the names of units and needs only.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;

use crate::check::{Check, Failure, Reason, UnitRef};
use crate::consumer::Consumer;
use crate::keyword::{NeedKind, ProviderKind, TargetKind};
use crate::name::AppName;
use crate::path::ProvidePath;
use crate::plan::{
    Plan, PlanBinding, PlanCommand, PlanUnit, Supplies, Unbound, UnboundAction, UnboundReason, Via,
};
use crate::record::{MXV, ONE_PASSWORD, Reference, pair_service};

/// Returns the failures of `check` on `plan`.
pub(super) fn failures(check: Check, plan: &Plan) -> Vec<Failure> {
    match check {
        Check::SecretValue => secret_value(plan),
        Check::NeedsBound => needs_bound(plan),
        Check::KnownProviders => known_providers(plan),
        Check::SecretReach => secret_reach(plan),
        Check::NoCloudFallback => no_cloud_fallback(plan),
        Check::StartOrder => start_order(plan),
    }
}

/// Returns the unit's name for a failure.
pub(super) fn unit_ref(unit: &PlanUnit) -> UnitRef {
    UnitRef::new(unit.app().clone(), unit.name().clone())
}

/// What receives a binding: a unit or a command of the plan.
#[derive(Clone, Copy)]
pub(super) enum Holder<'a> {
    Unit(&'a PlanUnit),
    Command(&'a PlanCommand),
}

impl<'a> Holder<'a> {
    /// Returns the holder's app.
    fn app(self) -> &'a AppName {
        match self {
            Self::Unit(unit) => unit.app(),
            Self::Command(command) => command.app(),
        }
    }

    /// Reports whether `consumers` name the holder: a unit by its name and
    /// a command by its task name.
    fn is_among(self, consumers: &crate::Distinct<Consumer, 1>) -> bool {
        consumers.iter().any(|consumer| match (self, consumer) {
            (Self::Unit(unit), Consumer::Unit(name)) => unit.name() == name,
            (Self::Command(command), Consumer::Command { command: name }) => command.name() == name,
            _ => false,
        })
    }

    /// Describes the holder for a message, such as `unit shelf/web`.
    fn describe(self) -> String {
        match self {
            Self::Unit(unit) => format!("unit {}", unit_ref(unit)),
            Self::Command(command) => format!("command {}/{}", command.app(), command.name()),
        }
    }
}

/// Returns every binding of the plan with the unit or command that receives
/// it.
pub(super) fn bindings(plan: &Plan) -> impl Iterator<Item = (Holder<'_>, &PlanBinding)> {
    let in_units = plan.units().iter().flat_map(|unit| {
        unit.bindings()
            .iter()
            .map(move |binding| (Holder::Unit(unit), binding))
    });
    let in_commands = plan.commands().iter().flat_map(|command| {
        command
            .bindings()
            .iter()
            .map(move |binding| (Holder::Command(command), binding))
    });
    in_units.chain(in_commands)
}

/// Returns a failure of `reason` about `binding` in `holder`, with `what` as
/// the end of the message.
pub(super) fn about_binding(
    reason: Reason,
    holder: Holder<'_>,
    binding: &PlanBinding,
    what: &str,
) -> Failure {
    let failure = Failure::new(
        reason,
        format!("{} receives {} {what}", holder.describe(), binding.need()),
    )
    .of_need(binding.need().clone());
    match holder {
        Holder::Unit(unit) => failure.of_unit(unit_ref(unit)),
        Holder::Command(_) => failure,
    }
}

/// Reports whether `binding` supplies a fallback value.
fn is_literal(binding: &PlanBinding) -> bool {
    matches!(binding.supplies(), Supplies::Fallback(_))
}

/// Check 1. No secret need carries a literal value, whatever made the
/// binding, and no other need carries one unless a fallback made it.
fn secret_value(plan: &Plan) -> Vec<Failure> {
    bindings(plan)
        .filter_map(|(unit, binding)| {
            if !is_literal(binding) {
                return None;
            }
            let what = if binding.kind() == NeedKind::Secret {
                "as a literal value, and the need is a secret, which never has a fallback"
            } else if binding.via() != Via::Fallback {
                "as a literal value from a binding that no fallback made"
            } else {
                return None;
            };
            Some(about_binding(Reason::SecretValue, unit, binding, what))
        })
        .collect()
}

/// Check 2. A need that nothing binds must say that the plan runs without
/// it.
fn needs_bound(plan: &Plan) -> Vec<Failure> {
    plan.unbound()
        .iter()
        .filter_map(|entry| {
            let problem = match (entry.reason(), entry.when_unbound()) {
                (UnboundReason::ProvideMissing(_), _) => {
                    "is unbound, because its binding names a provide that no record holds"
                }
                (UnboundReason::Ambiguous(_), _) => {
                    "is unbound, because several provides meet it and none is chosen"
                }
                (UnboundReason::NoProvide, UnboundAction::Fail) => {
                    "is unbound, nothing provides it and it fails when unbound"
                }
                (UnboundReason::FallbackOnCloud, UnboundAction::Fail) => {
                    "is unbound, only a fallback would meet it and it fails when unbound"
                }
                (UnboundReason::KindMismatch(_), _) => {
                    "is unbound, because the provide it names is a service of another kind"
                }
                (UnboundReason::LocalOnly, UnboundAction::Fail | UnboundAction::Fallback) => {
                    "is unbound, only a local service would meet it and it fails when unbound"
                }
                (UnboundReason::NoProvide, UnboundAction::Omit | UnboundAction::Fallback)
                | (
                    UnboundReason::FallbackOnCloud | UnboundReason::LocalOnly,
                    UnboundAction::Omit,
                )
                | (UnboundReason::FallbackOnCloud, UnboundAction::Fallback) => return None,
            };
            Some(
                Failure::new(Reason::NeedUnbound, format!("{} {problem}", entry.need()))
                    .of_need(entry.need().clone()),
            )
        })
        .collect()
}

/// Reports whether the plan knows the provider of `provide`: the secret store
/// `one-password`, the workspace `mxv`, or an app that has a unit in the
/// plan.
fn known_provider(provide: &ProvidePath, apps: &BTreeSet<&AppName>) -> bool {
    match provide.provider_kind() {
        ProviderKind::SecretStore => provide.provider().as_str() == ONE_PASSWORD,
        ProviderKind::App => apps
            .iter()
            .any(|app| app.as_str() == provide.provider().as_str()),
        ProviderKind::Workspace => provide.provider().as_str() == MXV,
        ProviderKind::Capability
        | ProviderKind::CloudEnvironment
        | ProviderKind::IdentityBroker => false,
    }
}

/// Reports whether `reference` fits a provide at `provide`: a secret belongs to
/// a secret store, a port, route, unit or socket to the app that it names, and
/// a service to a workspace.
fn fits(provide: &ProvidePath, reference: &Reference) -> bool {
    let of_app = |app: &AppName| {
        provide.provider_kind() == ProviderKind::App && provide.provider().as_str() == app.as_str()
    };
    match reference {
        Reference::Secret(_) => provide.provider_kind() == ProviderKind::SecretStore,
        Reference::Port(part) => of_app(part.app()),
        Reference::Route(part) => of_app(part.app()),
        Reference::Unit(part) => of_app(part.app()),
        Reference::Socket(part) => of_app(part.app()),
        Reference::Service(_) => provide.provider_kind() == ProviderKind::Workspace,
    }
}

/// Check 3. Each binding names a provide, unless a fallback made it, and the
/// provide's provider is known and fits what the binding supplies.
fn known_providers(plan: &Plan) -> Vec<Failure> {
    let apps: BTreeSet<&AppName> = plan.units().iter().map(PlanUnit::app).collect();
    bindings(plan)
        .filter_map(|(unit, binding)| {
            let problem = match (binding.via(), binding.provide()) {
                (Via::Fallback, Some(_)) => (
                    Reason::ProvideMismatch,
                    "from a fallback, which names no provide, and the binding names one",
                ),
                (Via::Fallback, None) => match binding.supplies() {
                    Supplies::Reference(_) => (
                        Reason::ProvideMismatch,
                        "from a fallback, which supplies a value, and the binding supplies a reference",
                    ),
                    Supplies::Fallback(_) => return None,
                },
                (Via::Binding | Via::Need | Via::Bind, None) => (
                    Reason::ProvideMismatch,
                    "from a match that names a provide, and the binding names none",
                ),
                (Via::Binding | Via::Need | Via::Bind, Some(provide)) => {
                    if !known_provider(provide, &apps) {
                        (
                            Reason::UnknownProvider,
                            "from a provide whose provider the plan does not know",
                        )
                    } else if matches!(binding.supplies(), Supplies::Reference(reference) if !fits(provide, reference))
                    {
                        (
                            Reason::ProvideMismatch,
                            "from a provide that the reference it supplies does not fit",
                        )
                    } else if matches!(binding.supplies(), Supplies::Reference(reference) if pair_service(binding.delivers_as(), reference).is_err())
                    {
                        (
                            Reason::ProvideMismatch,
                            "from a service of another kind than the one the need was declared against",
                        )
                    } else {
                        return None;
                    }
                }
            };
            Some(about_binding(problem.0, unit, binding, problem.1))
        })
        .collect()
}

/// Check 4. A secret or a fallback value reaches only the units that
/// declare its need, which are units of the need's app.
fn secret_reach(plan: &Plan) -> Vec<Failure> {
    bindings(plan)
        .filter(|(unit, binding)| {
            let secret = binding.kind() == NeedKind::Secret
                || match binding.supplies() {
                    Supplies::Reference(reference) => matches!(reference, Reference::Secret(_)),
                    Supplies::Fallback(_) => true,
                };
            secret
                && (matches!(unit, Holder::Command(_))
                    || binding.need().app() != unit.app()
                    || !unit.is_among(binding.consumers()))
        })
        .map(|(unit, binding)| {
            about_binding(
                Reason::SecretReach,
                unit,
                binding,
                "as a secret, and the unit is not one that declares the need",
            )
        })
        .collect()
}

/// Check 5. A cloud plan holds no fallback, as a binding or as a need that
/// only a fallback would meet.
fn no_cloud_fallback(plan: &Plan) -> Vec<Failure> {
    if plan.target().kind() != TargetKind::Cloud {
        return Vec::new();
    }
    let bound = bindings(plan)
        .filter(|(_, binding)| binding.via() == Via::Fallback || is_literal(binding))
        .map(|(unit, binding)| {
            about_binding(
                Reason::CloudFallback,
                unit,
                binding,
                "from a fallback, and a cloud target takes no fallback",
            )
        });
    let unbound = plan
        .unbound()
        .iter()
        .filter(|entry| is_fallback_only(entry))
        .map(|entry| {
            Failure::new(
                Reason::CloudFallback,
                format!(
                    "{} would be met only by its fallback, and a cloud target takes no fallback",
                    entry.need()
                ),
            )
            .of_need(entry.need().clone())
        });
    bound.chain(unbound).collect()
}

/// Reports whether only the fallback of the unbound need `entry` would meet
/// it.
fn is_fallback_only(entry: &Unbound) -> bool {
    entry.when_unbound() == UnboundAction::Fallback
        || matches!(entry.reason(), UnboundReason::FallbackOnCloud)
}

/// Check 6. Each unit appears once, starts only after units of its own app
/// in the plan, and no start order is a cycle.
fn start_order(plan: &Plan) -> Vec<Failure> {
    // The first listing of a unit stands for it in the order.
    let mut first: BTreeMap<UnitRef, usize> = BTreeMap::new();
    let mut failures = Vec::new();
    for (index, unit) in plan.units().iter().enumerate() {
        match first.entry(unit_ref(unit)) {
            Entry::Vacant(slot) => {
                slot.insert(index);
            }
            Entry::Occupied(_) => failures.push(
                Failure::new(
                    Reason::UnitRepeated,
                    format!("the plan lists unit {} more than once", unit_ref(unit)),
                )
                .of_unit(unit_ref(unit)),
            ),
        }
    }
    let mut after: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (index, unit) in plan.units().iter().enumerate() {
        let entry = after.entry(index).or_default();
        for name in unit.starts_after() {
            match first.get(&UnitRef::new(unit.app().clone(), name.clone())) {
                Some(predecessor) => {
                    entry.insert(*predecessor);
                }
                None => failures.push(
                    Failure::new(
                        Reason::UnitMissing,
                        format!(
                            "unit {} starts after {name}, which is not a unit of its app in the plan",
                            unit_ref(unit)
                        ),
                    )
                    .of_unit(unit_ref(unit)),
                ),
            }
        }
    }
    let cyclic = cyclic(&after);
    failures.extend(
        plan.units()
            .iter()
            .enumerate()
            .filter(|(index, _)| cyclic.contains(index))
            .map(|(_, unit)| {
                Failure::new(
                    Reason::StartCycle,
                    format!("unit {} is in a cycle of the start order", unit_ref(unit)),
                )
                .of_unit(unit_ref(unit))
            }),
    );
    failures
}

/// Returns the units that sit on a cycle, given what each unit starts after.
///
/// A unit is on a cycle when it starts after itself or shares a strongly
/// connected component with another unit. The search is Tarjan's, run with
/// an explicit stack so a long chain of units cannot overflow the call
/// stack. A unit that only waits for a cycle, or sits on a path between two
/// cycles, is not named.
fn cyclic(after: &BTreeMap<usize, BTreeSet<usize>>) -> BTreeSet<usize> {
    let mut index_of: BTreeMap<usize, usize> = BTreeMap::new();
    let mut lowlink: BTreeMap<usize, usize> = BTreeMap::new();
    let mut open: Vec<usize> = Vec::new();
    let mut on_open: BTreeSet<usize> = BTreeSet::new();
    let mut found: BTreeSet<usize> = BTreeSet::new();
    let mut counter = 0_usize;
    for root in after.keys().copied() {
        if index_of.contains_key(&root) {
            continue;
        }
        // Each frame is a unit and the units it still has to visit.
        let mut frames: Vec<(usize, Vec<usize>)> = Vec::new();
        index_of.insert(root, counter);
        lowlink.insert(root, counter);
        counter += 1;
        open.push(root);
        on_open.insert(root);
        frames.push((root, successors(after, root)));
        while let Some((node, pending)) = frames.last_mut() {
            let node = *node;
            if let Some(next) = pending.pop() {
                if let Some(seen) = index_of.get(&next).copied() {
                    if on_open.contains(&next) {
                        let low = lowlink.get(&node).copied().unwrap_or(seen).min(seen);
                        lowlink.insert(node, low);
                    }
                } else {
                    index_of.insert(next, counter);
                    lowlink.insert(next, counter);
                    counter += 1;
                    open.push(next);
                    on_open.insert(next);
                    frames.push((next, successors(after, next)));
                }
                continue;
            }
            frames.pop();
            let low = lowlink.get(&node).copied().unwrap_or_default();
            if let Some((parent, _)) = frames.last() {
                let parent_low = lowlink.get(parent).copied().unwrap_or(low).min(low);
                lowlink.insert(*parent, parent_low);
            }
            if index_of.get(&node).copied() == Some(low) {
                let mut component = Vec::new();
                while let Some(member) = open.pop() {
                    on_open.remove(&member);
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                let alone = component.len() == 1;
                let loops = after
                    .get(&node)
                    .is_some_and(|before| before.contains(&node));
                if !alone || loops {
                    found.extend(component);
                }
            }
        }
    }
    found
}

/// Returns the units that `node` starts after, in the order that a stack
/// of them is visited.
fn successors(after: &BTreeMap<usize, BTreeSet<usize>>, node: usize) -> Vec<usize> {
    after
        .get(&node)
        .map(|before| before.iter().copied().collect())
        .unwrap_or_default()
}
