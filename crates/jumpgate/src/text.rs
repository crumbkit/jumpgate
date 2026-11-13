//! The macro that gives each text type of the library its checking
//! constructor, accessor, `Display`, JSON form and JSON Schema.

/// Implements a text type that wraps a `String` checked by `$check`.
///
/// `$check` takes the text and returns `Result<(), RecordError>`.
/// `$schema` is the JSON Schema of the type's text form. [`FromStr`] is the
/// only way to build the type, and the record reader builds it through
/// [`FromStr`] too, so every value of the type has passed `$check`.
///
/// [`FromStr`]: std::str::FromStr
macro_rules! text_type {
    (
        $(#[$doc:meta])*
        $name:ident, $check:path, $schema:tt
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Returns the value as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::error::RecordError;

            /// Accepts `text` when it follows the type's rule.
            ///
            /// # Errors
            ///
            /// Returns a [`RecordError`](crate::RecordError) that names the
            /// rule `text` breaks, without quoting `text`.
            fn from_str(text: &str) -> Result<Self, Self::Err> {
                $check(text)?;
                Ok(Self(text.to_owned()))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
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
                schemars::json_schema!($schema)
            }
        }

        impl $crate::json::FromJson for $name {
            fn from_json(value: &serde_json::Value) -> Result<Self, $crate::error::RecordError> {
                $crate::json::text(value)?.parse()
            }
        }
    };
}
