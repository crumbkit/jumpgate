//! The four record types: [`Need`], [`Offer`], [`Binding`] and [`Target`].
//!
//! Each record's JSON form starts with `"$type"`, such as
//! `"jumpgate/need/v1"`. [`Record::from_json`] reads a record's text, refuses
//! a field the type does not have, a missing field and a `$type` that names
//! another type, and parses every string into its own type. Each refusal is
//! a [`RecordError`] whose code is `jumpgate::record::<reason>`.
//!
//! The JSON Schema of each type, draft 2020-12, comes from the Rust types
//! through [`schema`]. The committed copies live in the crate's
//! `schemas/jumpgate/<kind>/v1.schema.json`, and a test fails when they
//! differ from what [`schema`] returns.

mod binding;
mod need;
mod offer;
mod target;

use std::fmt;

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::Serialize;
use serde_json::Value;

pub use crate::record::binding::Binding;
pub use crate::record::need::{Condition, DeclaredIn, DeliversAs, Need, WhenUnbound};
pub use crate::record::offer::{
    Offer, PortReference, Provider, Reference, RouteReference, UnitReference,
};
pub use crate::record::target::Target;

use crate::error::{Reason, RecordError};
use crate::json::{Object, text as text_of};
use crate::path::RecordPath;

#[cfg(test)]
mod tests;

/// The most bytes a record's text holds. A longer text is refused with
/// [`Reason::TooLarge`] before it is parsed.
pub const MAX_RECORD_BYTES: usize = 64 * 1024;

/// One of the four record types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RecordType {
    /// `jumpgate/need/v1`.
    Need,
    /// `jumpgate/offer/v1`.
    Offer,
    /// `jumpgate/binding/v1`.
    Binding,
    /// `jumpgate/target/v1`.
    Target,
}

impl RecordType {
    /// Every record type.
    pub const ALL: &'static [Self] = &[Self::Need, Self::Offer, Self::Binding, Self::Target];

    /// Returns the type's `$type`, such as `jumpgate/need/v1`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Need => "jumpgate/need/v1",
            Self::Offer => "jumpgate/offer/v1",
            Self::Binding => "jumpgate/binding/v1",
            Self::Target => "jumpgate/target/v1",
        }
    }

    /// Returns the type's kind, the middle part of its `$type`, such as
    /// `need`.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Need => "need",
            Self::Offer => "offer",
            Self::Binding => "binding",
            Self::Target => "target",
        }
    }

    /// Returns the path of the type's committed schema within the crate's
    /// `schemas` directory, such as `jumpgate/need/v1.schema.json`.
    #[must_use]
    pub const fn schema_file(self) -> &'static str {
        match self {
            Self::Need => "jumpgate/need/v1.schema.json",
            Self::Offer => "jumpgate/offer/v1.schema.json",
            Self::Binding => "jumpgate/binding/v1.schema.json",
            Self::Target => "jumpgate/target/v1.schema.json",
        }
    }
}

impl fmt::Display for RecordType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A jumpgate record, which a store reads and writes at its path.
pub trait Record: Serialize + JsonSchema + Sized {
    /// The record's type.
    const TYPE: RecordType;

    /// Reads a record from its JSON text.
    ///
    /// # Errors
    ///
    /// Returns a [`RecordError`] when the text is too long or not JSON,
    /// when its `$type` names another type, when it lacks a field or holds
    /// one the type does not have, and when a value breaks its rule.
    fn from_json(text: &str) -> Result<Self, RecordError>;

    /// Returns the record's path in a store.
    fn record_path(&self) -> RecordPath;
}

/// Returns the JSON Schema, draft 2020-12, of `record_type`, with every
/// subschema inline.
#[must_use]
pub fn schema(record_type: RecordType) -> schemars::Schema {
    let generator = SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator();
    match record_type {
        RecordType::Need => generator.into_root_schema_for::<Need>(),
        RecordType::Offer => generator.into_root_schema_for::<Offer>(),
        RecordType::Binding => generator.into_root_schema_for::<Binding>(),
        RecordType::Target => generator.into_root_schema_for::<Target>(),
    }
}

/// Parses `text` as JSON and checks that it is an object whose `$type` is
/// `record_type`. The caller then reads the fields from the value.
///
/// serde_json keeps the last of two fields with the same key, so a record
/// that repeats a key is read with the later value. A reader that refuses
/// repeated keys needs a parser that reports them, which replaces this one
/// if a store is found to write them.
fn parse(text: &str, record_type: RecordType) -> Result<Value, RecordError> {
    if text.len() > MAX_RECORD_BYTES {
        return Err(RecordError::new(
            Reason::TooLarge,
            format!(
                "the record has {} bytes, and the limit is {MAX_RECORD_BYTES}",
                text.len()
            ),
        ));
    }
    let value: Value = serde_json::from_str(text).map_err(|error| {
        RecordError::new(
            Reason::Syntax,
            format!(
                "the record is not JSON, at line {} column {}",
                error.line(),
                error.column()
            ),
        )
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| RecordError::new(Reason::Shape, "the record is not a JSON object"))?;
    let found = object.get("$type").ok_or_else(|| {
        RecordError::new(Reason::MissingField, "the record lacks its $type").within("$type")
    })?;
    if text_of(found).map_err(|error| error.within("$type"))? != record_type.as_str() {
        return Err(RecordError::new(
            Reason::WrongType,
            format!("the record's $type is not {record_type}, the type this reader reads"),
        )
        .within("$type"));
    }
    Ok(value)
}

/// Opens the fields of a record, whose names are `known` beside `$type`.
fn fields<'a>(value: &'a Value, known: &[&str]) -> Result<Object<'a>, RecordError> {
    let mut all = vec!["$type"];
    all.extend_from_slice(known);
    Object::open(value, &all)
}

/// Declares the zero-sized type of a record's `$type` field, whose JSON
/// form is the record type's name.
macro_rules! type_tag {
    ($name:ident, $record_type:expr) => {
        /// The `$type` field of the record, which always holds the record
        /// type's name.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub(crate) struct $name;

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str($record_type.as_str())
            }
        }

        impl schemars::JsonSchema for $name {
            fn inline_schema() -> bool {
                true
            }

            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({
                    "type": "string",
                    "const": $record_type.as_str()
                })
            }
        }
    };
}

pub(crate) use type_tag;
