//! The fields that take one of a fixed set of words: a need's kind, a
//! provider's kind, an offer's scope and a target's kind.

use crate::error::{Reason, RecordError};

#[cfg(test)]
mod tests;

/// Declares an enum whose JSON form is one word per variant.
macro_rules! keyword_enum {
    (
        $(#[$doc:meta])*
        $name:ident, $what:literal {
            $($(#[$variant_doc:meta])* $variant:ident => $word:literal,)*
        }
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($(#[$variant_doc])* $variant,)*
        }

        impl $name {
            /// Every variant, in the order the type lists them.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// Returns the variant's word, as the JSON form and a record
            /// path spell it.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $word,)*
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = RecordError;

            /// Accepts one of the type's words.
            ///
            /// # Errors
            ///
            /// Returns [`Reason::Keyword`] for any other text, without
            /// quoting it.
            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|variant| variant.as_str() == text)
                    .ok_or_else(|| {
                        let words: Vec<&str> = Self::ALL.iter().map(|variant| variant.as_str()).collect();
                        RecordError::new(
                            Reason::Keyword,
                            format!("{} is one of {}", $what, words.join(", ")),
                        )
                    })
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
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
                    "enum": [$($word),*]
                })
            }
        }

        impl $crate::json::FromJson for $name {
            fn from_json(value: &serde_json::Value) -> Result<Self, RecordError> {
                $crate::json::text(value)?.parse()
            }
        }
    };
}

keyword_enum! {
    /// What a need asks for, and what an offer supplies.
    NeedKind, "a need kind" {
        /// A secret, such as an API token, which a secret store holds.
        Secret => "secret",
        /// The address of another app's port or route.
        Address => "address",
        /// A value that is not secret, such as a setting.
        Value => "value",
        /// Access to another app's data.
        DataAccess => "data-access",
        /// A capability, such as the right to call a route.
        Capability => "capability",
        /// Another app's unit, which must run for the need to be met.
        App => "app",
    }
}

keyword_enum! {
    /// The kind of thing that makes an offer.
    ProviderKind, "a provider kind" {
        /// A secret store, which `op://` references name. It never runs as a
        /// unit.
        SecretStore => "secret-store",
        /// An app, which provides its ports, routes and units.
        App => "app",
        /// A workspace, which provides values its apps share.
        Workspace => "workspace",
        /// A capability provider.
        Capability => "capability",
        /// A cloud environment, such as an account's network.
        CloudEnvironment => "cloud-environment",
        /// An identity broker, which issues credentials for an audience.
        IdentityBroker => "identity-broker",
    }
}

keyword_enum! {
    /// How widely an offer's value is shared.
    ///
    /// A later version may add a scope, so a `match` outside the crate
    /// needs a wildcard arm:
    ///
    /// ```compile_fail,E0004
    /// fn word(scope: jumpgate::Scope) -> &'static str {
    ///     match scope {
    ///         jumpgate::Scope::AppInstance => "app-instance",
    ///         jumpgate::Scope::Workspace => "workspace",
    ///     }
    /// }
    /// ```
    #[non_exhaustive]
    Scope, "a scope" {
        /// Each running instance of an app has its own value.
        AppInstance => "app-instance",
        /// Every app in a workspace shares one value.
        Workspace => "workspace",
    }
}

keyword_enum! {
    /// What a unit is.
    UnitKind, "a unit kind" {
        /// An mxv process, which runs a command in the checkout.
        Process => "process",
        /// An mxv container, which runs an image.
        Container => "container",
    }
}

keyword_enum! {
    /// Where a target runs.
    TargetKind, "a target kind" {
        /// A machine a person works on, through mxv.
        Local => "local",
        /// A cloud environment, which no fallback value may reach.
        Cloud => "cloud",
    }
}
