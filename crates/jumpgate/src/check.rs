//! The six checks that every plan must pass, whichever matcher wrote it.
//!
//! [`run`] runs every check on a parsed [`Plan`] and returns an [`Outcome`]
//! with one [`CheckResult`] for each [`Check`], in the order of the checks.
//! A check that fails lists one [`Failure`] for each unit, need or binding it
//! is about. A failure names its unit and need by their paths and never
//! quotes a value from the plan, because a value in the wrong place may be
//! the secret that check 1 looks for.
//!
//! The checks read only the plan. They read no store and no secret store,
//! and resolve no reference.
//!
//! 1. [`Check::SecretValue`]: the plan holds no secret value. Each binding
//!    states its need's kind. A literal value on a secret need fails, whatever
//!    made the binding, because starmap never gives a secret a default. A
//!    literal on another need is allowed only in a binding that a fallback
//!    made.
//! 2. [`Check::NeedsBound`]: every need is bound, or says when unbound that
//!    the plan can run without it: `omit`, or `fallback` for a target that
//!    takes one. A need that fails when unbound, a binding to an offer that
//!    does not exist and an ambiguous match each fail the check. A need that
//!    fails when unbound fails the check whatever its reason and whatever
//!    the target.
//! 3. [`Check::KnownProviders`]: every binding made by a matcher names an
//!    offer whose provider the plan knows, the secret store `one-password`
//!    or an app that has a unit in the plan, and the reference the binding
//!    supplies fits the offer.
//! 4. [`Check::SecretReach`]: a secret, or a fallback value, reaches only
//!    the units that declare its need. Each binding lists the need's
//!    consumers, and a unit outside that list, or in another app, fails.
//! 5. [`Check::NoCloudFallback`]: a plan for a cloud target has no need met
//!    by a fallback, as a binding or as a need that only a fallback would
//!    meet.
//! 6. [`Check::StartOrder`]: every unit that a unit starts after is a unit
//!    of the same app in the plan, each unit is listed once, and the start
//!    order has no cycle.

mod rules;

use std::fmt;

use crate::error::Code;
use crate::name::{AppName, UnitName};
use crate::path::NeedPath;
use crate::plan::Plan;

#[cfg(test)]
mod tests;

/// One of the six checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Check {
    /// The plan holds no secret value.
    SecretValue,
    /// Every need is bound or has a stated `when_unbound`.
    NeedsBound,
    /// Every binding names an offer with a known provider.
    KnownProviders,
    /// A secret reaches only the units that declare it.
    SecretReach,
    /// A plan for a cloud target has no need met by a fallback.
    NoCloudFallback,
    /// Every unit a unit starts after exists, and the start order has no
    /// cycle.
    StartOrder,
}

impl Check {
    /// Every check, in the order [`run`] runs them.
    pub const ALL: &'static [Self] = &[
        Self::SecretValue,
        Self::NeedsBound,
        Self::KnownProviders,
        Self::SecretReach,
        Self::NoCloudFallback,
        Self::StartOrder,
    ];

    /// Returns the check's number, from 1 to 6.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::SecretValue => 1,
            Self::NeedsBound => 2,
            Self::KnownProviders => 3,
            Self::SecretReach => 4,
            Self::NoCloudFallback => 5,
            Self::StartOrder => 6,
        }
    }

    /// Returns the check's name, such as `no-secret-value`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SecretValue => "no-secret-value",
            Self::NeedsBound => "needs-bound",
            Self::KnownProviders => "known-providers",
            Self::SecretReach => "secret-reach",
            Self::NoCloudFallback => "no-cloud-fallback",
            Self::StartOrder => "start-order",
        }
    }

    /// Returns what the check requires of a plan, in one sentence.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::SecretValue => "The plan holds no secret value.",
            Self::NeedsBound => "Every need is bound or has a stated when_unbound.",
            Self::KnownProviders => "Every binding names an offer with a known provider.",
            Self::SecretReach => "A secret reaches only the units that declare it.",
            Self::NoCloudFallback => "A plan for a cloud target has no need met by a fallback.",
            Self::StartOrder => {
                "Every unit a unit starts after exists, and the start order has no cycle."
            }
        }
    }
}

/// Why a check failed. Each reason belongs to one check and has the code
/// `jumpgate::check::<name>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Reason {
    /// Check 1: a binding carries a literal value although no fallback made
    /// it.
    SecretValue,
    /// Check 2: a need has no binding, and what it says to do when unbound
    /// does not let the plan run.
    NeedUnbound,
    /// Check 3: a binding's offer, its way of being matched and what it
    /// supplies disagree.
    OfferMismatch,
    /// Check 3: a binding names an offer whose provider the plan does not
    /// know.
    UnknownProvider,
    /// Check 4: a secret or a fallback value reaches a unit of an app that
    /// does not declare it.
    SecretReach,
    /// Check 5: a cloud plan has a need that a fallback meets.
    CloudFallback,
    /// Check 6: a unit starts after a unit that is not in the plan.
    UnitMissing,
    /// Check 6: the plan lists one unit more than once.
    UnitRepeated,
    /// Check 6: a unit is in a cycle of the start order.
    StartCycle,
}

impl Reason {
    /// Every reason, in the order of the checks.
    pub const ALL: &'static [Self] = &[
        Self::SecretValue,
        Self::NeedUnbound,
        Self::OfferMismatch,
        Self::UnknownProvider,
        Self::SecretReach,
        Self::CloudFallback,
        Self::UnitMissing,
        Self::UnitRepeated,
        Self::StartCycle,
    ];

    /// Returns the check that the reason belongs to.
    #[must_use]
    pub const fn check(self) -> Check {
        match self {
            Self::SecretValue => Check::SecretValue,
            Self::NeedUnbound => Check::NeedsBound,
            Self::OfferMismatch | Self::UnknownProvider => Check::KnownProviders,
            Self::SecretReach => Check::SecretReach,
            Self::CloudFallback => Check::NoCloudFallback,
            Self::UnitMissing | Self::UnitRepeated | Self::StartCycle => Check::StartOrder,
        }
    }

    /// Returns the reason's failure code, `jumpgate::check::<name>`.
    #[must_use]
    pub const fn code(self) -> Code {
        Code::new(
            "check",
            match self {
                Self::SecretValue => "secret_value",
                Self::NeedUnbound => "need_unbound",
                Self::OfferMismatch => "offer_mismatch",
                Self::UnknownProvider => "unknown_provider",
                Self::SecretReach => "secret_reach",
                Self::CloudFallback => "cloud_fallback",
                Self::UnitMissing => "unit_missing",
                Self::UnitRepeated => "unit_repeated",
                Self::StartCycle => "start_cycle",
            },
        )
    }
}

/// A unit of a plan, named by its app and its name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitRef {
    app: AppName,
    name: UnitName,
}

impl UnitRef {
    /// Names the unit `name` of `app`.
    pub(crate) const fn new(app: AppName, name: UnitName) -> Self {
        Self { app, name }
    }

    /// Returns the unit's app.
    #[must_use]
    pub const fn app(&self) -> &AppName {
        &self.app
    }

    /// Returns the unit's name within its app.
    #[must_use]
    pub const fn name(&self) -> &UnitName {
        &self.name
    }
}

impl fmt::Display for UnitRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.app, self.name)
    }
}

/// One thing that a check found wrong in a plan.
///
/// A failure names the unit and the need it is about, and its message is
/// built from those names alone, so it never quotes a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    reason: Reason,
    unit: Option<UnitRef>,
    need: Option<NeedPath>,
    message: String,
}

impl Failure {
    /// Builds a failure of `reason` with `message`, about no unit and no
    /// need.
    pub(crate) fn new(reason: Reason, message: String) -> Self {
        Self {
            reason,
            unit: None,
            need: None,
            message,
        }
    }

    /// Returns the failure about `unit`.
    #[must_use]
    pub(crate) fn of_unit(mut self, unit: UnitRef) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Returns the failure about `need`.
    #[must_use]
    pub(crate) fn of_need(mut self, need: NeedPath) -> Self {
        self.need = Some(need);
        self
    }

    /// Returns why the check failed.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        self.reason
    }

    /// Returns the check that failed.
    #[must_use]
    pub const fn check(&self) -> Check {
        self.reason.check()
    }

    /// Returns the failure code, `jumpgate::check::<reason>`.
    #[must_use]
    pub const fn code(&self) -> Code {
        self.reason.code()
    }

    /// Returns the unit the failure is about, if it is about one.
    #[must_use]
    pub const fn unit(&self) -> Option<&UnitRef> {
        self.unit.as_ref()
    }

    /// Returns the need the failure is about, if it is about one.
    #[must_use]
    pub const fn need(&self) -> Option<&NeedPath> {
        self.need.as_ref()
    }

    /// Returns the message, which names the unit and the need and quotes no
    /// value.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// What one check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    check: Check,
    failures: Vec<Failure>,
}

impl CheckResult {
    /// Returns the check.
    #[must_use]
    pub const fn check(&self) -> Check {
        self.check
    }

    /// Returns the failures of the check, which are empty when it passed.
    #[must_use]
    pub fn failures(&self) -> &[Failure] {
        &self.failures
    }

    /// Reports whether the check passed.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// What every check found in one plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    results: Vec<CheckResult>,
}

impl Outcome {
    /// Returns one result for each check, in the order of [`Check::ALL`].
    #[must_use]
    pub fn results(&self) -> &[CheckResult] {
        &self.results
    }

    /// Returns every failure of every check, in the order of the checks.
    pub fn failures(&self) -> impl Iterator<Item = &Failure> {
        self.results.iter().flat_map(|result| result.failures())
    }

    /// Reports whether every check passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.results.iter().all(CheckResult::passed)
    }
}

/// Runs every check on `plan`, whichever matcher wrote it.
#[must_use]
pub fn run(plan: &Plan) -> Outcome {
    Outcome {
        results: Check::ALL
            .iter()
            .map(|check| CheckResult {
                check: *check,
                failures: rules::failures(*check, plan),
            })
            .collect(),
    }
}
