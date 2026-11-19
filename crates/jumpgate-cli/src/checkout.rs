//! A checkout on disk, which gives the reader the text of its files.
//!
//! This module is the command's file system owner (BASE-5).

#![expect(
    clippy::disallowed_methods,
    reason = "this module owns the command's reads of a checkout's files"
)]

use std::fs;
use std::io::Read as _;
use std::path::PathBuf;

use jumpgate::RepoFilePath;
use jumpgate::declaration::{Checkout, CheckoutError, MAX_DECLARATION_BYTES};

#[cfg(test)]
mod tests;

/// The files under one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirectoryCheckout {
    root: PathBuf,
    canonical: Result<String, String>,
}

impl DirectoryCheckout {
    /// Returns the checkout whose root is `root`.
    pub(crate) fn new(root: PathBuf) -> Self {
        let canonical = fs::canonicalize(&root)
            .map_err(|error| format!("its directory cannot be resolved ({})", error.kind()))
            .and_then(|path| {
                path.to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "the path of its directory is not UTF-8 text".to_owned())
            });
        Self { root, canonical }
    }
}

/// Reports whether `dir` or a directory above it holds a `.git` entry, which
/// is how git finds the work tree that holds a directory.
fn in_work_tree(dir: &std::path::Path) -> bool {
    let start = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    start
        .ancestors()
        .any(|ancestor| fs::symlink_metadata(ancestor.join(".git")).is_ok())
}

impl Checkout for DirectoryCheckout {
    /// Returns the checkout's canonical directory, with every symlink
    /// resolved, or says that the directory cannot be resolved or that its
    /// path is not UTF-8 text.
    fn root(&self) -> Result<&str, CheckoutError> {
        self.canonical
            .as_deref()
            .map_err(|message| CheckoutError::new(message.clone()))
    }

    /// Returns the `starmap.toml` files that git tracks in the checkout.
    ///
    /// A directory with no `.git` entry in it or in a directory above it is
    /// no git work tree, so it lists none: a checkout named with
    /// `--repo <id>=<path>` that has no git directory is still read, and only
    /// its root `starmap.toml` can be found unread. In a work tree, any
    /// failure of git is an error, such as a checkout that git refuses for
    /// its ownership or a git that does not finish in time.
    fn starmap_files(&self) -> Result<Vec<RepoFilePath>, CheckoutError> {
        if !in_work_tree(&self.root) {
            return Ok(Vec::new());
        }
        let paths = crate::git::tracked_starmaps(&self.root)
            .map_err(|_| CheckoutError::new("git did not list the tracked files"))?;
        Ok(paths.iter().filter_map(|path| path.parse().ok()).collect())
    }

    /// Reads the file, which must be a regular file and not a link, so a
    /// link in a checkout cannot make the command read a file outside it.
    /// It reads one byte more than the reader accepts, so the reader can
    /// refuse a file that is too long without the command reading all of it.
    fn read(&self, path: &RepoFilePath) -> Result<Option<String>, CheckoutError> {
        let file = self.root.join(path.as_str());
        let metadata = match fs::symlink_metadata(&file) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(CheckoutError::new(format!("{}", error.kind()))),
        };
        if !metadata.is_file() {
            return Err(CheckoutError::new("it is not a regular file"));
        }
        let handle = fs::File::open(&file)
            .map_err(|error| CheckoutError::new(format!("{}", error.kind())))?;
        let limit = u64::try_from(MAX_DECLARATION_BYTES)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let mut bytes = Vec::new();
        handle
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(|error| CheckoutError::new(format!("{}", error.kind())))?;
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| CheckoutError::new("it is not UTF-8 text"))
    }
}
