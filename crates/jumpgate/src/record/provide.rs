//! The provide record, `jumpgate/provide/v1`, and the reference it holds.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::distinct::Distinct;
use crate::error::{Reason, RecordError};
use crate::json::{FromJson, Object, one_key};
use crate::keyword::{NeedKind, ProviderKind, Scope};
use crate::name::{AppName, PortName, ProvideName, ProviderName, UnitName};
use crate::path::{ProvidePath, RecordPath};
use crate::record::{Record, RecordType, fields, parse, type_tag};
use crate::secret::SecretReference;

#[cfg(test)]
mod tests;

type_tag!(ProvideType, RecordType::Provide);

/// Something a provider can supply, such as a secret in a secret store or a
/// port of an app. Its path is
/// `/jumpgate/provides/<provider-kind>/<provider>/<name>`.
///
/// A provide holds a reference to where its value lives and never the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provide {
    /// The record type, jumpgate/provide/v1.
    #[serde(rename = "$type")]
    record_type: ProvideType,
    /// The provider that makes the provide.
    provider: Provider,
    /// What the provide supplies, as the kind of need it meets.
    kind: NeedKind,
    /// The provide's name within its provider.
    name: ProvideName,
    /// Where the provide's value lives. The parser checks that the reference
    /// fits the provide, a rule across fields that this schema does not
    /// express: an op:// secret reference belongs to a secret provided by the
    /// secret store named one-password, a port or a route to an address
    /// provided by the app it names, and a unit to an app provided by the app
    /// it names. A provide from a secret store not named one-password has no
    /// reference variant yet.
    reference: Reference,
    /// The apps that may bind the provide; empty means any app.
    grants: Distinct<AppName>,
    /// The provide this one is derived from, such as a public key derived
    /// from a signing key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ProvidePath")]
    derived_from: Option<ProvidePath>,
    /// How widely the provide's value is shared.
    scope: Scope,
}

impl Provide {
    /// Builds the provide `name` of `provider`, which supplies `kind` from
    /// `reference` with `scope`. It starts granted to any app and derived
    /// from no other provide.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::ReferenceMismatch`] when `reference` does not fit
    /// `kind` or `provider`: an `op://` secret reference belongs to a secret
    /// provided by the secret store named `one-password`, a port or a route
    /// to an address provided by the app it names, and a unit to an app
    /// provided by the app it names. It returns the same reason for a kind
    /// or a provider kind that has no reference variant yet, or a secret
    /// store not named `one-password`.
    pub fn new(
        provider: Provider,
        kind: NeedKind,
        name: ProvideName,
        reference: Reference,
        scope: Scope,
    ) -> Result<Self, RecordError> {
        check_reference(&provider, kind, &reference)?;
        Ok(Self {
            record_type: ProvideType,
            provider,
            kind,
            name,
            reference,
            grants: Distinct::default(),
            derived_from: None,
            scope,
        })
    }

    /// Returns the provide granted only to `grants`, or to any app when
    /// `grants` is empty.
    #[must_use]
    pub fn with_grants(mut self, grants: Distinct<AppName>) -> Self {
        self.grants = grants;
        self
    }

    /// Returns the provide marked as derived from the provide at `derived_from`.
    #[must_use]
    pub fn with_derived_from(mut self, derived_from: ProvidePath) -> Self {
        self.derived_from = Some(derived_from);
        self
    }

    /// Returns the provider.
    #[must_use]
    pub const fn provider(&self) -> &Provider {
        &self.provider
    }

    /// Returns what the provide supplies.
    #[must_use]
    pub const fn kind(&self) -> NeedKind {
        self.kind
    }

    /// Returns the provide's name.
    #[must_use]
    pub const fn name(&self) -> &ProvideName {
        &self.name
    }

    /// Returns where the provide's value lives.
    #[must_use]
    pub const fn reference(&self) -> &Reference {
        &self.reference
    }

    /// Returns the apps that may bind the provide; empty means any app.
    #[must_use]
    pub const fn grants(&self) -> &Distinct<AppName> {
        &self.grants
    }

    /// Returns the provide this one is derived from, if any.
    #[must_use]
    pub const fn derived_from(&self) -> Option<&ProvidePath> {
        self.derived_from.as_ref()
    }

    /// Returns how widely the provide's value is shared.
    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }

    /// Returns the provide's path,
    /// `/jumpgate/provides/<provider-kind>/<provider>/<name>`.
    #[must_use]
    pub fn path(&self) -> ProvidePath {
        ProvidePath::new(
            self.provider.kind,
            self.provider.name.clone(),
            self.name.clone(),
        )
    }
}

/// The name of the secret-store provider whose references are `op://`
/// references.
pub(crate) const ONE_PASSWORD: &str = "one-password";

/// Checks that `reference` fits a provide of `kind` from `provider`.
///
/// `Reference` has variants only for what the first readers produce: a
/// secret in the secret store [`ONE_PASSWORD`], and an app's port, route or
/// unit. So the check handles provides of kind secret, address and app from
/// the secret store [`ONE_PASSWORD`] or an app provider. It refuses every
/// provide from a workspace, capability, cloud-environment or
/// identity-broker provider, every provide from a secret store not named
/// [`ONE_PASSWORD`], and every provide of kind value, data-access or
/// capability, with a message that says it has no reference variant yet.
/// The reader that first produces such a provide adds its variant to
/// `Reference` and its case here; the kinds stay in [`ProviderKind`] and
/// [`NeedKind`] because needs already name them.
fn check_reference(
    provider: &Provider,
    kind: NeedKind,
    reference: &Reference,
) -> Result<(), RecordError> {
    if !matches!(provider.kind, ProviderKind::SecretStore | ProviderKind::App) {
        return Err(no_variant(&format!(
            "a provide from a {} provider",
            provider.kind
        )));
    }
    if provider.kind == ProviderKind::SecretStore && provider.name.as_str() != ONE_PASSWORD {
        return Err(no_variant(&format!(
            "a provide from a secret store not named {ONE_PASSWORD}"
        )));
    }
    if !matches!(kind, NeedKind::Secret | NeedKind::Address | NeedKind::App) {
        return Err(no_variant(&format!("a provide of kind {kind}")));
    }
    let fits = match reference {
        Reference::Secret(_) => {
            provider.kind == ProviderKind::SecretStore && kind == NeedKind::Secret
        }
        Reference::Port(part) => is_app_part(provider, kind, NeedKind::Address, part.app()),
        Reference::Route(part) => is_app_part(provider, kind, NeedKind::Address, part.app()),
        Reference::Unit(part) => is_app_part(provider, kind, NeedKind::App, part.app()),
    };
    if fits {
        Ok(())
    } else {
        Err(RecordError::new(
            Reason::ReferenceMismatch,
            "the reference does not fit the provide: an op:// secret reference belongs to a \
             secret from the secret store named one-password, a port or a route to an address \
             from the app it names, and a unit to an app provided by the app it names",
        ))
    }
}

/// Returns the [`Reason::ReferenceMismatch`] error for `what`, such as "an
/// provide of kind value", which no variant of [`Reference`] can reference
/// yet.
fn no_variant(what: &str) -> RecordError {
    RecordError::new(
        Reason::ReferenceMismatch,
        format!("{what} has no reference variant yet, so jumpgate cannot record it"),
    )
}

/// Reports whether a provide of `kind` from `provider` may reference a part
/// of `app`: the provider is that app and the kind is `expected`.
fn is_app_part(provider: &Provider, kind: NeedKind, expected: NeedKind, app: &AppName) -> bool {
    provider.kind == ProviderKind::App && kind == expected && provider.name.as_str() == app.as_str()
}

impl Record for Provide {
    const TYPE: RecordType = RecordType::Provide;

    fn from_json(text: &str) -> Result<Self, RecordError> {
        let value = parse(text, Self::TYPE)?;
        let object = fields(
            &value,
            &[
                "provider",
                "kind",
                "name",
                "reference",
                "grants",
                "derived_from",
                "scope",
            ],
        )?;
        let provider: Provider = object.required("provider")?;
        let kind = object.required("kind")?;
        let name = object.required("name")?;
        let reference: Reference = object.required("reference")?;
        let grants = object.required("grants")?;
        let derived_from = object.optional("derived_from")?;
        let scope = object.required("scope")?;
        check_reference(&provider, kind, &reference).map_err(|error| error.within("reference"))?;
        Ok(Self {
            record_type: ProvideType,
            provider,
            kind,
            name,
            reference,
            grants,
            derived_from,
            scope,
        })
    }

    fn record_path(&self) -> RecordPath {
        self.path().into()
    }
}

/// The provider of a provide: its kind and its name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    /// The kind of provider.
    kind: ProviderKind,
    /// The provider's name within its kind.
    name: ProviderName,
}

impl Provider {
    /// Builds the provider `name` of kind `kind`.
    #[must_use]
    pub const fn new(kind: ProviderKind, name: ProviderName) -> Self {
        Self { kind, name }
    }

    /// Returns the provider's kind.
    #[must_use]
    pub const fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// Returns the provider's name.
    #[must_use]
    pub const fn name(&self) -> &ProviderName {
        &self.name
    }
}

impl FromJson for Provider {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let object = Object::open(value, &["kind", "name"])?;
        Ok(Self::new(
            object.required("kind")?,
            object.required("name")?,
        ))
    }
}

/// Where a provide's value lives. No variant can hold a secret value: a
/// secret is named by its secret-store reference, and everything else by
/// the app and the name of its port, route or unit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reference {
    /// A secret in a secret store.
    Secret(SecretReference),
    /// A port of an app, reached directly.
    Port(PortReference),
    /// A route of an app, a port that the proxy serves.
    Route(RouteReference),
    /// A unit of an app.
    Unit(UnitReference),
}

impl FromJson for Reference {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let (key, inner) = one_key(
            value,
            &["secret", "port", "route", "unit"],
            "the value is not an object with exactly one of the keys secret, port, route and unit",
        )?;
        let reference = match key {
            "secret" => SecretReference::from_json(inner).map(Self::Secret),
            "port" => PortReference::from_json(inner).map(Self::Port),
            "route" => RouteReference::from_json(inner).map(Self::Route),
            _ => UnitReference::from_json(inner).map(Self::Unit),
        };
        reference.map_err(|error| error.within(key))
    }
}

/// Declares a reference to a named part of an app.
macro_rules! app_part {
    ($(#[$doc:meta])* $name:ident, $field:ident, $field_type:ty, $field_doc:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            /// The app.
            app: AppName,
            #[doc = $field_doc]
            $field: $field_type,
        }

        impl $name {
            #[doc = concat!("Builds the reference to `", stringify!($field), "` of `app`.")]
            #[must_use]
            pub const fn new(app: AppName, $field: $field_type) -> Self {
                Self { app, $field }
            }

            /// Returns the app.
            #[must_use]
            pub const fn app(&self) -> &AppName {
                &self.app
            }

            #[doc = concat!("Returns the ", stringify!($field), ".")]
            #[must_use]
            pub const fn $field(&self) -> &$field_type {
                &self.$field
            }
        }

        impl FromJson for $name {
            fn from_json(value: &Value) -> Result<Self, RecordError> {
                let object = Object::open(value, &["app", stringify!($field)])?;
                Ok(Self::new(
                    object.required("app")?,
                    object.required(stringify!($field))?,
                ))
            }
        }
    };
}

app_part! {
    /// A port of an app, such as the port `http` of `auth`.
    PortReference, port, PortName, "The port's name."
}

app_part! {
    /// A route of an app: the port that the proxy serves.
    RouteReference, port, PortName, "The name of the port that the proxy serves."
}

app_part! {
    /// A unit of an app, an mxv process or container.
    UnitReference, unit, UnitName, "The unit's name."
}
