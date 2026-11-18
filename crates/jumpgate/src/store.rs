//! The store that holds the five record types, read and written by their
//! typed paths.
//!
//! [`Store`] is the trait the commands use, and [`DirectoryStore`] keeps one
//! JSON file per record in a directory. A store that keeps its records in a
//! board implements the same trait. It lives in the command, because it runs
//! the `board` program and this crate runs no process.
//!
//! A refused read or write is a [`StoreError`] whose code is
//! `jumpgate::store::<reason>`. A message names a file within the store and
//! never quotes the file's content.

mod directory;

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::error::{Code, RecordError};
use crate::name::{BoardId, TargetName};
use crate::path::RecordPath;
use crate::record::{Binding, Need, Offer, Record, RecordType, Target, Unit};

pub use crate::store::directory::{
    BOARD_FILE, DEFAULT_BOARD, DirectoryStore, read_plan_text, write_plan_file,
};

/// A record of any of the five types.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnyRecord {
    /// A need.
    Need(Need),
    /// An offer.
    Offer(Offer),
    /// A binding.
    Binding(Binding),
    /// A target.
    Target(Target),
    /// A unit.
    Unit(Unit),
}

impl AnyRecord {
    /// Returns the record's path in a store.
    #[must_use]
    pub fn path(&self) -> RecordPath {
        match self {
            Self::Need(record) => record.record_path(),
            Self::Offer(record) => record.record_path(),
            Self::Binding(record) => record.record_path(),
            Self::Target(record) => record.record_path(),
            Self::Unit(record) => record.record_path(),
        }
    }

    /// Returns the record's type.
    #[must_use]
    pub const fn record_type(&self) -> RecordType {
        match self {
            Self::Need(_) => RecordType::Need,
            Self::Offer(_) => RecordType::Offer,
            Self::Binding(_) => RecordType::Binding,
            Self::Target(_) => RecordType::Target,
            Self::Unit(_) => RecordType::Unit,
        }
    }

    /// Reads a record of type `record_type` from its JSON text.
    ///
    /// # Errors
    ///
    /// Returns a [`RecordError`] when the text is not a record of that
    /// type, as [`Record::from_json`] does.
    pub fn from_json(record_type: RecordType, text: &str) -> Result<Self, RecordError> {
        Ok(match record_type {
            RecordType::Need => Need::from_json(text)?.into(),
            RecordType::Offer => Offer::from_json(text)?.into(),
            RecordType::Binding => Binding::from_json(text)?.into(),
            RecordType::Target => Target::from_json(text)?.into(),
            RecordType::Unit => Unit::from_json(text)?.into(),
        })
    }

    /// Returns the record's JSON text.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when serde_json cannot write the record,
    /// which no record of the five types causes.
    pub fn to_json(&self) -> Result<String, StoreError> {
        let text = match self {
            Self::Need(record) => serde_json::to_string_pretty(record),
            Self::Offer(record) => serde_json::to_string_pretty(record),
            Self::Binding(record) => serde_json::to_string_pretty(record),
            Self::Target(record) => serde_json::to_string_pretty(record),
            Self::Unit(record) => serde_json::to_string_pretty(record),
        };
        text.map_err(|_| {
            StoreError::new(
                Reason::Corrupt,
                "a record cannot be written as JSON".to_owned(),
            )
        })
    }
}

macro_rules! any_record_from {
    ($($record:ident),*) => {$(
        impl From<$record> for AnyRecord {
            fn from(record: $record) -> Self {
                Self::$record(record)
            }
        }
    )*};
}

any_record_from!(Need, Offer, Binding, Target, Unit);

/// A binding and the board it was read from. The matcher counts a binding
/// only when its board is the authority of the target it plans for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBinding {
    board: BoardId,
    binding: Binding,
}

impl StoredBinding {
    /// Builds the binding `binding` as read from `board`.
    #[must_use]
    pub const fn new(board: BoardId, binding: Binding) -> Self {
        Self { board, binding }
    }

    /// Returns the board the binding was read from.
    #[must_use]
    pub const fn board(&self) -> &BoardId {
        &self.board
    }

    /// Returns the binding.
    #[must_use]
    pub const fn binding(&self) -> &Binding {
        &self.binding
    }
}

/// Where records are kept.
pub trait Store {
    /// Returns the id of the board that the store writes to.
    fn board(&self) -> &BoardId;

    /// Writes `records` in one batch, each replacing the record at its
    /// path. A write never deletes a record; [`Store::remove`] does.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when a record cannot be written.
    fn put(&self, records: &[AnyRecord]) -> Result<(), StoreError>;

    /// Deletes the records at `paths` from the store's own board. A path
    /// with no record is skipped.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when a record that exists cannot be
    /// deleted.
    fn remove(&self, paths: &[RecordPath]) -> Result<(), StoreError>;

    /// Writes `records` as [`Store::put`] does and deletes the records at
    /// `remove` as [`Store::remove`] does. A path that is in both is
    /// written and not deleted.
    ///
    /// The default writes first and deletes after. A store that can make
    /// both changes as one step overrides it, so a reader never sees the
    /// records of one run half written.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when a record cannot be written or
    /// deleted.
    fn apply(&self, records: &[AnyRecord], remove: &[RecordPath]) -> Result<(), StoreError> {
        if !records.is_empty() {
            self.put(records)?;
        }
        let written: BTreeSet<RecordPath> = records.iter().map(AnyRecord::path).collect();
        let gone: Vec<RecordPath> = remove
            .iter()
            .filter(|path| !written.contains(*path))
            .cloned()
            .collect();
        if gone.is_empty() {
            Ok(())
        } else {
            self.remove(&gone)
        }
    }

    /// Returns the record at `path`, or `None` when the store has none.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when the record exists and cannot be read.
    fn get(&self, path: &RecordPath) -> Result<Option<AnyRecord>, StoreError>;

    /// Returns every need, ordered by path.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when a need cannot be read.
    fn needs(&self) -> Result<Vec<Need>, StoreError>;

    /// Returns every offer, ordered by path.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when an offer cannot be read.
    fn offers(&self) -> Result<Vec<Offer>, StoreError>;

    /// Returns every unit, ordered by path.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when a unit cannot be read.
    fn units(&self) -> Result<Vec<Unit>, StoreError>;

    /// Returns every target, ordered by path.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when a target cannot be read.
    fn targets(&self) -> Result<Vec<Target>, StoreError>;

    /// Returns the bindings for `target` from every board the store holds,
    /// each with the board it came from.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when a binding cannot be read.
    fn bindings(&self, target: &TargetName) -> Result<Vec<StoredBinding>, StoreError>;
}

/// Why the store refused a read or a write. Each reason has the code
/// `jumpgate::store::<name>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Reason {
    /// The file system refused to read or write a file.
    Io,
    /// A file holds text that is not a record of the type its place says.
    Corrupt,
    /// A record's file is not at the path of the record it holds.
    Misplaced,
    /// The board file is not `{"board": "<board-id>"}`, a directory of
    /// another board's records is not named by a board id, or the
    /// directory of a board that a store opens is not named by one.
    BoardFile,
    /// The `board` program did not start, did not answer in time, printed
    /// something other than what the store asked for, or refused a read or
    /// a write.
    Board,
}

impl Reason {
    /// Every reason.
    pub const ALL: &'static [Self] = &[
        Self::Io,
        Self::Corrupt,
        Self::Misplaced,
        Self::BoardFile,
        Self::Board,
    ];

    /// Returns the reason's failure code, `jumpgate::store::<name>`.
    #[must_use]
    pub const fn code(self) -> Code {
        Code::new(
            "store",
            match self {
                Self::Io => "io",
                Self::Corrupt => "corrupt",
                Self::Misplaced => "misplaced",
                Self::BoardFile => "board_file",
                Self::Board => "board",
            },
        )
    }
}

/// A read or write that the store refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError {
    reason: Reason,
    message: Cow<'static, str>,
}

impl StoreError {
    /// Builds the error of `reason` with `message`, which must name a file
    /// or a record by its path and never quote a record's content.
    #[must_use]
    pub fn new(reason: Reason, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    /// Returns why the store refused.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        self.reason
    }

    /// Returns the failure code.
    #[must_use]
    pub const fn code(&self) -> Code {
        self.reason.code()
    }

    /// Returns the message, which names a file and never quotes content.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the error for a file that holds a refused record.
    #[must_use]
    pub fn corrupt(file: &str, error: &RecordError) -> Self {
        Self::new(
            Reason::Corrupt,
            format!("{file} is not a valid record: {error}"),
        )
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.message)
    }
}

impl Error for StoreError {}

#[cfg(test)]
mod tests;
