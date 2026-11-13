//! The name of an environment variable that a need is delivered as.

use crate::error::{Reason, RecordError};

#[cfg(test)]
mod tests;

/// The most bytes a variable name holds, as in starmap.
pub const MAX_VARIABLE_BYTES: usize = 128;

/// Checks `text` against the rule of [`VariableName`].
fn check(text: &str) -> Result<(), RecordError> {
    let refuse = |problem: String| {
        Err(RecordError::new(
            Reason::Variable,
            format!("a variable name {problem}"),
        ))
    };
    if text.is_empty() {
        return refuse("is empty".to_owned());
    }
    if text.len() > MAX_VARIABLE_BYTES {
        return refuse(format!(
            "has {} bytes, and the limit is {MAX_VARIABLE_BYTES}",
            text.len()
        ));
    }
    if let Some((at, _)) = text.char_indices().find(|(at, character)| {
        !(*character == '_'
            || character.is_ascii_alphabetic()
            || (*at > 0 && character.is_ascii_digit()))
    }) {
        return refuse(format!(
            "holds a character at byte {at} that is not an ASCII letter, a digit or '_', or starts with a digit"
        ));
    }
    Ok(())
}

text_type! {
    /// The name of an environment variable, such as `AUTH_API_URL`.
    ///
    /// A name matches `[A-Za-z_][A-Za-z0-9_]*` and has at most
    /// [`MAX_VARIABLE_BYTES`] bytes, which is the rule of starmap's `Name`,
    /// so every variable a `starmap.toml` declares has a name here. A name
    /// keeps its case.
    ///
    /// ```
    /// let name: jumpgate::VariableName = "AUTH_API_URL".parse()?;
    /// assert_eq!(name.as_str(), "AUTH_API_URL");
    /// assert!("1PORT".parse::<jumpgate::VariableName>().is_err());
    /// # Ok::<(), jumpgate::RecordError>(())
    /// ```
    VariableName, check, {
        "type": "string",
        "minLength": 1,
        "maxLength": MAX_VARIABLE_BYTES,
        "pattern": "^[A-Za-z_][A-Za-z0-9_]*$",
        "description": "Names an environment variable: ASCII letters, digits and '_', not starting with a digit, at most 128 bytes."
    }
}
