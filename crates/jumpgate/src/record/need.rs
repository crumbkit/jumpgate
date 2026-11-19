//! The need record, `jumpgate/need/v1`, and the values only it holds.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::consumer::Consumer;
use crate::distinct::Distinct;
use crate::error::{Reason, RecordError};
use crate::fallback::FallbackValue;
use crate::file_path::{DeliveryPath, RepoFilePath};
use crate::json::{FromJson, Object, one_key, shape};
use crate::keyword::{NeedKind, ServiceKind};
use crate::name::{AppName, NeedName, ProfileName, ProvideName, TargetName};
use crate::path::{NeedPath, ProvidePath, RecordPath};
use crate::record::{Record, RecordType, fields, parse, type_tag};
use crate::repo::RepoId;
use crate::variable::VariableName;

#[cfg(test)]
mod tests;

type_tag!(NeedType, RecordType::Need);

/// One thing an app requires, such as a secret it reads or the address of
/// another app it calls. Its path is `/jumpgate/needs/<app>/<name>`.
///
/// The record has no capability shape yet. The design gives a capability
/// need a shape, and no reader in the first version produces a capability
/// need, so the field waits for the reader that does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Need {
    /// The record type, jumpgate/need/v1.
    #[serde(rename = "$type")]
    record_type: NeedType,
    /// The app that has the need.
    app: AppName,
    /// The need's name within its app.
    name: NeedName,
    /// What the need asks for.
    kind: NeedKind,
    /// The units of the app that receive the need. Every need has at least
    /// one, because a need that no unit receives has nothing to deliver to.
    consumers: Distinct<Consumer, 1>,
    /// The repository and file that declare the need.
    declared_in: DeclaredIn,
    /// The variables or the file the need is delivered as.
    delivers_as: DeliversAs,
    /// Whether the app runs without the need, as its declaration says. It
    /// agrees with `when_unbound`, which is what the matcher acts on.
    optional: bool,
    /// The profiles or targets for which the app has the need; empty means
    /// always.
    when: Distinct<Condition>,
    /// What happens when nothing binds the need. This field, and not
    /// `optional`, governs matching: the matcher fails, omits or falls back
    /// as it says. It agrees with `optional`, a rule across fields that
    /// this schema does not express: an optional need never fails when
    /// unbound, and a required need is never omitted. Either may fall back.
    when_unbound: WhenUnbound,
    /// What the declaration asks for, so the matcher can match from records
    /// alone: the repository and provide that a need names, or the provide
    /// that a starmap bind derives. Absent when the declaration asks for
    /// nothing in particular.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Request")]
    requests: Option<Request>,
}

impl Need {
    /// Builds the need `name` of `app`, which asks for `kind`, is received
    /// by `consumers`, is declared in `declared_in` and is delivered as
    /// `delivers_as`.
    ///
    /// The need starts required, for every profile and target, and failing
    /// when unbound. [`Need::with_when`] and [`Need::with_unbound`] change
    /// these.
    #[must_use]
    pub fn new(
        app: AppName,
        name: NeedName,
        kind: NeedKind,
        consumers: Distinct<Consumer, 1>,
        declared_in: DeclaredIn,
        delivers_as: DeliversAs,
    ) -> Self {
        Self {
            record_type: NeedType,
            app,
            name,
            kind,
            consumers,
            declared_in,
            delivers_as,
            optional: false,
            when: Distinct::default(),
            when_unbound: WhenUnbound::Fail,
            requests: None,
        }
    }

    /// Returns the need that asks for `requests`.
    #[must_use]
    pub fn with_requests(mut self, requests: Request) -> Self {
        self.requests = Some(requests);
        self
    }

    /// Returns the need with `when` as the profiles or targets it applies
    /// to.
    #[must_use]
    pub fn with_when(mut self, when: Distinct<Condition>) -> Self {
        self.when = when;
        self
    }

    /// Returns the need marked optional when `optional` is true, with
    /// `when_unbound` as what happens when nothing binds it.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::OptionalConflict`] when the two contradict each
    /// other: an optional need that fails when unbound, or a required need
    /// that is omitted.
    pub fn with_unbound(
        mut self,
        optional: bool,
        when_unbound: WhenUnbound,
    ) -> Result<Self, RecordError> {
        check_unbound(optional, &when_unbound)?;
        self.optional = optional;
        self.when_unbound = when_unbound;
        Ok(self)
    }

    /// Returns the app that has the need.
    #[must_use]
    pub const fn app(&self) -> &AppName {
        &self.app
    }

    /// Returns the need's name.
    #[must_use]
    pub const fn name(&self) -> &NeedName {
        &self.name
    }

    /// Returns what the need asks for.
    #[must_use]
    pub const fn kind(&self) -> NeedKind {
        self.kind
    }

    /// Returns the units that receive the need, of which there is at least
    /// one.
    #[must_use]
    pub const fn consumers(&self) -> &Distinct<Consumer, 1> {
        &self.consumers
    }

    /// Returns where the need is declared.
    #[must_use]
    pub const fn declared_in(&self) -> &DeclaredIn {
        &self.declared_in
    }

    /// Returns what the need is delivered as.
    #[must_use]
    pub const fn delivers_as(&self) -> &DeliversAs {
        &self.delivers_as
    }

    /// Reports whether the app runs without the need.
    #[must_use]
    pub const fn optional(&self) -> bool {
        self.optional
    }

    /// Returns the profiles or targets the need applies to; empty means
    /// always.
    #[must_use]
    pub const fn when(&self) -> &Distinct<Condition> {
        &self.when
    }

    /// Returns what happens when nothing binds the need.
    #[must_use]
    pub const fn when_unbound(&self) -> &WhenUnbound {
        &self.when_unbound
    }

    /// Returns what the declaration asks for, if it asks for anything in
    /// particular.
    #[must_use]
    pub const fn requests(&self) -> Option<&Request> {
        self.requests.as_ref()
    }

    /// Returns the need's path, `/jumpgate/needs/<app>/<name>`.
    #[must_use]
    pub fn path(&self) -> NeedPath {
        NeedPath::new(self.app.clone(), self.name.clone())
    }
}

impl Record for Need {
    const TYPE: RecordType = RecordType::Need;

    fn from_json(text: &str) -> Result<Self, RecordError> {
        let value = parse(text, Self::TYPE)?;
        let object = fields(
            &value,
            &[
                "app",
                "name",
                "kind",
                "consumers",
                "declared_in",
                "delivers_as",
                "optional",
                "when",
                "when_unbound",
                "requests",
            ],
        )?;
        let need = Self {
            record_type: NeedType,
            app: object.required("app")?,
            name: object.required("name")?,
            kind: object.required("kind")?,
            consumers: object.required("consumers")?,
            declared_in: object.required("declared_in")?,
            delivers_as: object.required("delivers_as")?,
            optional: object.required("optional")?,
            when: object.required("when")?,
            when_unbound: object.required("when_unbound")?,
            requests: object.optional("requests")?,
        };
        check_unbound(need.optional, &need.when_unbound)
            .map_err(|error| error.within("when_unbound"))?;
        Ok(need)
    }

    fn record_path(&self) -> RecordPath {
        self.path().into()
    }
}

/// Checks that `optional` agrees with `when_unbound`: an optional need
/// never fails when unbound, and a required need is never omitted.
fn check_unbound(optional: bool, when_unbound: &WhenUnbound) -> Result<(), RecordError> {
    let problem = match (optional, when_unbound) {
        (true, WhenUnbound::Fail) => {
            "an optional need never fails when unbound; use omit or a fallback"
        }
        (false, WhenUnbound::Omit) => "a required need is never omitted; use fail or a fallback",
        (true, WhenUnbound::Omit | WhenUnbound::Fallback(_))
        | (false, WhenUnbound::Fail | WhenUnbound::Fallback(_)) => return Ok(()),
    };
    Err(RecordError::new(Reason::OptionalConflict, problem))
}

/// The repository and the file within it that declare a need.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeclaredIn {
    /// The repository.
    repo: RepoId,
    /// The file within the repository, relative to its root. It has at
    /// most 1024 bytes, which maxLength cannot check for text outside ASCII
    /// because it counts characters.
    file: RepoFilePath,
}

impl DeclaredIn {
    /// Builds the place `file` within `repo`.
    #[must_use]
    pub const fn new(repo: RepoId, file: RepoFilePath) -> Self {
        Self { repo, file }
    }

    /// Returns the repository.
    #[must_use]
    pub const fn repo(&self) -> &RepoId {
        &self.repo
    }

    /// Returns the file within the repository.
    #[must_use]
    pub const fn file(&self) -> &RepoFilePath {
        &self.file
    }
}

impl FromJson for DeclaredIn {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let object = Object::open(value, &["repo", "file"])?;
        Ok(Self::new(
            object.required("repo")?,
            object.required("file")?,
        ))
    }
}

/// What a need is delivered as: environment variables or a file.
///
/// A later version may add a way to deliver a need, so a `match` outside
/// the crate needs a wildcard arm:
///
/// ```compile_fail,E0004
/// fn is_file(delivers_as: &jumpgate::DeliversAs) -> bool {
///     match delivers_as {
///         jumpgate::DeliversAs::Variables(_) => false,
///         jumpgate::DeliversAs::File(_) => true,
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DeliversAs {
    /// One or more environment variables, each of which receives the value.
    Variables(Distinct<VariableName, 1>),
    /// A file whose content is the value.
    File(DeliveryPath),
}

impl DeliversAs {
    /// Returns the kind of the service that these variables are the
    /// endpoint of, when one of them is the endpoint variable of a
    /// service kind. A need for a service is delivered as that variable, so
    /// the delivery records which kind the need was declared against.
    #[must_use]
    pub fn service_kind(&self) -> Option<ServiceKind> {
        let Self::Variables(variables) = self else {
            return None;
        };
        ServiceKind::ALL.iter().copied().find(|kind| {
            variables
                .iter()
                .any(|variable| variable.as_str() == kind.endpoint_variable())
        })
    }
}

/// A service need that meets a provide of a service of another kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindMismatch {
    /// The kind of service that the need was declared against.
    pub needed: ServiceKind,
    /// The kind of service that the provide gives.
    pub provided: ServiceKind,
}

impl std::fmt::Display for KindMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "the need was declared against a {} service, and the provide is a {} service",
            self.needed, self.provided
        )
    }
}

/// Pairs a need that is delivered as `delivers_as` with a provide that gives
/// `reference`, and refuses the pair when both are services and their kinds
/// differ. This is the one place that decides whether a service need may meet
/// a service provide. A need that is not for a service, or a provide that is not
/// a service, pairs with anything, because other checks govern those pairs.
///
/// # Errors
///
/// Returns the two kinds when they differ.
pub fn pair_service(
    delivers_as: &DeliversAs,
    reference: &crate::record::Reference,
) -> Result<(), KindMismatch> {
    match (delivers_as.service_kind(), reference) {
        (Some(needed), crate::record::Reference::Service(service)) if needed != service.kind() => {
            Err(KindMismatch {
                needed,
                provided: service.kind(),
            })
        }
        _ => Ok(()),
    }
}

impl FromJson for DeliversAs {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let (key, inner) = one_key(
            value,
            &["variables", "file"],
            "the value is not an object with exactly one of the keys variables and file",
        )?;
        let delivers_as = if key == "file" {
            DeliveryPath::from_json(inner).map(Self::File)
        } else {
            Distinct::from_json(inner).map(Self::Variables)
        };
        delivers_as.map_err(|error| error.within(key))
    }
}

/// A profile or a target for which an app has a need.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Condition {
    /// The app has the need when it runs in this profile.
    Profile(ProfileName),
    /// The app has the need when it runs on this target.
    Target(TargetName),
}

impl FromJson for Condition {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let (key, inner) = one_key(
            value,
            &["profile", "target"],
            "the value is not an object with exactly one of the keys profile and target",
        )?;
        let condition = if key == "target" {
            TargetName::from_json(inner).map(Self::Target)
        } else {
            ProfileName::from_json(inner).map(Self::Profile)
        };
        condition.map_err(|error| error.within(key))
    }
}

/// What happens to a need that nothing binds.
///
/// A later version may add a rule, so a `match` outside the crate needs a
/// wildcard arm:
///
/// ```compile_fail,E0004
/// fn fails(when_unbound: &jumpgate::WhenUnbound) -> bool {
///     match when_unbound {
///         jumpgate::WhenUnbound::Fail => true,
///         jumpgate::WhenUnbound::Omit | jumpgate::WhenUnbound::Fallback(_) => false,
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WhenUnbound {
    /// The plan lists the need as unbound, and the units that need it fail.
    Fail,
    /// The plan leaves the need out, and the app runs without it.
    Omit,
    /// On a local target the need receives this value, and the plan says
    /// so. The plan checks refuse a cloud plan in which a need is met by its
    /// fallback.
    Fallback(FallbackValue),
}

impl FromJson for WhenUnbound {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        match value {
            Value::String(word) => match word.as_str() {
                "fail" => Ok(Self::Fail),
                "omit" => Ok(Self::Omit),
                _ => Err(RecordError::new(
                    Reason::Keyword,
                    "the value is fail, omit, or an object whose one key is fallback",
                )),
            },
            Value::Object(_) => {
                let (_, inner) = one_key(
                    value,
                    &["fallback"],
                    "the value is not an object whose one key is fallback",
                )?;
                FallbackValue::from_json(inner)
                    .map(Self::Fallback)
                    .map_err(|error| error.within("fallback"))
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => Err(shape(
                "the value is fail, omit, or an object whose one key is fallback",
            )),
        }
    }
}

/// What a need's declaration asks for, which the matcher reads from the
/// need alone.
///
/// A later version may add a request, so a `match` outside the crate needs a
/// wildcard arm:
///
/// ```compile_fail,E0004
/// fn is_need(request: &jumpgate::Request) -> bool {
///     match request {
///         jumpgate::Request::Need(_) => true,
///         jumpgate::Request::Bind(_) => false,
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Request {
    /// The need comes from a need: the provide of the named repository with
    /// the named provide.
    Need(Source),
    /// The need is a secret whose starmap entry has a bind: the provide that
    /// the reader derived from the bind.
    Bind(ProvidePath),
}

/// The schema is one object with a `need` key or a `bind` key, and no more
/// than one, in place of the `oneOf` that the derive writes. Board finds a
/// reference only under `properties`, so a `oneOf` that holds the `bind`
/// reference would make Board refuse the schema.
impl JsonSchema for Request {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Request".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "What a need's declaration asks for: exactly one of `need`, the repository and\nprovide that a need names, and `bind`, the provide that the reader derived\nfrom a starmap bind.",
            "type": "object",
            "properties": {
                "need": generator.subschema_for::<Source>(),
                "bind": generator.subschema_for::<ProvidePath>()
            },
            "minProperties": 1,
            "maxProperties": 1,
            "additionalProperties": false
        })
    }
}

impl FromJson for Request {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let (key, inner) = one_key(
            value,
            &["need", "bind"],
            "the value is not an object with exactly one of the keys need and bind",
        )?;
        let request = if key == "need" {
            Source::from_json(inner).map(Self::Need)
        } else {
            ProvidePath::from_json(inner).map(Self::Bind)
        };
        request.map_err(|error| error.within(key))
    }
}

/// The repository and the name it provides, which a need asks for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The repository that provides the provide.
    repo: RepoId,
    /// The name that repository provides.
    from: ProvideName,
    /// The app, which is the last segment of `repo`.
    #[serde(skip)]
    #[schemars(skip)]
    app: AppName,
}

impl Source {
    /// Builds the source that names the provide `from` of `repo`.
    ///
    /// # Errors
    ///
    /// Returns a [`RecordError`] when the last segment of `repo` is not an
    /// app name, such as `shelf.rs`.
    pub fn new(repo: RepoId, from: ProvideName) -> Result<Self, RecordError> {
        let app = repo.app()?;
        Ok(Self { repo, from, app })
    }

    /// Returns the app that provides the provide, the last segment of the
    /// repository.
    #[must_use]
    pub const fn app(&self) -> &AppName {
        &self.app
    }

    /// Returns the repository that provides the provide.
    #[must_use]
    pub const fn repo(&self) -> &RepoId {
        &self.repo
    }

    /// Returns the name the repository provides.
    #[must_use]
    pub const fn provide(&self) -> &ProvideName {
        &self.from
    }
}

impl FromJson for Source {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let object = Object::open(value, &["repo", "from"])?;
        let repo: RepoId = object.required("repo")?;
        let from = object.required("from")?;
        Self::new(repo, from).map_err(|error| error.within("repo"))
    }
}
