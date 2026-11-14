//! The repository id that a git remote URL names.

use jumpgate::RepoId;

#[cfg(test)]
mod tests;

/// Returns the id `host/owner/name` that the remote URL `url` names, or
/// `None` when `url` is not a remote of that shape.
///
/// The id is lowercase, with the scheme, the user, a port and a trailing
/// `.git` removed. It reads the forms `https://host/owner/name.git`,
/// `ssh://user@host:22/owner/name` and `user@host:owner/name`.
pub(crate) fn repo_id(url: &str) -> Option<RepoId> {
    let url = url.trim().to_lowercase();
    let (host, path) = match url.split_once("://") {
        Some((_, rest)) => {
            let (authority, path) = rest.split_once('/')?;
            let host = authority
                .rsplit_once('@')
                .map_or(authority, |(_, host)| host);
            (host.split_once(':').map_or(host, |(host, _)| host), path)
        }
        None => {
            let (authority, path) = url.split_once(':')?;
            (
                authority
                    .rsplit_once('@')
                    .map_or(authority, |(_, host)| host),
                path,
            )
        }
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    format!("{host}/{path}").parse().ok()
}
