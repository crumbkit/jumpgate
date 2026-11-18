//! Matching needs to offers for one target, and the plan that results.
//!
//! [`Matcher`] decides what meets one need, and [`resolve`] runs a matcher
//! over every need of a store and builds the [`Plan`]. How a need is matched
//! is open, and a person or a company may supply their own matcher. The
//! crate ships [`DefaultMatcher`], which follows four rules in order:
//!
//! 1. A binding record for the need, for this target, in the target's
//!    authority board wins. A binding for the target in any other board is
//!    reported and ignored.
//! 2. A need that comes from a want matches the offer of the named
//!    repository's app with the named provide.
//! 3. A secret need that requests the offer a starmap bind derives matches
//!    that offer.
//! 4. Anything else is unbound. On a local target a need with a fallback is
//!    bound to its fallback. On a cloud target such a need is listed as
//!    unbound, because a fallback never reaches a cloud target.
//!
//! When two offers match one need, the matcher reports both and binds
//! neither. A need whose `when` names targets applies only to those
//! targets, and a need whose `when` names only profiles never applies,
//! because a plan is for a target and has no profile.

use std::collections::BTreeSet;
use std::fmt;

use crate::fallback::FallbackValue;
use crate::keyword::{NeedKind, ProviderKind, TargetKind};
use crate::name::{Audience, BoardId, Projection, UnitName};
use crate::path::{NeedPath, OfferPath};
use crate::plan::{Plan, PlanBinding, PlanTarget, PlanUnit, Supplies, Unbound, UnboundReason, Via};
use crate::record::{Binding, Condition, Need, Offer, Request, Target, Unit, WhenUnbound};
use crate::store::StoredBinding;

#[cfg(test)]
mod tests;

/// Everything a matcher reads: the target and the records of the store.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
    target: &'a Target,
    needs: &'a [Need],
    offers: &'a [Offer],
    units: &'a [Unit],
    bindings: &'a [StoredBinding],
}

impl<'a> Input<'a> {
    /// Gathers the records to match for `target`. `bindings` are the
    /// bindings for the target from every board, each with its board.
    #[must_use]
    pub const fn new(
        target: &'a Target,
        needs: &'a [Need],
        offers: &'a [Offer],
        units: &'a [Unit],
        bindings: &'a [StoredBinding],
    ) -> Self {
        Self {
            target,
            needs,
            offers,
            units,
            bindings,
        }
    }

    /// Returns the target.
    #[must_use]
    pub const fn target(&self) -> &'a Target {
        self.target
    }

    /// Returns the needs.
    #[must_use]
    pub const fn needs(&self) -> &'a [Need] {
        self.needs
    }

    /// Returns the offers.
    #[must_use]
    pub const fn offers(&self) -> &'a [Offer] {
        self.offers
    }

    /// Returns the units.
    #[must_use]
    pub const fn units(&self) -> &'a [Unit] {
        self.units
    }

    /// Returns the bindings for the target, with the board of each.
    #[must_use]
    pub const fn bindings(&self) -> &'a [StoredBinding] {
        self.bindings
    }

    /// Returns the offer at `path`, if the store holds it.
    #[must_use]
    pub fn offer(&self, path: &OfferPath) -> Option<&'a Offer> {
        self.offers.iter().find(|offer| offer.path() == *path)
    }
}

/// The offer that meets a need, and how it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    offer: OfferPath,
    via: Via,
    projection: Option<Projection>,
    audience: Option<Audience>,
}

impl Choice {
    /// Chooses `offer`, which `via` found.
    #[must_use]
    pub const fn new(offer: OfferPath, via: Via) -> Self {
        Self {
            offer,
            via,
            projection: None,
            audience: None,
        }
    }

    /// Takes the projection and audience of `binding`, the record that
    /// named the offer.
    #[must_use]
    pub fn from_binding(binding: &Binding) -> Self {
        Self {
            offer: binding.offer().clone(),
            via: Via::Binding,
            projection: binding.projection().cloned(),
            audience: binding.audience().cloned(),
        }
    }

    /// Returns the chosen offer's path.
    #[must_use]
    pub const fn offer(&self) -> &OfferPath {
        &self.offer
    }

    /// Returns how the offer was found.
    #[must_use]
    pub const fn via(&self) -> Via {
        self.via
    }
}

/// What a matcher decides for one need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// An offer meets the need.
    Bound(Choice),
    /// The need receives its fallback value.
    Fallback(FallbackValue),
    /// Nothing meets the need.
    Unbound(UnboundReason),
}

/// Decides which offer meets a need, for one target.
pub trait Matcher {
    /// Returns what meets `need`.
    fn decide(&self, input: &Input<'_>, need: &Need) -> Decision;
}

/// The matcher that follows the four rules in the module's documentation.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultMatcher;

impl Matcher for DefaultMatcher {
    fn decide(&self, input: &Input<'_>, need: &Need) -> Decision {
        let target = input.target();
        let path = need.path();
        let chosen = authority_bindings(input, &path)
            .or_else(|| wanted(input, need))
            .or_else(|| bound(input, need));
        match chosen {
            Some(Candidates::One(choice)) => Decision::Bound(choice),
            Some(Candidates::Several(offers)) => {
                Decision::Unbound(UnboundReason::Ambiguous(offers))
            }
            None => match (need.when_unbound(), target.kind()) {
                (WhenUnbound::Fallback(value), TargetKind::Local) => {
                    Decision::Fallback(value.clone())
                }
                (WhenUnbound::Fallback(_), TargetKind::Cloud) => {
                    Decision::Unbound(UnboundReason::FallbackOnCloud)
                }
                (WhenUnbound::Fail | WhenUnbound::Omit, _) => {
                    Decision::Unbound(UnboundReason::NoOffer)
                }
            },
        }
    }
}

/// The offers that one rule found for a need.
enum Candidates {
    One(Choice),
    Several(Vec<OfferPath>),
}

/// Rule 1: the binding records of the target's authority board.
fn authority_bindings(input: &Input<'_>, need: &NeedPath) -> Option<Candidates> {
    let found: Vec<&Binding> = input
        .bindings()
        .iter()
        .filter(|stored| stored.board() == input.target().authority())
        .map(StoredBinding::binding)
        .filter(|binding| binding.need() == need && binding.target() == input.target().name())
        .collect();
    let distinct: BTreeSet<&OfferPath> = found.iter().map(|binding| binding.offer()).collect();
    match (found.first(), distinct.len()) {
        (None, _) => None,
        (Some(binding), 1) => Some(Candidates::One(Choice::from_binding(binding))),
        (Some(_), _) => Some(Candidates::Several(distinct.into_iter().cloned().collect())),
    }
}

/// Rule 2: the offer of the named repository's app with the named provide.
fn wanted(input: &Input<'_>, need: &Need) -> Option<Candidates> {
    let Some(Request::Want(want)) = need.requests() else {
        return None;
    };
    // The kind of the need is not compared, because the reader guesses it
    // when the provider was not read in the same run.
    let app = want.app();
    let found: BTreeSet<OfferPath> = input
        .offers()
        .iter()
        .filter(|offer| {
            offer.provider().kind() == ProviderKind::App
                && offer.provider().name().as_str() == app.as_str()
                && offer.name() == want.provides()
        })
        .map(Offer::path)
        .collect();
    candidates(found, Via::Want)
}

/// Rule 3: the offer that a starmap bind derives.
fn bound(input: &Input<'_>, need: &Need) -> Option<Candidates> {
    let Some(Request::Bind(path)) = need.requests() else {
        return None;
    };
    input
        .offer(path)
        .filter(|offer| offer.kind() == NeedKind::Secret)
        .map(|offer| Candidates::One(Choice::new(offer.path(), Via::Bind)))
}

fn candidates(found: BTreeSet<OfferPath>, via: Via) -> Option<Candidates> {
    let mut found = found.into_iter();
    match (found.next(), found.next()) {
        (None, _) => None,
        (Some(only), None) => Some(Candidates::One(Choice::new(only, via))),
        (Some(first), Some(second)) => {
            let mut all = vec![first, second];
            all.extend(found);
            Some(Candidates::Several(all))
        }
    }
}

/// Something `resolve` found that is not a failure of the plan but that a
/// reader of the result needs to see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Note {
    /// A need is met by its fallback value on a local target.
    FallbackUsed {
        /// The need.
        need: NeedPath,
    },
    /// A binding for the target sits in a board that is not the target's
    /// authority, so it is ignored.
    IgnoredBinding {
        /// The need the binding names.
        need: NeedPath,
        /// The board the binding came from.
        board: BoardId,
    },
    /// A need names a consumer unit that no unit record describes, so the
    /// plan gives that unit no binding for it.
    UnknownConsumer {
        /// The need.
        need: NeedPath,
        /// The unit that no record describes.
        unit: UnitName,
    },
    /// A need applies only to other targets or to profiles, so the plan
    /// leaves it out.
    NotForTarget {
        /// The need.
        need: NeedPath,
    },
}

impl fmt::Display for Note {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FallbackUsed { need } => write!(
                formatter,
                "{need}: nothing offers it, so the plan binds its fallback value"
            ),
            Self::IgnoredBinding { need, board } => write!(
                formatter,
                "{need}: a binding in board {board} is ignored, because that board is not the target's authority"
            ),
            Self::UnknownConsumer { need, unit } => write!(
                formatter,
                "{need}: the consumer {unit} has no unit record, so the plan gives it no binding"
            ),
            Self::NotForTarget { need } => write!(
                formatter,
                "{need}: it does not apply to this target, so the plan leaves it out"
            ),
        }
    }
}

/// The plan for a target and what `resolve` noted on the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    plan: Plan,
    notes: Vec<Note>,
}

impl Resolution {
    /// Returns the plan.
    #[must_use]
    pub const fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Returns the notes, in the order of the needs.
    #[must_use]
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// Reports whether any need blocks the plan, which is when a command
    /// exits with 1.
    #[must_use]
    pub fn blocked(&self) -> bool {
        self.plan.unbound().iter().any(Unbound::blocks)
    }
}

/// Matches every need of `input` with `matcher` and builds the plan.
///
/// Each unit of the input is a unit of the plan. A need that is bound adds
/// a binding to each unit that consumes it, and a need that is not bound is
/// listed in the plan with its reason. The listing puts the needs that block
/// the plan first.
#[must_use]
pub fn resolve(matcher: &impl Matcher, input: &Input<'_>) -> Resolution {
    let mut units: Vec<PlanUnit> = input.units().iter().map(plan_unit).collect();
    units.sort_by(|a, b| (a.app(), a.name()).cmp(&(b.app(), b.name())));
    let mut unbound: Vec<Unbound> = Vec::new();
    let mut notes: Vec<Note> = Vec::new();
    for need in input.needs() {
        if !applies(input.target(), need) {
            notes.push(Note::NotForTarget { need: need.path() });
            continue;
        }
        notes.extend(ignored_bindings(input, &need.path()));
        match binding_of(input, need, matcher.decide(input, need)) {
            Ok(binding) => {
                if binding.via() == Via::Fallback {
                    notes.push(Note::FallbackUsed { need: need.path() });
                }
                deliver(&mut units, &mut notes, need, &binding);
            }
            Err(reason) => unbound.push(Unbound::new(
                need.path(),
                need.kind(),
                need.consumers().clone(),
                need.when_unbound().clone(),
                reason,
            )),
        }
    }
    unbound.sort_by_key(|entry| !entry.blocks());
    Resolution {
        plan: Plan::new(PlanTarget::of(input.target()), units, unbound),
        notes,
    }
}

/// Turns a matcher's `decision` for `need` into a plan binding, or into the
/// reason nothing binds it. A choice of an offer that the store lacks is
/// unbound, and so is a fallback on a cloud target, whichever matcher made
/// the decision.
fn binding_of(
    input: &Input<'_>,
    need: &Need,
    decision: Decision,
) -> Result<PlanBinding, UnboundReason> {
    match decision {
        Decision::Bound(choice) => {
            let offer = input
                .offer(choice.offer())
                .ok_or_else(|| UnboundReason::OfferMissing(choice.offer().clone()))?;
            Ok(PlanBinding::new(
                need.path(),
                need.kind(),
                need.consumers().clone(),
                Some(choice.offer.clone()),
                choice.via,
                need.delivers_as().clone(),
                Supplies::Reference(offer.reference().clone()),
            )
            .with_binding_fields(choice.projection, choice.audience))
        }
        Decision::Fallback(_) if input.target().kind() == TargetKind::Cloud => {
            Err(UnboundReason::FallbackOnCloud)
        }
        Decision::Fallback(value) => Ok(PlanBinding::new(
            need.path(),
            need.kind(),
            need.consumers().clone(),
            None,
            Via::Fallback,
            need.delivers_as().clone(),
            Supplies::Fallback(value),
        )),
        Decision::Unbound(reason) => Err(reason),
    }
}

fn plan_unit(unit: &Unit) -> PlanUnit {
    PlanUnit::new(
        unit.app().clone(),
        unit.name().clone(),
        unit.repo().clone(),
        unit.kind(),
        unit.starts_after().iter().cloned().collect(),
        unit.ports().iter().cloned().collect(),
    )
}

/// Reports whether a need applies to `target`.
fn applies(target: &Target, need: &Need) -> bool {
    need.when().is_empty()
        || need
            .when()
            .iter()
            .any(|condition| matches!(condition, Condition::Target(name) if name == target.name()))
}

fn ignored_bindings(input: &Input<'_>, need: &NeedPath) -> Vec<Note> {
    input
        .bindings()
        .iter()
        .filter(|stored| {
            stored.board() != input.target().authority()
                && stored.binding().need() == need
                && stored.binding().target() == input.target().name()
        })
        .map(|stored| Note::IgnoredBinding {
            need: need.clone(),
            board: stored.board().clone(),
        })
        .collect()
}

/// Adds `binding` to each unit that consumes `need`.
fn deliver(units: &mut [PlanUnit], notes: &mut Vec<Note>, need: &Need, binding: &PlanBinding) {
    for consumer in need.consumers() {
        let unit = units
            .iter_mut()
            .find(|unit| unit.app() == need.app() && unit.name() == consumer);
        match unit {
            Some(unit) => unit.bind(binding.clone()),
            None => notes.push(Note::UnknownConsumer {
                need: need.path(),
                unit: consumer.clone(),
            }),
        }
    }
}
