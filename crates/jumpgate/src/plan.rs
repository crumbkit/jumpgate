//! The plan, `jumpgate/plan/v1alpha1`: the one interface between the
//! declarations and every target.
//!
//! A plan names a target and lists each unit with its start order, its
//! addresses and its bindings, then each need that nothing binds. It holds
//! references and the fallback values of a local target, and never a secret
//! value: a secret is the reference of an offer, such as
//! `op://vault/item/field`, which a unit's runtime resolves when the unit
//! starts. The `adapter` block belongs to a target adapter, and the matcher
//! leaves it empty.
//!
//! The format starts new in this crate. Its bindings hold references only.
//! It covers units that are not containers, gives the start order, and names
//! the need and offer each binding came from and the way each was matched.
//!
//! [`schema`] returns the JSON Schema, draft 2020-12, of the plan. The
//! committed copy is `schemas/jumpgate/plan/v1alpha1.schema.json`, and a
//! test fails when it differs from what [`schema`] returns.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::Serialize;

use crate::distinct::Distinct;
use crate::fallback::FallbackValue;
use crate::keyword::{NeedKind, TargetKind, UnitKind};
use crate::name::{AppName, Audience, PortName, Projection, TargetName, UnitName};
use crate::path::{NeedPath, OfferPath};
use crate::record::{DeliversAs, Reference, Target, WhenUnbound};
use crate::repo::RepoId;

mod read;
#[cfg(test)]
mod tests;

/// The most bytes a plan's text holds. A longer text is refused with the
/// `too_large` reason before it is parsed. A plan lists every unit and
/// binding of a stack, so its limit is larger than a record's.
pub const MAX_PLAN_BYTES: usize = 16 * 1024 * 1024;

/// The `$type` of a plan.
pub const PLAN_TYPE: &str = "jumpgate/plan/v1alpha1";

/// The path of the committed schema of the plan within the crate's `schemas`
/// directory.
pub const PLAN_SCHEMA_FILE: &str = "jumpgate/plan/v1alpha1.schema.json";

/// The `$type` field of a plan, which always holds [`PLAN_TYPE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlanTypeTag;

impl Serialize for PlanTypeTag {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(PLAN_TYPE)
    }
}

impl JsonSchema for PlanTypeTag {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PlanTypeTag".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "const": PLAN_TYPE
        })
    }
}

/// Returns the JSON Schema, draft 2020-12, of the plan, with every subschema
/// inline.
#[must_use]
pub fn schema() -> schemars::Schema {
    SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<Plan>()
}

/// A plan for one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    #[serde(rename = "$type")]
    plan_type: PlanTypeTag,
    target: PlanTarget,
    // The unbound needs come before the units, so a reader of the file sees
    // what blocks the plan before what it binds.
    unbound: Vec<Unbound>,
    units: Vec<PlanUnit>,
    adapter: Adapter,
}

impl Plan {
    /// Builds the plan of `target` from its `units` and the needs that
    /// nothing binds. The `adapter` block is empty.
    #[must_use]
    pub const fn new(target: PlanTarget, units: Vec<PlanUnit>, unbound: Vec<Unbound>) -> Self {
        Self {
            plan_type: PlanTypeTag,
            target,
            unbound,
            units,
            adapter: Adapter {},
        }
    }

    /// Returns the plan as pretty-printed JSON without a final newline. The
    /// plan holds only strings, lists and maps with string keys, so
    /// serialising it cannot fail.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Returns the target the plan is for.
    #[must_use]
    pub const fn target(&self) -> &PlanTarget {
        &self.target
    }

    /// Returns the units, ordered by app and name.
    #[must_use]
    pub fn units(&self) -> &[PlanUnit] {
        &self.units
    }

    /// Returns the needs that nothing binds, the ones that block the plan
    /// first.
    #[must_use]
    pub fn unbound(&self) -> &[Unbound] {
        &self.unbound
    }
}

/// The block that a target adapter owns. The matcher writes it empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Adapter {}

/// The target a plan is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanTarget {
    name: TargetName,
    kind: TargetKind,
}

impl PlanTarget {
    /// Names the target of `target`.
    #[must_use]
    pub fn of(target: &Target) -> Self {
        Self {
            name: target.name().clone(),
            kind: target.kind(),
        }
    }

    /// Returns the target's name.
    #[must_use]
    pub const fn name(&self) -> &TargetName {
        &self.name
    }

    /// Returns the target's kind.
    #[must_use]
    pub const fn kind(&self) -> TargetKind {
        self.kind
    }
}

/// A unit of the plan: a process or container, where it comes from, what it
/// starts after, the ports it serves and what it receives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanUnit {
    app: AppName,
    name: UnitName,
    repo: RepoId,
    kind: UnitKind,
    starts_after: Vec<UnitName>,
    addresses: Vec<PortName>,
    bindings: Vec<PlanBinding>,
}

impl PlanUnit {
    /// Builds a unit with no bindings. `starts_after` and `addresses` are
    /// the unit record's start order and served ports.
    #[must_use]
    pub const fn new(
        app: AppName,
        name: UnitName,
        repo: RepoId,
        kind: UnitKind,
        starts_after: Vec<UnitName>,
        addresses: Vec<PortName>,
    ) -> Self {
        Self {
            app,
            name,
            repo,
            kind,
            starts_after,
            addresses,
            bindings: Vec::new(),
        }
    }

    /// Adds `binding` to what the unit receives.
    pub fn bind(&mut self, binding: PlanBinding) {
        self.bindings.push(binding);
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

    /// Returns the repository the unit comes from.
    #[must_use]
    pub const fn repo(&self) -> &RepoId {
        &self.repo
    }

    /// Returns the unit's kind.
    #[must_use]
    pub const fn kind(&self) -> UnitKind {
        self.kind
    }

    /// Returns the units of the same app that this unit starts after.
    #[must_use]
    pub fn starts_after(&self) -> &[UnitName] {
        &self.starts_after
    }

    /// Returns the names of the ports the unit serves.
    #[must_use]
    pub fn addresses(&self) -> &[PortName] {
        &self.addresses
    }

    /// Returns what the unit receives, in the order the needs were matched.
    #[must_use]
    pub fn bindings(&self) -> &[PlanBinding] {
        &self.bindings
    }
}

/// How a need was matched to what it receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Via {
    /// A binding record in the target's authority store named the offer.
    Binding,
    /// The need's want names the offer's repository and provide.
    Want,
    /// The offer is the one the need's own declaration requests: the
    /// secret a starmap bind derives, or the local service that a process
    /// lists in `needs`.
    Bind,
    /// Nothing matched, and the need's fallback value is used on a local
    /// target.
    Fallback,
}

/// What a binding supplies to a unit: the reference of an offer, or the
/// fallback value of a local target. It never holds a secret value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Supplies {
    /// The reference of the offer, such as an `op://` secret reference or
    /// an app's port.
    Reference(Reference),
    /// The placeholder value of a need's fallback. A fallback is for a
    /// local target only.
    Fallback(FallbackValue),
}

/// A need matched for a unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanBinding {
    need: NeedPath,
    kind: NeedKind,
    consumers: Distinct<UnitName, 1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "OfferPath")]
    offer: Option<OfferPath>,
    via: Via,
    delivers_as: DeliversAs,
    supplies: Supplies,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Projection")]
    projection: Option<Projection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Audience")]
    audience: Option<Audience>,
}

impl PlanBinding {
    /// Builds the binding of `need`, a need of `kind` that `consumers`
    /// declare, which `via` matched to `offer` and which delivers
    /// `supplies` as `delivers_as`. `offer` is `None` for a fallback value.
    #[must_use]
    pub const fn new(
        need: NeedPath,
        kind: NeedKind,
        consumers: Distinct<UnitName, 1>,
        offer: Option<OfferPath>,
        via: Via,
        delivers_as: DeliversAs,
        supplies: Supplies,
    ) -> Self {
        Self {
            need,
            kind,
            consumers,
            offer,
            via,
            delivers_as,
            supplies,
            projection: None,
            audience: None,
        }
    }

    /// Sets the projection and audience that a binding record names.
    #[must_use]
    pub fn with_binding_fields(
        mut self,
        projection: Option<Projection>,
        audience: Option<Audience>,
    ) -> Self {
        self.projection = projection;
        self.audience = audience;
        self
    }

    /// Returns the path of the need record the binding came from.
    #[must_use]
    pub const fn need(&self) -> &NeedPath {
        &self.need
    }

    /// Returns the kind of the need, which says whether it is a secret.
    #[must_use]
    pub const fn kind(&self) -> NeedKind {
        self.kind
    }

    /// Returns the units of the need's app that declare the need, which are
    /// the only units that may receive its value.
    #[must_use]
    pub const fn consumers(&self) -> &Distinct<UnitName, 1> {
        &self.consumers
    }

    /// Returns the path of the offer record the binding came from, or
    /// `None` for a fallback value.
    #[must_use]
    pub const fn offer(&self) -> Option<&OfferPath> {
        self.offer.as_ref()
    }

    /// Returns how the need was matched.
    #[must_use]
    pub const fn via(&self) -> Via {
        self.via
    }

    /// Returns the variables or file the need is delivered as.
    #[must_use]
    pub const fn delivers_as(&self) -> &DeliversAs {
        &self.delivers_as
    }

    /// Returns what the binding supplies.
    #[must_use]
    pub const fn supplies(&self) -> &Supplies {
        &self.supplies
    }

    /// Returns the projection of the binding record, if it names one.
    #[must_use]
    pub const fn projection(&self) -> Option<&Projection> {
        self.projection.as_ref()
    }

    /// Returns the audience of the binding record, if it names one.
    #[must_use]
    pub const fn audience(&self) -> Option<&Audience> {
        self.audience.as_ref()
    }
}

/// Why a need has no binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnboundReason {
    /// No binding record, want or bind gave an offer.
    NoOffer,
    /// A binding record names an offer that the store does not hold.
    OfferMissing(OfferPath),
    /// Two or more offers match the need, and the matcher binds none of
    /// them.
    Ambiguous(Vec<OfferPath>),
    /// Only the need's fallback value would meet it, and the target is a
    /// cloud target, which never takes a fallback.
    FallbackOnCloud,
    /// The need asks for a local AWS service that mxv runs, and the target
    /// is a cloud target, where mxv's backends never run. A binding record
    /// that names another offer meets the need.
    LocalOnly,
    /// A service need meets an offer of a service of another kind, so no
    /// binding is made. The path is the offer, which a binding record or the
    /// need's request names.
    KindMismatch(OfferPath),
}

/// A need that no binding meets, with the need's own answer to that.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Unbound {
    need: NeedPath,
    kind: NeedKind,
    consumers: Distinct<UnitName, 1>,
    when_unbound: UnboundAction,
    reason: UnboundReason,
}

/// What a need says to do when nothing binds it, without the fallback value
/// that a need may carry. The value is a local-only literal, so the plan
/// never repeats it outside a binding on a local target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnboundAction {
    /// The units that need it fail.
    Fail,
    /// The app runs without it.
    Omit,
    /// The need names a fallback value.
    Fallback,
}

impl From<&WhenUnbound> for UnboundAction {
    fn from(when_unbound: &WhenUnbound) -> Self {
        match when_unbound {
            WhenUnbound::Fail => Self::Fail,
            WhenUnbound::Omit => Self::Omit,
            WhenUnbound::Fallback(_) => Self::Fallback,
        }
    }
}

impl Unbound {
    /// Lists `need`, a need of `kind` that `consumers` declare, as unbound
    /// for `reason`, with what its `when_unbound` says to do. The fallback
    /// value of the need is not kept.
    #[must_use]
    pub fn new(
        need: NeedPath,
        kind: NeedKind,
        consumers: Distinct<UnitName, 1>,
        when_unbound: WhenUnbound,
        reason: UnboundReason,
    ) -> Self {
        Self {
            need,
            kind,
            consumers,
            when_unbound: UnboundAction::from(&when_unbound),
            reason,
        }
    }

    /// Returns the path of the need.
    #[must_use]
    pub const fn need(&self) -> &NeedPath {
        &self.need
    }

    /// Returns the kind of the need.
    #[must_use]
    pub const fn kind(&self) -> NeedKind {
        self.kind
    }

    /// Returns the units that declare the need.
    #[must_use]
    pub const fn consumers(&self) -> &Distinct<UnitName, 1> {
        &self.consumers
    }

    /// Returns what the need says to do when nothing binds it.
    #[must_use]
    pub const fn when_unbound(&self) -> UnboundAction {
        self.when_unbound
    }

    /// Returns why nothing binds the need.
    #[must_use]
    pub const fn reason(&self) -> &UnboundReason {
        &self.reason
    }

    /// Reports whether the unbound need stops the plan: a need that fails
    /// when unbound, an ambiguous match, a binding to a missing offer, and
    /// a need met only by a fallback on a cloud target. An optional need
    /// that nothing binds, with `omit`, does not stop the plan.
    #[must_use]
    pub const fn blocks(&self) -> bool {
        match (&self.reason, self.when_unbound) {
            (UnboundReason::NoOffer | UnboundReason::LocalOnly, UnboundAction::Omit) => false,
            (
                UnboundReason::NoOffer | UnboundReason::LocalOnly,
                UnboundAction::Fail | UnboundAction::Fallback,
            )
            | (
                UnboundReason::OfferMissing(_)
                | UnboundReason::Ambiguous(_)
                | UnboundReason::KindMismatch(_)
                | UnboundReason::FallbackOnCloud,
                _,
            ) => true,
        }
    }
}
