//! The text form of each record path, which `Display` writes and
//! `FromStr` reads.

use std::fmt;
use std::str::FromStr;

use crate::error::{Reason, RecordError};
use crate::path::{BindingPath, NeedPath, OfferPath, RecordPath, TargetPath, UnitPath};

const NEEDS: &str = "/jumpgate/needs/";
const OFFERS: &str = "/jumpgate/offers/";
const BINDINGS: &str = "/jumpgate/bindings/";
const TARGETS: &str = "/jumpgate/targets/";
const UNITS: &str = "/jumpgate/units/";

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

impl fmt::Display for UnitPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{UNITS}{}/{}", self.app, self.unit)
    }
}

impl fmt::Display for RecordPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Need(path) => path.fmt(formatter),
            Self::Offer(path) => path.fmt(formatter),
            Self::Binding(path) => path.fmt(formatter),
            Self::Target(path) => path.fmt(formatter),
            Self::Unit(path) => path.fmt(formatter),
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
const UNIT_SHAPE: &str = "/jumpgate/units/<app>/<unit>";

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

impl FromStr for UnitPath {
    type Err = RecordError;

    /// Accepts `/jumpgate/units/<app>/<unit>` with a lowercase unit.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::RecordPath`] for any other text.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let [app, unit] = segments(text, UNITS, UNIT_SHAPE)?;
        Self::new(segment(app, UNIT_SHAPE)?, segment(unit, UNIT_SHAPE)?)
            .map_err(|_| refused(UNIT_SHAPE))
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
        } else if text.starts_with(UNITS) {
            text.parse().map(Self::Unit)
        } else {
            Err(RecordError::new(
                Reason::RecordPath,
                "the path is not under /jumpgate/needs/, /jumpgate/offers/, /jumpgate/bindings/, /jumpgate/targets/ or /jumpgate/units/",
            ))
        }
    }
}
