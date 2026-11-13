//! The reader that walks a record's JSON value field by field, so each
//! refusal carries its own [`Reason`] and the field where it was found.
//!
//! serde's derived readers would quote a refused string in some messages,
//! such as `unknown variant` or `invalid type: string "..."`, and would fold
//! every refusal into one error kind. Walking a [`serde_json::Value`] keeps
//! both under the library's control.

use serde_json::{Map, Value};

use crate::error::{Reason, RecordError};

#[cfg(test)]
mod tests;

/// A type that the record reader builds from a JSON value.
pub(crate) trait FromJson: Sized {
    /// Builds the value from `value`.
    fn from_json(value: &Value) -> Result<Self, RecordError>;
}

impl FromJson for bool {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        value
            .as_bool()
            .ok_or_else(|| shape("the value is not a boolean"))
    }
}

/// Returns the [`Reason::Shape`] error with `message`.
pub(crate) fn shape(message: &'static str) -> RecordError {
    RecordError::new(Reason::Shape, message)
}

/// Returns the text of `value`, or the shape error when it is not a string.
pub(crate) fn text(value: &Value) -> Result<&str, RecordError> {
    value
        .as_str()
        .ok_or_else(|| shape("the value is not a string"))
}

/// Returns the one entry of `value`, an object that holds exactly one of
/// `keys`.
///
/// # Errors
///
/// Returns [`Reason::Shape`] when `value` is not an object or holds no key
/// or more than one, and [`Reason::UnknownField`] when its key is not one of
/// `keys`.
pub(crate) fn one_key<'a>(
    value: &'a Value,
    keys: &'static [&'static str],
    message: &'static str,
) -> Result<(&'static str, &'a Value), RecordError> {
    let object = value.as_object().ok_or_else(|| shape(message))?;
    let mut entries = object.iter();
    let (Some((key, inner)), None) = (entries.next(), entries.next()) else {
        return Err(shape(message));
    };
    let known = keys
        .iter()
        .find(|known| **known == key.as_str())
        .ok_or_else(|| unknown_field(keys))?;
    Ok((known, inner))
}

/// Returns the [`Reason::UnknownField`] error for an object whose fields
/// are among `known` and which holds another field.
///
/// The message names the fields the object may hold and never the field it
/// holds, because a key can be a value pasted into the wrong place, such as
/// a token, and a token can look like a field name.
fn unknown_field(known: &[&str]) -> RecordError {
    RecordError::new(
        Reason::UnknownField,
        format!(
            "the object holds a field its type does not have; its fields are {}",
            known.join(", ")
        ),
    )
}

/// The fields of one JSON object, which a reader takes one at a time.
pub(crate) struct Object<'a> {
    map: &'a Map<String, Value>,
}

impl<'a> Object<'a> {
    /// Opens `value` as an object whose fields are among `known`.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::Shape`] when `value` is not an object, and
    /// [`Reason::UnknownField`] when it holds a field that `known` does not
    /// list.
    pub(crate) fn open(value: &'a Value, known: &[&str]) -> Result<Self, RecordError> {
        let map = value
            .as_object()
            .ok_or_else(|| shape("the value is not a JSON object"))?;
        if map.keys().any(|key| !known.contains(&key.as_str())) {
            return Err(unknown_field(known));
        }
        Ok(Self { map })
    }

    /// Reads the field `key`, which the object must hold.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::MissingField`] when the object lacks `key`, and the
    /// value's own error, placed at `key`, when it is refused.
    pub(crate) fn required<T: FromJson>(&self, key: &'static str) -> Result<T, RecordError> {
        let value = self.map.get(key).ok_or_else(|| {
            RecordError::new(Reason::MissingField, "the object lacks this field").within(key)
        })?;
        T::from_json(value).map_err(|error| error.within(key))
    }

    /// Reads the field `key`, which the object may leave out. A `null`
    /// value is refused, because the writer leaves an empty field out.
    ///
    /// # Errors
    ///
    /// Returns the value's own error, placed at `key`, when it is refused.
    pub(crate) fn optional<T: FromJson>(
        &self,
        key: &'static str,
    ) -> Result<Option<T>, RecordError> {
        self.map
            .get(key)
            .map(|value| T::from_json(value).map_err(|error| error.within(key)))
            .transpose()
    }
}
