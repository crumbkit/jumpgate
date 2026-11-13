//! The id of a repository, `host/owner/name`.

use crate::error::{Reason, RecordError};

#[cfg(test)]
mod tests;

/// The most bytes a repository id holds.
pub const MAX_REPO_ID_BYTES: usize = 255;

/// Checks `text` against the rule of [`RepoId`].
fn check(text: &str) -> Result<(), RecordError> {
    let refuse = |problem: &str| {
        Err(RecordError::new(
            Reason::Repository,
            format!("a repository id {problem}"),
        ))
    };
    if text.len() > MAX_REPO_ID_BYTES {
        return refuse("is longer than 255 bytes");
    }
    let mut segments = text.split('/');
    let (Some(host), Some(owner), Some(name), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return refuse("is not host/owner/name");
    };
    let host_fits = host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
                })
        });
    if !host_fits {
        return refuse(
            "has a host that is not lowercase labels of letters, digits and '-' joined by '.'",
        );
    }
    let part_fits = |part: &str| {
        !part.is_empty()
            && !part.starts_with('.')
            && !part.contains("..")
            && part.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '-' | '_' | '.')
            })
    };
    if !(part_fits(owner) && part_fits(name)) {
        return refuse(
            "has an owner or name that is not lowercase letters, digits, '-', '_' and '.', or that starts with '.' or holds '..'",
        );
    }
    Ok(())
}

text_type! {
    /// The id of a repository, written `host/owner/name`, such as
    /// `github.com/acme/shelf`.
    ///
    /// The id is the normalised form of a repository's origin remote, and
    /// it is lowercase, because the hosts that jumpgate meets treat owner
    /// and repository names without regard to case, so one repository has
    /// one id. The host holds lowercase letters, digits, `-` and `.`, with
    /// at least one `.` and no empty label. The owner and the name each hold
    /// lowercase letters, digits, `-`, `_` and `.`, do not start with `.`,
    /// and hold no `..`. The whole id has at most [`MAX_REPO_ID_BYTES`]
    /// bytes. The library takes the id as text and runs no git, so the
    /// caller normalises a remote into this form.
    ///
    /// ```
    /// let repo: jumpgate::RepoId = "github.com/acme/shelf".parse()?;
    /// assert_eq!(repo.as_str(), "github.com/acme/shelf");
    /// assert!("https://github.com/acme/shelf".parse::<jumpgate::RepoId>().is_err());
    /// # Ok::<(), jumpgate::RecordError>(())
    /// ```
    RepoId, check, {
        "type": "string",
        "minLength": 5,
        "maxLength": MAX_REPO_ID_BYTES,
        "pattern": "^[a-z0-9-]+(\\.[a-z0-9-]+)+/[a-z0-9_-]+(\\.[a-z0-9_-]+)*\\.?/[a-z0-9_-]+(\\.[a-z0-9_-]+)*\\.?$",
        "description": "Names a repository as host/owner/name, in lowercase, such as github.com/acme/shelf."
    }
}
