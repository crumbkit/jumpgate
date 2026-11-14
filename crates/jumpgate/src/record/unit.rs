//! The unit record, `jumpgate/unit/v1`.

use schemars::JsonSchema;
use serde::Serialize;

use crate::distinct::Distinct;
use crate::error::RecordError;
use crate::keyword::UnitKind;
use crate::name::{AppName, PortName, UnitName};
use crate::path::{RecordPath, UnitPath};
use crate::record::{Record, RecordType, fields, parse, type_tag};
use crate::repo::RepoId;

#[cfg(test)]
mod tests;

type_tag!(UnitType, RecordType::Unit);

/// One process or container that an app runs, which a plan lists as a unit.
/// Its path is `/jumpgate/units/<app>/<unit>`.
///
/// mxv allows uppercase letters in a unit's name, and a record path holds
/// none, so a unit's name is lowercase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    /// The record type, jumpgate/unit/v1.
    #[serde(rename = "$type")]
    record_type: UnitType,
    /// The app that runs the unit.
    app: AppName,
    /// The unit's name within its app. A record path holds no uppercase
    /// letter, a rule that this schema does not express: the name holds no
    /// uppercase letter.
    name: UnitName,
    /// The repository the app comes from.
    repo: RepoId,
    /// Whether the unit is a process or a container.
    kind: UnitKind,
    /// The units of the same app that start before this one.
    starts_after: Distinct<UnitName>,
    /// The names of the ports the unit serves.
    ports: Distinct<PortName>,
}

impl Unit {
    /// Builds the unit `name` of `app`, which comes from `repo` and is of
    /// kind `kind`. It starts after no unit and serves no port;
    /// [`Unit::with_starts_after`] and [`Unit::with_ports`] set these.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::Name`](crate::Reason::Name) when `name` holds an uppercase letter,
    /// which no record path holds.
    pub fn new(
        app: AppName,
        name: UnitName,
        repo: RepoId,
        kind: UnitKind,
    ) -> Result<Self, RecordError> {
        UnitPath::new(app.clone(), name.clone())?;
        Ok(Self {
            record_type: UnitType,
            app,
            name,
            repo,
            kind,
            starts_after: Distinct::default(),
            ports: Distinct::default(),
        })
    }

    /// Returns the unit that starts after the units `starts_after`.
    #[must_use]
    pub fn with_starts_after(mut self, starts_after: Distinct<UnitName>) -> Self {
        self.starts_after = starts_after;
        self
    }

    /// Returns the unit that serves the ports `ports`.
    #[must_use]
    pub fn with_ports(mut self, ports: Distinct<PortName>) -> Self {
        self.ports = ports;
        self
    }

    /// Returns the app that runs the unit.
    #[must_use]
    pub const fn app(&self) -> &AppName {
        &self.app
    }

    /// Returns the unit's name.
    #[must_use]
    pub const fn name(&self) -> &UnitName {
        &self.name
    }

    /// Returns the repository the app comes from.
    #[must_use]
    pub const fn repo(&self) -> &RepoId {
        &self.repo
    }

    /// Returns whether the unit is a process or a container.
    #[must_use]
    pub const fn kind(&self) -> UnitKind {
        self.kind
    }

    /// Returns the units of the same app that start before this one.
    #[must_use]
    pub const fn starts_after(&self) -> &Distinct<UnitName> {
        &self.starts_after
    }

    /// Returns the names of the ports the unit serves.
    #[must_use]
    pub const fn ports(&self) -> &Distinct<PortName> {
        &self.ports
    }

    /// Returns the unit's path, `/jumpgate/units/<app>/<unit>`.
    #[must_use]
    pub fn path(&self) -> UnitPath {
        UnitPath::from_checked(self.app.clone(), self.name.clone())
    }
}

impl Record for Unit {
    const TYPE: RecordType = RecordType::Unit;

    fn from_json(text: &str) -> Result<Self, RecordError> {
        let value = parse(text, Self::TYPE)?;
        let object = fields(
            &value,
            &["app", "name", "repo", "kind", "starts_after", "ports"],
        )?;
        let app: AppName = object.required("app")?;
        let name: UnitName = object.required("name")?;
        let repo = object.required("repo")?;
        let kind = object.required("kind")?;
        let starts_after = object.required("starts_after")?;
        let ports = object.required("ports")?;
        let unit = Self::new(app, name, repo, kind).map_err(|error| error.within("name"))?;
        Ok(unit.with_starts_after(starts_after).with_ports(ports))
    }

    fn record_path(&self) -> RecordPath {
        self.path().into()
    }
}
