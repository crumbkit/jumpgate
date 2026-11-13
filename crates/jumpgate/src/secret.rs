//! The reference to a secret in a secret store.

use crate::error::{Reason, RecordError};

#[cfg(test)]
mod tests;

/// Checks `text` against the rule of [`SecretReference`].
fn check(text: &str) -> Result<(), RecordError> {
    let fits = text.strip_prefix("op://").is_some_and(|rest| {
        let parts: Vec<&str> = rest.split('/').collect();
        (3..=4).contains(&parts.len()) && parts.iter().all(|part| part_fits(part))
    });
    if fits {
        Ok(())
    } else {
        Err(RecordError::new(
            Reason::SecretReference,
            "a secret-store reference is op://<vault>/<item>/<field> or \
             op://<vault>/<item>/<section>/<field>, and each part is not empty, '.' or '..' and \
             holds no control character",
        ))
    }
}

/// Reports whether `part` is a vault, item, section or field of a
/// reference: not empty, not `.` or `..`, and without a control character.
/// starmap's `OnePasswordRef` refuses an empty part and a control
/// character; this type also refuses `.` and `..`.
fn part_fits(part: &str) -> bool {
    !matches!(part, "" | "." | "..") && !part.chars().any(char::is_control)
}

/// The pattern of one part in the JSON Schema: one or more characters that
/// are not `/` or a control character (U+0000 to U+001F and U+007F to
/// U+009F, the characters that `char::is_control` names), and not `.` or
/// `..`.
macro_rules! part_pattern {
    () => {
        r"(?:[^/.\x00-\x1F\x7F-\x9F][^/\x00-\x1F\x7F-\x9F]*|\.[^/.\x00-\x1F\x7F-\x9F][^/\x00-\x1F\x7F-\x9F]*|\.\.[^/\x00-\x1F\x7F-\x9F]+)"
    };
}

/// The pattern of a reference in the JSON Schema.
const PATTERN: &str = concat!(
    "^op://",
    part_pattern!(),
    "/",
    part_pattern!(),
    "(?:/",
    part_pattern!(),
    ")?/",
    part_pattern!(),
    "$"
);

text_type! {
    /// A reference to a secret in a secret store, written
    /// `op://<vault>/<item>/<field>` or `op://<vault>/<item>/<section>/<field>`,
    /// such as `op://shelf-dev/api-token/credential`.
    ///
    /// A reference names where a secret lives, never the secret. The type
    /// accepts what starmap's `OnePasswordRef` accepts, so every reference
    /// that a starmap file binds can become an offer: three or four parts,
    /// each not empty and without a control character, so a part may hold
    /// spaces, apostrophes, letters outside ASCII and `?`. It also refuses
    /// a part that is `.` or `..`. Like starmap, it sets no length of its
    /// own; the record limit,
    /// [`MAX_RECORD_BYTES`](crate::record::MAX_RECORD_BYTES), bounds it.
    /// Any other text is refused, and the refusal never quotes the text,
    /// because a value typed where a reference belongs may be the secret
    /// itself.
    ///
    /// ```
    /// let reference: jumpgate::SecretReference = "op://shelf-dev/api-token/credential".parse()?;
    /// assert_eq!(reference.as_str(), "op://shelf-dev/api-token/credential");
    /// let refused = "hunter2".parse::<jumpgate::SecretReference>().unwrap_err();
    /// assert!(!refused.to_string().contains("hunter2"));
    /// # Ok::<(), jumpgate::RecordError>(())
    /// ```
    SecretReference, check, {
        "type": "string",
        "pattern": PATTERN,
        "description": "References a secret in a secret store as op://<vault>/<item>/<field> or op://<vault>/<item>/<section>/<field>; each part is not empty, '.' or '..' and holds no control character. It names where the secret lives and never holds the secret."
    }
}
