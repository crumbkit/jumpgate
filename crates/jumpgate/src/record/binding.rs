//! The binding record, `jumpgate/binding/v1`.

use schemars::JsonSchema;
use serde::Serialize;

use crate::error::RecordError;
use crate::name::{Audience, Projection, TargetName};
use crate::path::{BindingPath, NeedPath, ProvidePath, RecordPath};
use crate::record::{Record, RecordType, fields, parse, type_tag};

#[cfg(test)]
mod tests;

type_tag!(BindingType, RecordType::Binding);

/// A need matched to a provide for one target. Its path is
/// `/jumpgate/bindings/<target>/<app>/<need>`.
///
/// A binding names the need and the provide by their paths and holds no
/// value, so no field can hold a secret. It says nothing about delivery:
/// the plan delivers the value as the need's `delivers_as` says. It is
/// active as soon as it is written, and it counts only in the board that
/// its target names as its authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// The record type, jumpgate/binding/v1.
    #[serde(rename = "$type")]
    record_type: BindingType,
    /// The need that the binding meets.
    need: NeedPath,
    /// The provide that meets the need.
    provide: ProvidePath,
    /// The target the binding is for.
    target: TargetName,
    /// The part of the provide's value that the need receives, when it
    /// receives only a part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Projection")]
    projection: Option<Projection>,
    /// The audience the credential is issued for, when the provide issues
    /// one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Audience")]
    audience: Option<Audience>,
}

impl Binding {
    /// Builds the binding of the need at `need` to the provide at `provide` for
    /// `target`, with no projection and no audience.
    #[must_use]
    pub const fn new(need: NeedPath, provide: ProvidePath, target: TargetName) -> Self {
        Self {
            record_type: BindingType,
            need,
            provide,
            target,
            projection: None,
            audience: None,
        }
    }

    /// Returns the binding with `projection` as the part of the provide's
    /// value it delivers.
    #[must_use]
    pub fn with_projection(mut self, projection: Projection) -> Self {
        self.projection = Some(projection);
        self
    }

    /// Returns the binding with `audience` as the audience of its
    /// credential.
    #[must_use]
    pub fn with_audience(mut self, audience: Audience) -> Self {
        self.audience = Some(audience);
        self
    }

    /// Returns the need's path.
    #[must_use]
    pub const fn need(&self) -> &NeedPath {
        &self.need
    }

    /// Returns the provide's path.
    #[must_use]
    pub const fn provide(&self) -> &ProvidePath {
        &self.provide
    }

    /// Returns the target.
    #[must_use]
    pub const fn target(&self) -> &TargetName {
        &self.target
    }

    /// Returns the projection, if any.
    #[must_use]
    pub const fn projection(&self) -> Option<&Projection> {
        self.projection.as_ref()
    }

    /// Returns the audience, if any.
    #[must_use]
    pub const fn audience(&self) -> Option<&Audience> {
        self.audience.as_ref()
    }

    /// Returns the binding's path, `/jumpgate/bindings/<target>/<app>/<need>`.
    #[must_use]
    pub fn path(&self) -> BindingPath {
        BindingPath::new(
            self.target.clone(),
            self.need.app().clone(),
            self.need.name().clone(),
        )
    }
}

impl Record for Binding {
    const TYPE: RecordType = RecordType::Binding;

    fn from_json(text: &str) -> Result<Self, RecordError> {
        let value = parse(text, Self::TYPE)?;
        let object = fields(
            &value,
            &["need", "provide", "target", "projection", "audience"],
        )?;
        Ok(Self {
            record_type: BindingType,
            need: object.required("need")?,
            provide: object.required("provide")?,
            target: object.required("target")?,
            projection: object.optional("projection")?,
            audience: object.optional("audience")?,
        })
    }

    fn record_path(&self) -> RecordPath {
        self.path().into()
    }
}
