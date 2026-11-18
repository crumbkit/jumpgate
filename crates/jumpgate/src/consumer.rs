//! What receives a need: a unit of its app, or one of the app's commands.

use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::error::RecordError;
use crate::json::{FromJson, Object};
use crate::name::{TaskName, UnitName};

#[cfg(test)]
mod tests;

/// A consumer of a need.
///
/// A unit is an mxv process or container that runs while the app runs. A
/// command is an mxv task, which a person or a tool runs by name. In JSON a
/// unit is its name as a string, and a command is an object,
/// `{"command": "<task name>"}`.
///
/// A unit and a command may share a name, so the form tells them apart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(untagged)]
pub enum Consumer {
    /// A process or container of the app.
    Unit(UnitName),
    /// A task of the app.
    Command {
        /// The task's name.
        command: TaskName,
    },
}

impl Consumer {
    /// Returns the consumer that is the task `name`.
    #[must_use]
    pub const fn command(name: TaskName) -> Self {
        Self::Command { command: name }
    }

    /// Returns the unit's name, or `None` for a command.
    #[must_use]
    pub const fn unit(&self) -> Option<&UnitName> {
        match self {
            Self::Unit(name) => Some(name),
            Self::Command { .. } => None,
        }
    }
}

impl fmt::Display for Consumer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit(name) => write!(formatter, "{name}"),
            Self::Command { command } => write!(formatter, "command {command}"),
        }
    }
}

impl JsonSchema for Consumer {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Consumer".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "What receives a need: a unit of its app, written as the unit's name, or one of the app's commands, written as an object with the task's name under `command`.",
            "anyOf": [
                generator.subschema_for::<UnitName>(),
                {
                    "type": "object",
                    "properties": {"command": generator.subschema_for::<TaskName>()},
                    "required": ["command"],
                    "additionalProperties": false
                }
            ]
        })
    }
}

impl FromJson for Consumer {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        if value.is_object() {
            let object = Object::open(value, &["command"])?;
            let command = object.required("command")?;
            return Ok(Self::Command { command });
        }
        UnitName::from_json(value).map(Self::Unit)
    }
}
