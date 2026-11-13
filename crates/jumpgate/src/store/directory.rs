//! The directory store: one JSON file per record.
//!
//! This module is the library's file system owner (BASE-5): no other module
//! reads or writes a file.

#![expect(
    clippy::disallowed_methods,
    reason = "the directory store owns the library's file system effects"
)]

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::name::{BoardId, TargetName};
use crate::path::RecordPath;
use crate::record::{Binding, Need, Offer, Record, Target, Unit};
use crate::store::{AnyRecord, Reason, Store, StoreError, StoredBinding};

/// The file in the store directory that names the board the store writes
/// to, `{"board": "<board-id>"}`.
pub const BOARD_FILE: &str = "store.json";

/// The board id of a directory that has no [`BOARD_FILE`] yet.
pub const DEFAULT_BOARD: &str = "local";

/// The directory that holds the records of other boards, one directory per
/// board id, laid out as the store's own records are.
const OTHER_BOARDS: &str = "boards";

/// The extension of a record's file.
const EXTENSION: &str = "json";

/// A store that keeps the record at path `/jumpgate/needs/shelf/token` in
/// the file `<dir>/jumpgate/needs/shelf/token.json`.
///
/// The records of another board, which the matcher reads to ignore, live in
/// the same layout under `<dir>/boards/<board-id>/`. The store writes only
/// its own records and never deletes a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryStore {
    dir: PathBuf,
    board: BoardId,
}

impl DirectoryStore {
    /// Opens the store in `dir`, which need not exist yet. Its board id is
    /// read from [`BOARD_FILE`], and is [`DEFAULT_BOARD`] when the file is
    /// missing.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] when the board file cannot be read or does
    /// not hold a board id.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let dir = dir.into();
        let file = dir.join(BOARD_FILE);
        let board = match fs::read_to_string(&file) {
            Ok(text) => parse_board_file(&text)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => default_board()?,
            Err(error) => return Err(io_error(&file, &error)),
        };
        Ok(Self { dir, board })
    }

    /// Returns the store's directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn write_board_file(&self) -> Result<(), StoreError> {
        let file = self.dir.join(BOARD_FILE);
        if file.exists() {
            return Ok(());
        }
        let text = serde_json::json!({ "board": self.board.as_str() }).to_string();
        write_atomically(&file, &text)
    }

    /// Reads every record of type `R` under `root`, a store's root, in the
    /// directory `kind` and below, ordered by path.
    fn list<R: Record>(root: &Path, kind: &str, below: &[&str]) -> Result<Vec<R>, StoreError> {
        let mut start = root.join("jumpgate").join(kind);
        for part in below {
            start.push(part);
        }
        let mut files = Vec::new();
        collect_files(&start, &mut files)?;
        files.sort();
        files
            .iter()
            .map(|file| read_record::<R>(root, file))
            .collect()
    }
}

impl Store for DirectoryStore {
    fn board(&self) -> &BoardId {
        &self.board
    }

    fn put(&self, records: &[AnyRecord]) -> Result<(), StoreError> {
        // Every record is rendered before the first file is written, so a
        // record that cannot be written leaves the directory as it was.
        let mut rendered = Vec::with_capacity(records.len());
        for record in records {
            rendered.push((file_of(&self.dir, &record.path()), record.to_json()?));
        }
        self.write_board_file()?;
        for (file, text) in rendered {
            write_atomically(&file, &text)?;
        }
        Ok(())
    }

    fn get(&self, path: &RecordPath) -> Result<Option<AnyRecord>, StoreError> {
        let file = file_of(&self.dir, path);
        if !file.is_file() {
            return Ok(None);
        }
        let record = match path {
            RecordPath::Need(_) => read_record::<Need>(&self.dir, &file)?.into(),
            RecordPath::Offer(_) => read_record::<Offer>(&self.dir, &file)?.into(),
            RecordPath::Binding(_) => read_record::<Binding>(&self.dir, &file)?.into(),
            RecordPath::Target(_) => read_record::<Target>(&self.dir, &file)?.into(),
            RecordPath::Unit(_) => read_record::<Unit>(&self.dir, &file)?.into(),
        };
        Ok(Some(record))
    }

    fn needs(&self) -> Result<Vec<Need>, StoreError> {
        Self::list(&self.dir, "needs", &[])
    }

    fn offers(&self) -> Result<Vec<Offer>, StoreError> {
        Self::list(&self.dir, "offers", &[])
    }

    fn units(&self) -> Result<Vec<Unit>, StoreError> {
        Self::list(&self.dir, "units", &[])
    }

    fn targets(&self) -> Result<Vec<Target>, StoreError> {
        Self::list(&self.dir, "targets", &[])
    }

    fn bindings(&self, target: &TargetName) -> Result<Vec<StoredBinding>, StoreError> {
        let mut found = Vec::new();
        let own: Vec<Binding> = Self::list(&self.dir, "bindings", &[target.as_str()])?;
        found.extend(
            own.into_iter()
                .map(|binding| StoredBinding::new(self.board.clone(), binding)),
        );
        let mut boards = Vec::new();
        let others = self.dir.join(OTHER_BOARDS);
        if others.is_dir() {
            for entry in read_dir(&others)? {
                let name = entry.file_name();
                let board: BoardId = name
                    .to_str()
                    .and_then(|name| name.parse().ok())
                    .ok_or_else(|| {
                        StoreError::new(
                            Reason::BoardFile,
                            "a directory under boards is not named by a board id",
                        )
                    })?;
                boards.push((board, entry.path()));
            }
        }
        boards.sort();
        for (board, root) in boards {
            let bindings: Vec<Binding> = Self::list(&root, "bindings", &[target.as_str()])?;
            found.extend(
                bindings
                    .into_iter()
                    .map(|binding| StoredBinding::new(board.clone(), binding)),
            );
        }
        Ok(found)
    }
}

fn default_board() -> Result<BoardId, StoreError> {
    DEFAULT_BOARD
        .parse()
        .map_err(|_| StoreError::new(Reason::BoardFile, "the default board id is not valid"))
}

fn parse_board_file(text: &str) -> Result<BoardId, StoreError> {
    let refused = || {
        StoreError::new(
            Reason::BoardFile,
            format!("{BOARD_FILE} is not {{\"board\": \"<board-id>\"}}"),
        )
    };
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| refused())?;
    let object = value
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or_else(refused)?;
    object
        .get("board")
        .and_then(serde_json::Value::as_str)
        .and_then(|board| board.parse().ok())
        .ok_or_else(refused)
}

/// Returns the file that holds the record at `path` in the store rooted at
/// `root`.
fn file_of(root: &Path, path: &RecordPath) -> PathBuf {
    let text = path.to_string();
    let mut file = root.to_path_buf();
    for segment in text.split('/').filter(|segment| !segment.is_empty()) {
        file.push(segment);
    }
    let mut name = file.as_os_str().to_owned();
    name.push(format!(".{EXTENSION}"));
    PathBuf::from(name)
}

fn io_error(file: &Path, error: &std::io::Error) -> StoreError {
    StoreError::new(
        Reason::Io,
        format!(
            "{} cannot be read or written: {}",
            file.display(),
            error.kind()
        ),
    )
}

fn read_dir(dir: &Path) -> Result<Vec<fs::DirEntry>, StoreError> {
    let entries = fs::read_dir(dir).map_err(|error| io_error(dir, &error))?;
    entries
        .map(|entry| entry.map_err(|error| io_error(dir, &error)))
        .collect()
}

/// Adds every record file below `dir` to `files`. A missing directory holds
/// none, and a file that does not end in `.json`, such as a temporary file
/// of an interrupted write, is not a record.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), StoreError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in read_dir(dir)? {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == EXTENSION)
        {
            files.push(path);
        }
    }
    Ok(())
}

/// Reads the record in `file`, which must be at the path the record holds
/// within the store rooted at `root`.
fn read_record<R: Record>(root: &Path, file: &Path) -> Result<R, StoreError> {
    let text = fs::read_to_string(file).map_err(|error| io_error(file, &error))?;
    let shown = file.display().to_string();
    let record = R::from_json(&text).map_err(|error| StoreError::corrupt(&shown, &error))?;
    if file_of(root, &record.record_path()) == file {
        Ok(record)
    } else {
        Err(StoreError::new(
            Reason::Misplaced,
            format!("{shown} holds a record whose path is another file"),
        ))
    }
}

/// Writes `text` to `file` through a temporary file in the same directory
/// and a rename, so a reader sees the old record or the new one.
fn write_atomically(file: &Path, text: &str) -> Result<(), StoreError> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
    }
    let mut temporary = file.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let write = || -> std::io::Result<()> {
        let mut handle = fs::File::create(&temporary)?;
        handle.write_all(text.as_bytes())?;
        handle.write_all(b"\n")?;
        handle.sync_all()?;
        fs::rename(&temporary, file)
    };
    write().map_err(|error| io_error(file, &error))
}
