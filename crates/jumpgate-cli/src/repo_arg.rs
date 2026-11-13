//! The value of `--repo`: a checkout's path, with its repository id when
//! the caller gives one.

use std::path::PathBuf;
use std::str::FromStr;

use jumpgate::RepoId;

#[cfg(test)]
mod tests;

/// One `--repo` value, `<path>` or `<host/owner/name>=<path>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoArg {
    /// The id the caller gave, or `None` when git's `origin` names it.
    id: Option<RepoId>,
    /// The checkout's directory.
    path: PathBuf,
}

impl RepoArg {
    /// Returns the id the caller gave.
    pub(crate) const fn id(&self) -> Option<&RepoId> {
        self.id.as_ref()
    }

    /// Returns the checkout's directory.
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl FromStr for RepoArg {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err("a repository is a path, or <host/owner/name>=<path>".to_owned());
        }
        // A path may hold `=`, so the text is an id and a path only when
        // the part before the first `=` is an id.
        if let Some((id, path)) = text.split_once('=')
            && let Ok(id) = id.parse::<RepoId>()
        {
            if path.is_empty() {
                return Err("the path after = is empty".to_owned());
            }
            return Ok(Self {
                id: Some(id),
                path: PathBuf::from(path),
            });
        }
        Ok(Self {
            id: None,
            path: PathBuf::from(text),
        })
    }
}
