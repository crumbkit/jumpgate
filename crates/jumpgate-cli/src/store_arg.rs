//! The options that choose the store a command reads and writes: a
//! directory with `--store`, or a board, which is the default.

use std::path::{self, PathBuf};

use clap::Args;
use jumpgate::BoardId;
use jumpgate::store::{DirectoryStore, Reason, Store, StoreError};

use crate::board::{Board, Scope};
use crate::board_store::BoardStore;

/// The id of the personal board, which the matcher compares with the
/// authority of a target.
const PERSONAL: &str = "personal";

/// The options that choose the store.
#[derive(Debug, Args)]
pub(crate) struct StoreArgs {
    /// A directory of JSON files, one for each record, to use as the
    /// store instead of a board.
    #[arg(long, value_name = "DIR", conflicts_with = "board")]
    pub(crate) store: Option<PathBuf>,

    /// The directory of the board to use as the store. Without `--store`
    /// and `--board` the store is the personal board.
    #[arg(long, value_name = "DIR")]
    pub(crate) board: Option<PathBuf>,
}

/// A store the options chose.
pub(crate) struct Chosen {
    store: Box<dyn Store>,
    place: String,
}

impl Chosen {
    /// Returns the store.
    pub(crate) fn store(&self) -> &dyn Store {
        &*self.store
    }

    /// Describes where the store is, for a report.
    pub(crate) fn place(&self) -> &str {
        &self.place
    }
}

impl StoreArgs {
    /// Opens the store the options name.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when a directory store's board file is
    /// refused, or when a board's directory is not named by a board id.
    pub(crate) fn open(&self) -> Result<Chosen, StoreError> {
        if let Some(dir) = &self.store {
            let store = DirectoryStore::open(dir)?;
            return Ok(Chosen {
                store: Box::new(store),
                place: dir.display().to_string(),
            });
        }
        let (scope, id, place) = match &self.board {
            Some(dir) => (
                Scope::Directory(dir.clone()),
                id_of(dir)?,
                format!("the board in {}", dir.display()),
            ),
            None => (
                Scope::Personal,
                personal()?,
                "the personal board".to_owned(),
            ),
        };
        Ok(Chosen {
            store: Box::new(BoardStore::new(Board::new(scope), id)),
            place,
        })
    }
}

/// Returns the id of the personal board.
fn personal() -> Result<BoardId, StoreError> {
    PERSONAL.parse().map_err(|_| {
        StoreError::new(
            Reason::BoardFile,
            "the id of the personal board is not valid",
        )
    })
}

/// Returns the board id that the directory of a board gives: the last part
/// of its absolute path.
fn id_of(dir: &path::Path) -> Result<BoardId, StoreError> {
    path::absolute(dir)
        .ok()
        .and_then(|absolute| {
            absolute
                .file_name()
                .and_then(|name| name.to_str().map(str::to_owned))
        })
        .and_then(|name| name.parse().ok())
        .ok_or_else(|| {
            StoreError::new(
                Reason::BoardFile,
                "the directory of the board is not named by a board id",
            )
        })
}

#[cfg(test)]
mod tests;
