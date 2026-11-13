//! The failure codes of the library and the error that a refused value or
//! record returns.
//!
//! No message quotes the text it refuses. A value that belongs in one field
//! may be a secret typed into the wrong place, such as a token where a
//! secret-store reference belongs, so a message names only the field, the
//! rule and, where it helps, a length or a byte offset. A message about an
//! unknown field names the fields the object may hold and never the key it
//! holds, because a key may be a pasted token too.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;

#[cfg(test)]
mod tests;

/// A stable failure code, written `jumpgate::<area>::<name>`.
///
/// The area is a noun from jumpgate's domain, such as `record`, and never a
/// module or a verb, so moving code never changes a code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Code {
    area: &'static str,
    name: &'static str,
}

impl Code {
    /// Builds the code `jumpgate::<area>::<name>`.
    #[must_use]
    pub const fn new(area: &'static str, name: &'static str) -> Self {
        Self { area, name }
    }

    /// Returns the code's area, such as `record`.
    #[must_use]
    pub const fn area(&self) -> &'static str {
        self.area
    }

    /// Returns the code's name within its area, such as `secret_reference`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl fmt::Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "jumpgate::{}::{}", self.area, self.name)
    }
}

/// Declares [`Reason`], its codes and its list of every reason.
macro_rules! reasons {
    ($($(#[$doc:meta])* $variant:ident => $name:literal,)*) => {
        /// Why a value or a record was refused. Each reason has the code
        /// `jumpgate::record::<name>`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum Reason {
            $($(#[$doc])* $variant,)*
        }

        impl Reason {
            /// Every reason, in the order the codes are documented.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// Returns the reason's failure code.
            #[must_use]
            pub const fn code(self) -> Code {
                match self {
                    $(Self::$variant => Code::new("record", $name),)*
                }
            }
        }
    };
}

reasons! {
    /// The record's text is not JSON.
    Syntax => "syntax",
    /// The record's text is longer than [`crate::record::MAX_RECORD_BYTES`].
    TooLarge => "too_large",
    /// A value has the wrong JSON type, such as a number where a string
    /// belongs, or an object without the one key it needs.
    Shape => "shape",
    /// The record's `$type` names another record type.
    WrongType => "wrong_type",
    /// An object holds a field its type does not have.
    UnknownField => "unknown_field",
    /// An object lacks a field its type needs.
    MissingField => "missing_field",
    /// A name, such as an app, unit, need, target or board name, breaks its
    /// rule.
    Name => "name",
    /// An environment variable's name breaks its rule.
    Variable => "variable",
    /// A repository id is not `host/owner/name`.
    Repository => "repository",
    /// A file path is empty, too long, or holds an empty, `.` or `..`
    /// segment or a character a path never holds.
    FilePath => "file_path",
    /// A record path does not have the shape of the record type's path.
    RecordPath => "record_path",
    /// A secret-store reference is not `op://<vault>/<item>/<field>` or
    /// `op://<vault>/<item>/<section>/<field>`.
    SecretReference => "secret_reference",
    /// A value is not one of the words its field takes, such as a need kind.
    Keyword => "keyword",
    /// A list repeats an entry.
    Duplicate => "duplicate",
    /// A list that needs an entry is empty.
    Empty => "empty",
    /// An offer's reference does not fit its kind or its provider, or no
    /// reference variant exists yet for its kind or its provider's kind.
    ReferenceMismatch => "reference_mismatch",
    /// A fallback value is too long or holds a NUL character.
    Fallback => "fallback",
    /// A need's `optional` flag contradicts its `when_unbound`: an optional
    /// need fails when unbound, or a required need is omitted.
    OptionalConflict => "optional_conflict",
}

/// The error of a refused value or record.
///
/// It carries the [`Reason`], whose [`Code`] is stable, the field where the
/// value was found, such as `declared_in.repo` or `consumers[2]`, and a
/// message that never quotes the refused text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordError {
    reason: Reason,
    field: String,
    message: Cow<'static, str>,
}

impl RecordError {
    /// Builds an error for `reason` with `message`, at no field.
    pub(crate) fn new(reason: Reason, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            reason,
            field: String::new(),
            message: message.into(),
        }
    }

    /// Returns why the value was refused.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        self.reason
    }

    /// Returns the failure code, `jumpgate::record::<reason>`.
    #[must_use]
    pub const fn code(&self) -> Code {
        self.reason.code()
    }

    /// Returns the field that holds the refused value, such as
    /// `declared_in.repo` or `consumers[2]`, or `None` when the error is
    /// about the whole text.
    #[must_use]
    pub fn field(&self) -> Option<&str> {
        (!self.field.is_empty()).then_some(self.field.as_str())
    }

    /// Places the error inside the object field `key`.
    pub(crate) fn within(mut self, key: &str) -> Self {
        self.field = if self.field.is_empty() {
            key.to_owned()
        } else if self.field.starts_with('[') {
            format!("{key}{}", self.field)
        } else {
            format!("{key}.{}", self.field)
        };
        self
    }

    /// Places the error inside the list entry at `index`.
    pub(crate) fn at_index(mut self, index: usize) -> Self {
        self.field = if self.field.is_empty() || self.field.starts_with('[') {
            format!("[{index}]{}", self.field)
        } else {
            format!("[{index}].{}", self.field)
        };
        self
    }
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.field.is_empty() {
            formatter.write_str(&self.message)
        } else {
            write!(formatter, "{}: {}", self.field, self.message)
        }
    }
}

impl Error for RecordError {}
