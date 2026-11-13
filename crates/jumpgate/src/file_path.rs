//! The file paths that records hold: a file within a repository, and the
//! file a need is delivered as.

use crate::error::{Reason, RecordError};

#[cfg(test)]
mod tests;

/// The most bytes a file path holds.
pub const MAX_FILE_PATH_BYTES: usize = 1024;

/// Checks `text` as a path relative to a repository's root.
fn check_relative(text: &str) -> Result<(), RecordError> {
    if text.starts_with('/') {
        return Err(refused(
            "starts with '/', and a path within a repository is relative to its root",
        ));
    }
    check_segments(text, text)
}

/// Checks `text` as a path that may start with `/`.
fn check_delivery(text: &str) -> Result<(), RecordError> {
    check_segments(text, text.strip_prefix('/').unwrap_or(text))
}

/// Checks the length of `whole` and each segment of `relative`, the part of
/// `whole` after any leading `/`.
fn check_segments(whole: &str, relative: &str) -> Result<(), RecordError> {
    if whole.len() > MAX_FILE_PATH_BYTES {
        return Err(refused("is longer than 1024 bytes"));
    }
    let segment_fits = |segment: &str| {
        !matches!(segment, "" | "." | "..")
            && !segment
                .chars()
                .any(|character| character == '\\' || character.is_control())
    };
    if relative.split('/').all(segment_fits) {
        Ok(())
    } else {
        Err(refused(
            "holds an empty, '.' or '..' segment, a '\\' or a control character",
        ))
    }
}

/// Returns the [`Reason::FilePath`] error for a path that `problem`.
fn refused(problem: &str) -> RecordError {
    RecordError::new(Reason::FilePath, format!("the file path {problem}"))
}

/// The pattern of one segment in the JSON Schema: one or more characters
/// that are not `/`, `\` or a control character (U+0000 to U+001F and
/// U+007F to U+009F, the characters that `char::is_control` names), and
/// not `.` or `..`.
macro_rules! segment_pattern {
    () => {
        r"(?:[^/.\\\x00-\x1F\x7F-\x9F][^/\\\x00-\x1F\x7F-\x9F]*|\.[^/.\\\x00-\x1F\x7F-\x9F][^/\\\x00-\x1F\x7F-\x9F]*|\.\.[^/\\\x00-\x1F\x7F-\x9F]+)"
    };
}

/// The pattern of a [`RepoFilePath`] in the JSON Schema.
const RELATIVE_PATTERN: &str = concat!("^", segment_pattern!(), "(?:/", segment_pattern!(), ")*$");

/// The pattern of a [`DeliveryPath`] in the JSON Schema.
const DELIVERY_PATTERN: &str =
    concat!("^/?", segment_pattern!(), "(?:/", segment_pattern!(), ")*$");

text_type! {
    /// The path of a file within a repository, relative to its root and
    /// written with `/`, such as `services/api/starmap.toml`.
    ///
    /// Each segment is not empty, is not `.` or `..`, and holds no `\` and
    /// no control character. The path does not start with `/` and has at
    /// most [`MAX_FILE_PATH_BYTES`] bytes, so it cannot leave the
    /// repository.
    ///
    /// ```
    /// let path: jumpgate::RepoFilePath = "services/api/starmap.toml".parse()?;
    /// assert_eq!(path.as_str(), "services/api/starmap.toml");
    /// assert!("../starmap.toml".parse::<jumpgate::RepoFilePath>().is_err());
    /// # Ok::<(), jumpgate::RecordError>(())
    /// ```
    RepoFilePath, check_relative, {
        "type": "string",
        "minLength": 1,
        "maxLength": MAX_FILE_PATH_BYTES,
        "pattern": RELATIVE_PATTERN,
        "description": "Names a file within a repository, relative to its root and written with '/'; no segment is empty, '.' or '..', and none holds a '\\' or a control character. The path has at most 1024 bytes, which maxLength cannot check for text outside ASCII because it counts characters."
    }
}

impl RepoFilePath {
    /// Returns the path for a literal that this crate's tests check, such
    /// as the name of a declaration file, without parsing it again.
    pub(crate) fn known(text: &'static str) -> Self {
        Self(text.to_owned())
    }
}

text_type! {
    /// The path of the file that a need is delivered as, such as
    /// `/run/secrets/api-token` or `config/token`.
    ///
    /// It follows the rule of [`RepoFilePath`], except that it may start
    /// with `/`. A relative path is read from the unit's working directory.
    DeliveryPath, check_delivery, {
        "type": "string",
        "minLength": 1,
        "maxLength": MAX_FILE_PATH_BYTES,
        "pattern": DELIVERY_PATTERN,
        "description": "Names the file a need is delivered as, absolute or relative to the unit's working directory; no segment is empty, '.' or '..', and none holds a '\\' or a control character. The path has at most 1024 bytes, which maxLength cannot check for text outside ASCII because it counts characters."
    }
}
