//! The path of each record in a store.
//!
//! | Record | Path |
//! | --- | --- |
//! | Need | `/jumpgate/needs/<app>/<name>` |
//! | Offer | `/jumpgate/offers/<provider-kind>/<provider>/<name>` |
//! | Binding | `/jumpgate/bindings/<target>/<app>/<need>` |
//! | Target | `/jumpgate/targets/<name>` |
//!
//! Every segment after the prefix is a typed name or a keyword, and each
//! follows a rule that allows no `/`, no `.` and no uppercase letter, so a
//! path built from a record's data cannot climb out of its directory or
//! name a second file on a file system that ignores case.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::Serialize;
use serde_json::Value;

use crate::error::{Reason, RecordError};
use crate::json::{FromJson, text};
use crate::keyword::ProviderKind;
use crate::name::{AppName, NeedName, OfferName, ProviderName, TargetName};

#[cfg(test)]
mod tests;

/// The path of a need record, `/jumpgate/needs/<app>/<name>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NeedPath {
    app: AppName,
    name: NeedName,
}

/// The path of an offer record,
/// `/jumpgate/offers/<provider-kind>/<provider>/<name>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OfferPath {
    provider_kind: ProviderKind,
    provider: ProviderName,
    name: OfferName,
}

/// The path of a binding record, `/jumpgate/bindings/<target>/<app>/<need>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingPath {
    target: TargetName,
    app: AppName,
    need: NeedName,
}

/// The path of a target record, `/jumpgate/targets/<name>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetPath {
    name: TargetName,
}

/// The path of any jumpgate record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RecordPath {
    /// A need's path.
    Need(NeedPath),
    /// An offer's path.
    Offer(OfferPath),
    /// A binding's path.
    Binding(BindingPath),
    /// A target's path.
    Target(TargetPath),
}

const NEEDS: &str = "/jumpgate/needs/";
const OFFERS: &str = "/jumpgate/offers/";
const BINDINGS: &str = "/jumpgate/bindings/";
const TARGETS: &str = "/jumpgate/targets/";

impl NeedPath {
    /// Builds the path of the need `name` of `app`.
    #[must_use]
    pub const fn new(app: AppName, name: NeedName) -> Self {
        Self { app, name }
    }

    /// Returns the app.
    #[must_use]
    pub const fn app(&self) -> &AppName {
        &self.app
    }

    /// Returns the need's name.
    #[must_use]
    pub const fn name(&self) -> &NeedName {
        &self.name
    }
}

impl OfferPath {
    /// Builds the path of the offer `name` of the provider `provider` of
    /// kind `provider_kind`.
    #[must_use]
    pub const fn new(provider_kind: ProviderKind, provider: ProviderName, name: OfferName) -> Self {
        Self {
            provider_kind,
            provider,
            name,
        }
    }

    /// Returns the provider's kind.
    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    /// Returns the provider's name.
    #[must_use]
    pub const fn provider(&self) -> &ProviderName {
        &self.provider
    }

    /// Returns the offer's name.
    #[must_use]
    pub const fn name(&self) -> &OfferName {
        &self.name
    }
}

impl BindingPath {
    /// Builds the path of the binding for `target` of the need `need` of
    /// `app`.
    #[must_use]
    pub const fn new(target: TargetName, app: AppName, need: NeedName) -> Self {
        Self { target, app, need }
    }

    /// Returns the target.
    #[must_use]
    pub const fn target(&self) -> &TargetName {
        &self.target
    }

    /// Returns the app.
    #[must_use]
    pub const fn app(&self) -> &AppName {
        &self.app
    }

    /// Returns the need's name.
    #[must_use]
    pub const fn need(&self) -> &NeedName {
        &self.need
    }
}

impl TargetPath {
    /// Builds the path of the target `name`.
    #[must_use]
    pub const fn new(name: TargetName) -> Self {
        Self { name }
    }

    /// Returns the target's name.
    #[must_use]
    pub const fn name(&self) -> &TargetName {
        &self.name
    }
}

impl fmt::Display for NeedPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{NEEDS}{}/{}", self.app, self.name)
    }
}

impl fmt::Display for OfferPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{OFFERS}{}/{}/{}",
            self.provider_kind, self.provider, self.name
        )
    }
}

impl fmt::Display for BindingPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{BINDINGS}{}/{}/{}",
            self.target, self.app, self.need
        )
    }
}

impl fmt::Display for TargetPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{TARGETS}{}", self.name)
    }
}

impl fmt::Display for RecordPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Need(path) => path.fmt(formatter),
            Self::Offer(path) => path.fmt(formatter),
            Self::Binding(path) => path.fmt(formatter),
            Self::Target(path) => path.fmt(formatter),
        }
    }
}

/// Returns the `N` segments of `text` after `prefix`, or the error that
/// names `shape`, the form the path should have.
fn segments<'a, const N: usize>(
    text: &'a str,
    prefix: &str,
    shape: &'static str,
) -> Result<[&'a str; N], RecordError> {
    let rest = text.strip_prefix(prefix).ok_or_else(|| refused(shape))?;
    let mut parts = rest.split('/');
    let mut segments = [""; N];
    for slot in &mut segments {
        *slot = parts.next().ok_or_else(|| refused(shape))?;
    }
    if parts.next().is_some() {
        return Err(refused(shape));
    }
    Ok(segments)
}

/// Returns the [`Reason::RecordPath`] error for a path that is not `shape`.
fn refused(shape: &'static str) -> RecordError {
    RecordError::new(Reason::RecordPath, format!("the path is not {shape}"))
}

/// Parses one segment of a path, or returns the error that names `shape`.
fn segment<T: FromStr<Err = RecordError>>(
    text: &str,
    shape: &'static str,
) -> Result<T, RecordError> {
    text.parse().map_err(|_| refused(shape))
}

const NEED_SHAPE: &str = "/jumpgate/needs/<app>/<name>";
const OFFER_SHAPE: &str = "/jumpgate/offers/<provider-kind>/<provider>/<name>";
const BINDING_SHAPE: &str = "/jumpgate/bindings/<target>/<app>/<need>";
const TARGET_SHAPE: &str = "/jumpgate/targets/<name>";

impl FromStr for NeedPath {
    type Err = RecordError;

    /// Accepts `/jumpgate/needs/<app>/<name>`.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::RecordPath`] for any other text.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let [app, name] = segments(text, NEEDS, NEED_SHAPE)?;
        Ok(Self::new(
            segment(app, NEED_SHAPE)?,
            segment(name, NEED_SHAPE)?,
        ))
    }
}

impl FromStr for OfferPath {
    type Err = RecordError;

    /// Accepts `/jumpgate/offers/<provider-kind>/<provider>/<name>`.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::RecordPath`] for any other text.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let [kind, provider, name] = segments(text, OFFERS, OFFER_SHAPE)?;
        Ok(Self::new(
            segment(kind, OFFER_SHAPE)?,
            segment(provider, OFFER_SHAPE)?,
            segment(name, OFFER_SHAPE)?,
        ))
    }
}

impl FromStr for BindingPath {
    type Err = RecordError;

    /// Accepts `/jumpgate/bindings/<target>/<app>/<need>`.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::RecordPath`] for any other text.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let [target, app, need] = segments(text, BINDINGS, BINDING_SHAPE)?;
        Ok(Self::new(
            segment(target, BINDING_SHAPE)?,
            segment(app, BINDING_SHAPE)?,
            segment(need, BINDING_SHAPE)?,
        ))
    }
}

impl FromStr for TargetPath {
    type Err = RecordError;

    /// Accepts `/jumpgate/targets/<name>`.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::RecordPath`] for any other text.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let [name] = segments(text, TARGETS, TARGET_SHAPE)?;
        Ok(Self::new(segment(name, TARGET_SHAPE)?))
    }
}

impl FromStr for RecordPath {
    type Err = RecordError;

    /// Accepts the path of any jumpgate record.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::RecordPath`] for text that is no record's path.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.starts_with(NEEDS) {
            text.parse().map(Self::Need)
        } else if text.starts_with(OFFERS) {
            text.parse().map(Self::Offer)
        } else if text.starts_with(BINDINGS) {
            text.parse().map(Self::Binding)
        } else if text.starts_with(TARGETS) {
            text.parse().map(Self::Target)
        } else {
            Err(RecordError::new(
                Reason::RecordPath,
                "the path is not under /jumpgate/needs/, /jumpgate/offers/, /jumpgate/bindings/ or /jumpgate/targets/",
            ))
        }
    }
}

impl From<NeedPath> for RecordPath {
    fn from(path: NeedPath) -> Self {
        Self::Need(path)
    }
}

impl From<OfferPath> for RecordPath {
    fn from(path: OfferPath) -> Self {
        Self::Offer(path)
    }
}

impl From<BindingPath> for RecordPath {
    fn from(path: BindingPath) -> Self {
        Self::Binding(path)
    }
}

impl From<TargetPath> for RecordPath {
    fn from(path: TargetPath) -> Self {
        Self::Target(path)
    }
}

/// Implements the JSON form and the JSON Schema of a path that a record
/// field references. The schema carries `"format": "board-ref"`, so a
/// board that registers the type reads the field as a reference.
macro_rules! referenced_path {
    ($name:ident, $pattern:literal, $description:literal) => {
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl schemars::JsonSchema for $name {
            fn inline_schema() -> bool {
                true
            }

            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({
                    "type": "string",
                    "format": "board-ref",
                    "pattern": $pattern,
                    "description": $description
                })
            }
        }

        impl FromJson for $name {
            fn from_json(value: &Value) -> Result<Self, RecordError> {
                text(value)?.parse()
            }
        }
    };
}

referenced_path!(
    NeedPath,
    "^/jumpgate/needs/[a-z0-9][a-z0-9_-]{0,63}/[a-z0-9][a-z0-9_-]{0,63}$",
    "References a need record at /jumpgate/needs/<app>/<name>."
);

referenced_path!(
    OfferPath,
    "^/jumpgate/offers/(secret-store|app|workspace|capability|cloud-environment|identity-broker)/[a-z0-9][a-z0-9_-]{0,63}/[a-z0-9][a-z0-9_-]{0,63}$",
    "References an offer record at /jumpgate/offers/<provider-kind>/<provider>/<name>."
);
