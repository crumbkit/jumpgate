//! A list that holds each entry once.

use std::borrow::Cow;

use serde::Serialize;
use serde_json::Value;

use crate::error::{Reason, RecordError};
use crate::json::{FromJson, shape};

#[cfg(test)]
mod tests;

/// A list of entries in which no entry repeats, holding at least `MIN`
/// entries.
///
/// The order is the order the entries were given in, so a record's text is
/// the same each time it is written.
///
/// ```
/// use jumpgate::{Distinct, UnitName};
///
/// let api: UnitName = "api".parse()?;
/// let worker: UnitName = "worker".parse()?;
/// let units: Distinct<UnitName> = Distinct::new(vec![api.clone(), worker])?;
/// assert_eq!(units.len(), 2);
/// assert!(Distinct::<UnitName>::new(vec![api.clone(), api]).is_err());
/// # Ok::<(), jumpgate::RecordError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Distinct<T, const MIN: usize = 0>(Vec<T>);

impl<T: PartialEq, const MIN: usize> Distinct<T, MIN> {
    /// Builds the list from `entries`.
    ///
    /// # Errors
    ///
    /// Returns [`Reason::Duplicate`] at the first entry that repeats an
    /// earlier one, and [`Reason::Empty`] when `entries` holds fewer than
    /// `MIN` entries.
    pub fn new(entries: Vec<T>) -> Result<Self, RecordError> {
        if let Some(index) = first_repeat(&entries) {
            return Err(RecordError::new(
                Reason::Duplicate,
                "the entry repeats an earlier entry of the list",
            )
            .at_index(index));
        }
        if entries.len() < MIN {
            return Err(too_few(entries.len(), MIN));
        }
        Ok(Self(entries))
    }
}

impl<T, const MIN: usize> Distinct<T, MIN> {
    /// Returns the entries in order.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Returns an iterator over the entries in order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Reports whether the list holds no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> Default for Distinct<T, 0> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<'a, T, const MIN: usize> IntoIterator for &'a Distinct<T, MIN> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Serialize, const MIN: usize> Serialize for Distinct<T, MIN> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<T: schemars::JsonSchema, const MIN: usize> schemars::JsonSchema for Distinct<T, MIN> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Distinct_{}_{MIN}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let items = generator.subschema_for::<T>();
        if MIN == 0 {
            schemars::json_schema!({
                "type": "array",
                "items": items,
                "uniqueItems": true
            })
        } else {
            schemars::json_schema!({
                "type": "array",
                "items": items,
                "uniqueItems": true,
                "minItems": MIN
            })
        }
    }
}

impl<T: FromJson + PartialEq, const MIN: usize> FromJson for Distinct<T, MIN> {
    fn from_json(value: &Value) -> Result<Self, RecordError> {
        let entries = value
            .as_array()
            .ok_or_else(|| shape("the value is not a JSON array"))?;
        let entries = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| T::from_json(entry).map_err(|error| error.at_index(index)))
            .collect::<Result<Vec<T>, RecordError>>()?;
        Self::new(entries)
    }
}

/// Returns the index of the first entry of `entries` that repeats an
/// earlier one. The lists a record holds are short, so the quadratic scan
/// costs less than hashing each entry.
fn first_repeat<T: PartialEq>(entries: &[T]) -> Option<usize> {
    entries
        .iter()
        .enumerate()
        .find(|(index, entry)| entries.iter().take(*index).any(|earlier| earlier == *entry))
        .map(|(index, _)| index)
}

/// Returns the error for a list with fewer than `min` entries.
fn too_few(len: usize, min: usize) -> RecordError {
    RecordError::new(
        Reason::Empty,
        format!("the list holds {len} entries, and it needs at least {min}"),
    )
}
