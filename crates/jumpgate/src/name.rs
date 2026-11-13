//! The names that records hold: apps, units, ports, needs, targets,
//! providers, offers, profiles, boards, projections and audiences.
//!
//! Two rules cover every name.
//!
//! - The lowercase rule: 1 to [`MAX_NAME_BYTES`] lowercase ASCII letters,
//!   digits, `-` and `_`, starting with a letter or a digit. It is the rule
//!   of mxv's app names, and every name that is a segment of a record path
//!   follows it, so two paths never differ only in case on a file system
//!   that ignores case.
//! - The unit rule: the same, with uppercase letters allowed. It is the rule
//!   that mxv applies to its processes, containers and ports, so every unit
//!   and port that mxv accepts has a name here.

use crate::error::{Reason, RecordError};

#[cfg(test)]
mod tests;

/// The most bytes a name holds.
pub const MAX_NAME_BYTES: usize = 64;

/// The two rules a name follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// Lowercase letters, digits, `-` and `_`.
    Lowercase,
    /// Letters of either case, digits, `-` and `_`.
    Unit,
}

/// Checks `text`, a name of the kind `what`, such as "an app name",
/// against `rule`.
fn check(text: &str, rule: Rule, what: &'static str) -> Result<(), RecordError> {
    let refuse = |problem: String| {
        Err(RecordError::new(
            Reason::Name,
            format!("{what} name {problem}"),
        ))
    };
    let Some(first) = text.chars().next() else {
        return refuse("is empty".to_owned());
    };
    if text.len() > MAX_NAME_BYTES {
        return refuse(format!(
            "has {} bytes, and the limit is {MAX_NAME_BYTES}",
            text.len()
        ));
    }
    let allowed = |character: char| match rule {
        Rule::Lowercase => character.is_ascii_lowercase() || character.is_ascii_digit(),
        Rule::Unit => character.is_ascii_alphanumeric(),
    };
    let (letter, letters) = match rule {
        Rule::Lowercase => ("lowercase letter", "lowercase letters"),
        Rule::Unit => ("letter", "letters"),
    };
    if !allowed(first) {
        return refuse(format!(
            "starts with a character other than a {letter} or a digit"
        ));
    }
    if let Some((at, _)) = text
        .char_indices()
        .find(|(_, character)| !(allowed(*character) || matches!(character, '-' | '_')))
    {
        return refuse(format!(
            "holds a character at byte {at} other than {letters}, digits, '-' and '_'"
        ));
    }
    Ok(())
}

/// Declares one name type that follows the lowercase rule.
macro_rules! lowercase_name {
    ($(#[$doc:meta])* $name:ident, $check:ident, $what:literal) => {
        fn $check(text: &str) -> Result<(), RecordError> {
            check(text, Rule::Lowercase, $what)
        }

        text_type! {
            $(#[$doc])*
            $name, $check, {
                "type": "string",
                "minLength": 1,
                "maxLength": MAX_NAME_BYTES,
                "pattern": "^[a-z0-9][a-z0-9_-]*$",
                "description": concat!(
                    "Names ", $what, ": 1 to 64 lowercase letters, digits, '-' and '_', ",
                    "starting with a letter or a digit."
                )
            }
        }
    };
}

/// Declares one name type that follows the unit rule.
macro_rules! unit_name {
    ($(#[$doc:meta])* $name:ident, $check:ident, $what:literal) => {
        fn $check(text: &str) -> Result<(), RecordError> {
            check(text, Rule::Unit, $what)
        }

        text_type! {
            $(#[$doc])*
            $name, $check, {
                "type": "string",
                "minLength": 1,
                "maxLength": MAX_NAME_BYTES,
                "pattern": "^[A-Za-z0-9][A-Za-z0-9_-]*$",
                "description": concat!(
                    "Names ", $what, ": 1 to 64 letters, digits, '-' and '_', ",
                    "starting with a letter or a digit."
                )
            }
        }
    };
}

lowercase_name! {
    /// The name of an app, such as `shelf`. It follows the lowercase rule.
    ///
    /// ```
    /// let app: jumpgate::AppName = "shelf".parse()?;
    /// assert_eq!(app.as_str(), "shelf");
    /// assert!("Shelf".parse::<jumpgate::AppName>().is_err());
    /// # Ok::<(), jumpgate::RecordError>(())
    /// ```
    AppName, app_name, "an app"
}

lowercase_name! {
    /// The name of a need within its app, such as `auth-api` for a want or
    /// `api_token` for a secret. It follows the lowercase rule.
    NeedName, need_name, "a need"
}

lowercase_name! {
    /// The name of a target, such as `local`. It follows the lowercase rule.
    TargetName, target_name, "a target"
}

lowercase_name! {
    /// The name of a provider within its kind, such as `one-password` for
    /// a secret store or `auth` for an app. It follows the lowercase rule.
    ProviderName, provider_name, "a provider"
}

lowercase_name! {
    /// The name of an offer within its provider, such as `auth-api`. It
    /// follows the lowercase rule.
    OfferName, offer_name, "an offer"
}

lowercase_name! {
    /// The name of a profile that a need's `when` names, such as `dev`. It
    /// follows the lowercase rule.
    ProfileName, profile_name, "a profile"
}

lowercase_name! {
    /// The id of a board, which a target names as the authority for its
    /// bindings, such as `personal`. It follows the lowercase rule.
    BoardId, board_id, "a board"
}

lowercase_name! {
    /// The part of an offer's value that a binding delivers, such as
    /// `public-key`. It follows the lowercase rule.
    Projection, projection, "a projection"
}

lowercase_name! {
    /// The audience that a binding's credential is issued for, such as
    /// `ledger`. It follows the lowercase rule.
    Audience, audience, "an audience"
}

unit_name! {
    /// The name of a unit that an app runs, an mxv process or container,
    /// such as `api`. It follows the unit rule.
    UnitName, unit_name, "a unit"
}

unit_name! {
    /// The name of a port that an app declares, such as `http`. It follows
    /// the unit rule.
    PortName, port_name, "a port"
}
