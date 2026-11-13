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
}

impl DirectoryCheckout {
    /// Returns the checkout whose root is `root`.
    pub(crate) const fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Checkout for DirectoryCheckout {
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
