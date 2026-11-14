//! The JSON form and the JSON Schema of the paths that record fields
//! reference.

use std::borrow::Cow;

use serde::Serialize;
use serde_json::Value;

use crate::error::RecordError;
use crate::json::{FromJson, text};
use crate::path::{NeedPath, OfferPath};

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
