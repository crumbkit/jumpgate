//! The value a need receives on a local target when nothing binds it.

use crate::error::{Reason, RecordError};

#[cfg(test)]
mod tests;

/// The most bytes a fallback value holds.
pub const MAX_FALLBACK_BYTES: usize = 4096;

/// Checks `text` against the rule of [`FallbackValue`].
fn check(text: &str) -> Result<(), RecordError> {
    if text.len() > MAX_FALLBACK_BYTES {
        Err(RecordError::new(
            Reason::Fallback,
            format!(
                "a fallback value has {} bytes, and the limit is {MAX_FALLBACK_BYTES}",
                text.len()
            ),
        ))
    } else if text.contains('\0') {
        Err(RecordError::new(
            Reason::Fallback,
            "a fallback value holds a NUL character",
        ))
    } else {
        Ok(())
    }
}

text_type! {
    /// The value a need receives when nothing binds it, such as a
    /// placeholder token for a local workspace.
    ///
    /// A fallback is only ever for a local target; the plan checks refuse a
    /// cloud plan in which a need is met by its fallback. The record allows
    /// any fallback, which may be empty, holds no NUL character and has at
    /// most [`MAX_FALLBACK_BYTES`] bytes.
    FallbackValue, check, {
        "type": "string",
        "maxLength": MAX_FALLBACK_BYTES,
        "pattern": "^[^\\x00]*$",
        "description": "The value a need receives on a local target when nothing binds it. It is a placeholder and never a real secret. It holds no NUL character and has at most 4096 bytes, which maxLength cannot check for text outside ASCII because it counts characters."
    }
}
