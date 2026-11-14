//! The path of each record in a store.
//!
//! | Record | Path |
//! | --- | --- |
//! | Need | `/jumpgate/needs/<app>/<name>` |
//! | Offer | `/jumpgate/offers/<provider-kind>/<provider>/<name>` |
//! | Binding | `/jumpgate/bindings/<target>/<app>/<need>` |
//! | Target | `/jumpgate/targets/<name>` |
//! | Unit | `/jumpgate/units/<app>/<unit>` |
//!
//! Every segment after the prefix is a typed name or a keyword, and each
//! follows a rule that allows no `/`, no `.` and no uppercase letter, so a
//! path built from a record's data cannot climb out of its directory or
//! name a second file on a file system that ignores case.

use crate::error::{Reason, RecordError};
use crate::keyword::ProviderKind;
use crate::name::{AppName, NeedName, OfferName, ProviderName, TargetName, UnitName};

mod reference;
#[cfg(test)]
mod tests;
mod text;

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

/// The path of a unit record, `/jumpgate/units/<app>/<unit>`.
///
/// mxv allows uppercase letters in a unit's name, and a path segment holds
/// none, so the unit's name is lowercase.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitPath {
    app: AppName,
    unit: UnitName,
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
    /// A unit's path.
    Unit(UnitPath),
}

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

impl UnitPath {
    /// Builds the path of the unit `unit` of `app`.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::Name`] when `unit` holds an uppercase letter,
    /// which no path segment holds.
    pub fn new(app: AppName, unit: UnitName) -> Result<Self, RecordError> {
        if unit
            .as_str()
            .chars()
            .any(|character| character.is_ascii_uppercase())
        {
            return Err(RecordError::new(
                Reason::Name,
                "a unit name in a record path holds no uppercase letter",
            ));
        }
        Ok(Self { app, unit })
    }

    /// Builds the path of a unit whose name `Unit::new` already checked.
    pub(crate) const fn from_checked(app: AppName, unit: UnitName) -> Self {
        Self { app, unit }
    }

    /// Returns the app.
    #[must_use]
    pub const fn app(&self) -> &AppName {
        &self.app
    }

    /// Returns the unit's name.
    #[must_use]
    pub const fn unit(&self) -> &UnitName {
        &self.unit
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

impl From<UnitPath> for RecordPath {
    fn from(path: UnitPath) -> Self {
        Self::Unit(path)
    }
}
