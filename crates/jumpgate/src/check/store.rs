//! What a plan states that only the store can confirm: the target's kind,
//! each need's kind, consumers and answer when unbound, and each offer's
//! reference. Every failure names units and needs by path and quotes no
//! value.

use std::collections::{BTreeMap, BTreeSet};

use crate::check::rules::{about_binding, bindings};
use crate::check::{Failure, Reason};
use crate::keyword::NeedKind;
use crate::matcher::applies;
use crate::name::UnitName;
use crate::path::{NeedPath, OfferPath};
use crate::plan::{Plan, Supplies, Unbound, UnboundAction, Via};
use crate::record::{Need, Offer, Target, WhenUnbound};
use crate::store::{Store, StoreError};

#[cfg(test)]
mod tests;

/// Returns the failures of comparing `plan` with the records of `store`.
pub(super) fn failures(
    plan: &Plan,
    store: &(impl Store + ?Sized),
) -> Result<Vec<Failure>, StoreError> {
    let needs = store.needs()?;
    let offers = store.offers()?;
    let targets = store.targets()?;
    let target = targets
        .iter()
        .find(|target| target.name() == plan.target().name());
    let needs_by_path: BTreeMap<NeedPath, &Need> =
        needs.iter().map(|need| (need.path(), need)).collect();
    let offers_by_path: BTreeMap<OfferPath, &Offer> =
        offers.iter().map(|offer| (offer.path(), offer)).collect();
    let mut found = target_failures(plan, target);
    for (unit, binding) in bindings(plan) {
        let Some(need) = needs_by_path.get(binding.need()) else {
            found.push(about_binding(
                Reason::NeedUnknown,
                unit,
                binding,
                "and the store holds no need record at that path",
            ));
            continue;
        };
        if binding.kind() != need.kind() {
            found.push(about_binding(
                Reason::NeedKind,
                unit,
                binding,
                &format!(
                    "as a need of kind {}, and its record has kind {}",
                    binding.kind().as_str(),
                    need.kind().as_str()
                ),
            ));
        }
        if !same_consumers(binding.consumers(), need) {
            found.push(about_binding(
                Reason::NeedConsumers,
                unit,
                binding,
                "with consumers other than its record lists",
            ));
        }
        if binding.via() == Via::Fallback
            && !matches!(need.when_unbound(), WhenUnbound::Fallback(_))
        {
            found.push(about_binding(
                Reason::UnboundDiffers,
                unit,
                binding,
                "from a fallback, and the need record does not fall back",
            ));
        }
        if let Some(path) = binding.offer() {
            match offers_by_path.get(path) {
                None => found.push(about_binding(
                    Reason::OfferUnknown,
                    unit,
                    binding,
                    "from an offer that the store does not hold",
                )),
                Some(offer) => {
                    if matches!(binding.supplies(), Supplies::Reference(reference) if reference != offer.reference())
                    {
                        found.push(about_binding(
                            Reason::OfferReference,
                            unit,
                            binding,
                            "with a reference other than the one in the offer record",
                        ));
                    }
                }
            }
        }
    }
    for entry in plan.unbound() {
        found.extend(unbound_failures(
            entry,
            needs_by_path.get(entry.need()).copied(),
        ));
    }
    if let Some(target) = target {
        found.extend(omitted(plan, target, &needs));
    }
    Ok(found)
}

/// The failures of the plan's target against its record.
fn target_failures(plan: &Plan, target: Option<&Target>) -> Vec<Failure> {
    let name = plan.target().name();
    match target {
        None => vec![Failure::new(
            Reason::TargetUnknown,
            format!("the store holds no target record named {name}, so its kind is unknown"),
        )],
        Some(target) if target.kind() != plan.target().kind() => vec![Failure::new(
            Reason::TargetKind,
            format!(
                "the plan gives target {name} kind {}, and its record has kind {}",
                plan.target().kind().as_str(),
                target.kind().as_str()
            ),
        )],
        Some(_) => Vec::new(),
    }
}

/// Reports whether `consumers` are the consumers of `need`, in any order.
fn same_consumers(consumers: &crate::Distinct<UnitName, 1>, need: &Need) -> bool {
    let stated: BTreeSet<&UnitName> = consumers.iter().collect();
    let held: BTreeSet<&UnitName> = need.consumers().iter().collect();
    stated == held
}

/// The failures of one need that the plan lists as unbound, against the
/// need's record.
fn unbound_failures(entry: &Unbound, need: Option<&Need>) -> Vec<Failure> {
    let about = |reason: Reason, what: &str| {
        Failure::new(reason, format!("{} {what}", entry.need())).of_need(entry.need().clone())
    };
    let Some(need) = need else {
        return vec![about(
            Reason::NeedUnknown,
            "is listed as unbound, and the store holds no need record at that path",
        )];
    };
    let mut found = Vec::new();
    if entry.kind() != need.kind() {
        found.push(about(
            Reason::NeedKind,
            &format!(
                "is listed as unbound with kind {}, and its record has kind {}",
                entry.kind().as_str(),
                need.kind().as_str()
            ),
        ));
    }
    if !same_consumers(entry.consumers(), need) {
        found.push(about(
            Reason::NeedConsumers,
            "is listed as unbound with consumers other than its record lists",
        ));
    }
    if entry.when_unbound() != UnboundAction::from(need.when_unbound()) {
        found.push(about(
            Reason::UnboundDiffers,
            "is listed as unbound with an answer other than its record gives",
        ));
    }
    if need.kind() == NeedKind::Secret && matches!(need.when_unbound(), WhenUnbound::Fallback(_)) {
        found.push(about(
            Reason::SecretFallback,
            "is a secret that falls back when unbound, and no secret has a fallback",
        ));
    }
    found
}

/// The needs that apply to `target`, that a unit of the plan receives, and
/// that the plan neither binds nor lists as unbound.
fn omitted(plan: &Plan, target: &Target, needs: &[Need]) -> Vec<Failure> {
    let mentioned: BTreeSet<&NeedPath> = bindings(plan)
        .map(|(_, binding)| binding.need())
        .chain(plan.unbound().iter().map(Unbound::need))
        .collect();
    needs
        .iter()
        .filter(|need| applies(target, need))
        .filter(|need| !mentioned.contains(&need.path()))
        .filter(|need| {
            need.consumers().iter().any(|consumer| {
                plan.units()
                    .iter()
                    .any(|unit| unit.app() == need.app() && unit.name() == consumer)
            })
        })
        .map(|need| {
            Failure::new(
                Reason::NeedOmitted,
                format!(
                    "{} applies to the target and a unit of the plan receives it, and the plan neither binds nor lists it",
                    need.path()
                ),
            )
            .of_need(need.path())
        })
        .collect()
}
