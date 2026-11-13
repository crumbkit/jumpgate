//! The target record, `jumpgate/target/v1`.

use schemars::JsonSchema;
use serde::Serialize;

use crate::error::RecordError;
use crate::keyword::TargetKind;
use crate::name::{BoardId, TargetName};
use crate::path::{RecordPath, TargetPath};
use crate::record::{Record, RecordType, fields, parse, type_tag};

#[cfg(test)]
mod tests;

type_tag!(TargetType, RecordType::Target);

/// A place a plan runs, and the board whose bindings count for it. Its path
/// is `/jumpgate/targets/<name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// The record type, jumpgate/target/v1.
    #[serde(rename = "$type")]
    record_type: TargetType,
    /// The target's name.
    name: TargetName,
    /// Where the target runs.
    kind: TargetKind,
    /// The board whose bindings count for the target. A binding for the
    /// target in any other board is reported and ignored.
    authority: BoardId,
}

impl Target {
    /// Builds the target `name` of kind `kind`, whose bindings count only in
    /// the board `authority`.
    #[must_use]
    pub const fn new(name: TargetName, kind: TargetKind, authority: BoardId) -> Self {
        Self {
            record_type: TargetType,
            name,
            kind,
            authority,
        }
    }

    /// Returns the target's name.
    #[must_use]
    pub const fn name(&self) -> &TargetName {
        &self.name
    }

    /// Returns where the target runs.
    #[must_use]
    pub const fn kind(&self) -> TargetKind {
        self.kind
    }

    /// Returns the board whose bindings count for the target.
    #[must_use]
    pub const fn authority(&self) -> &BoardId {
        &self.authority
    }

    /// Returns the target's path, `/jumpgate/targets/<name>`.
    #[must_use]
    pub fn path(&self) -> TargetPath {
        TargetPath::new(self.name.clone())
    }
}

impl Record for Target {
    const TYPE: RecordType = RecordType::Target;

    fn from_json(text: &str) -> Result<Self, RecordError> {
        let value = parse(text, Self::TYPE)?;
        let object = fields(&value, &["name", "kind", "authority"])?;
        Ok(Self::new(
            object.required("name")?,
            object.required("kind")?,
            object.required("authority")?,
        ))
    }

    fn record_path(&self) -> RecordPath {
        self.path().into()
    }
}
