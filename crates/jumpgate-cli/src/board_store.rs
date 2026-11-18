//! The store that keeps the five record types in a board.
//!
//! [`BoardStore`] implements the library's [`Store`] by running the `board`
//! command: one `board query` for each read, and one `board put --batch` for
//! each write. It reads and writes no file of the board, and the library
//! depends on no crate of Board.

use std::collections::BTreeSet;

use jumpgate::record;
use jumpgate::store::{AnyRecord, Reason, Store, StoreError, StoredBinding};
use jumpgate::{BoardId, Need, Offer, RecordPath, RecordType, Target, TargetName, Unit};
use serde_json::{Value, json};

use crate::board::{Board, BoardError};

#[cfg(test)]
mod tests;

/// Every jumpgate record is at this path or below it.
const ROOT: &str = "/jumpgate/";

/// A store whose records live in a board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoardStore {
    board: Board,
    id: BoardId,
}

impl BoardStore {
    /// Keeps records in `board`, which the matcher knows as `id`.
    pub(crate) const fn new(board: Board, id: BoardId) -> Self {
        Self { board, id }
    }

    /// Runs one query and returns each row's `logical` path and `body`.
    fn rows(&self, sql: &str) -> Result<Vec<(String, Option<String>)>, StoreError> {
        let text = self
            .board
            .run(&["query", "--all", "--json", sql], None)
            .map_err(|error| refused("board query", &error))?;
        let value: Value = serde_json::from_str(&text).map_err(|_| unreadable())?;
        let rows = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(unreadable)?;
        rows.iter()
            .map(|row| {
                let cells = row.as_array().ok_or_else(unreadable)?;
                let logical = cells
                    .first()
                    .and_then(Value::as_str)
                    .ok_or_else(unreadable)?;
                let body = cells.get(1).and_then(Value::as_str).map(str::to_owned);
                Ok((logical.to_owned(), body))
            })
            .collect()
    }

    /// Reads every record of `record_type` whose path starts with `prefix`,
    /// ordered by path.
    fn records(&self, record_type: RecordType, prefix: &str) -> Result<Vec<AnyRecord>, StoreError> {
        let sql = format!(
            "select logical, body from records where type = {} and logical like {} order by logical",
            literal(record_type.as_str()),
            literal(&format!("{prefix}%")),
        );
        self.rows(&sql)?
            .into_iter()
            .map(|(logical, body)| {
                let body = body.ok_or_else(|| {
                    StoreError::new(
                        Reason::Corrupt,
                        format!("the board holds no readable record at {logical}"),
                    )
                })?;
                let record = AnyRecord::from_json(record_type, &body)
                    .map_err(|error| StoreError::corrupt(&logical, &error))?;
                if record.path().to_string() == logical {
                    Ok(record)
                } else {
                    Err(StoreError::new(
                        Reason::Misplaced,
                        format!("{logical} holds a record whose path is another"),
                    ))
                }
            })
            .collect()
    }

    /// Returns the paths of the records the board holds below `/jumpgate/`.
    fn held(&self) -> Result<BTreeSet<String>, StoreError> {
        let sql = format!(
            "select logical from entries where kind = 'record' and logical like {}",
            literal(&format!("{ROOT}%")),
        );
        Ok(self.rows(&sql)?.into_iter().map(|(path, _)| path).collect())
    }

    /// Writes one batch that registers the five types, writes `records` and
    /// deletes `gone`, which `held` says exist.
    fn write(&self, records: &[AnyRecord], gone: &[&RecordPath]) -> Result<(), StoreError> {
        let mut lines: Vec<String> = Vec::new();
        for record_type in RecordType::ALL {
            let schema = serde_json::to_value(record::schema(*record_type)).map_err(|_| {
                StoreError::new(Reason::Corrupt, "a schema cannot be written as JSON")
            })?;
            lines.push(
                json!({ "path": record_type.board_schema_path(), "record": schema }).to_string(),
            );
        }
        for record in records {
            let body: Value = serde_json::from_str(&record.to_json()?).map_err(|_| unreadable())?;
            lines.push(json!({ "path": record.path().to_string(), "record": body }).to_string());
        }
        for path in gone {
            lines.push(json!({ "path": path.to_string(), "remove": true }).to_string());
        }
        let mut input = lines.join("\n");
        input.push('\n');
        self.board
            .run(&["put", "--batch"], Some(input.into_bytes()))
            .map(drop)
            .map_err(|error| refused("board put --batch", &error))
    }

    /// Reads the one record at `path`, which `record_type` says the type of.
    fn one(&self, path: &RecordPath) -> Result<Option<AnyRecord>, StoreError> {
        let record_type = match path {
            RecordPath::Need(_) => RecordType::Need,
            RecordPath::Offer(_) => RecordType::Offer,
            RecordPath::Binding(_) => RecordType::Binding,
            RecordPath::Target(_) => RecordType::Target,
            RecordPath::Unit(_) => RecordType::Unit,
        };
        let found = self.records(record_type, &path.to_string())?;
        Ok(found.into_iter().find(|record| record.path() == *path))
    }
}

/// Quotes `text` as an SQL string literal.
fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// Returns the error for a `board` run of `what` that gave no output.
fn refused(what: &str, error: &BoardError) -> StoreError {
    let message = match error {
        BoardError::Start => {
            "the board program did not start, so it is not installed or not on PATH".to_owned()
        }
        BoardError::TimedOut => format!("{what} did not finish in time"),
        BoardError::Output => {
            format!("{what} printed more than the command reads, or text that is not UTF-8")
        }
        BoardError::Refused(Some(code)) => format!("{what} refused with {code}"),
        BoardError::Refused(None) => format!("{what} ended with a failure status"),
    };
    StoreError::new(Reason::Board, message)
}

/// Returns the error for output of `board query` that is not the report the
/// store asked for.
fn unreadable() -> StoreError {
    StoreError::new(
        Reason::Board,
        "board query printed something other than its JSON report",
    )
}

impl Store for BoardStore {
    fn board(&self) -> &BoardId {
        &self.id
    }

    fn put(&self, records: &[AnyRecord]) -> Result<(), StoreError> {
        if records.is_empty() {
            return Ok(());
        }
        self.write(records, &[])
    }

    fn remove(&self, paths: &[RecordPath]) -> Result<(), StoreError> {
        self.apply(&[], paths)
    }

    fn apply(&self, records: &[AnyRecord], remove: &[RecordPath]) -> Result<(), StoreError> {
        let written: BTreeSet<RecordPath> = records.iter().map(AnyRecord::path).collect();
        let mut gone: Vec<&RecordPath> = Vec::new();
        if !remove.is_empty() {
            // Board refuses to delete a path that holds nothing, so the batch
            // deletes only the paths the board holds.
            let held = self.held()?;
            gone = remove
                .iter()
                .filter(|path| !written.contains(*path) && held.contains(&path.to_string()))
                .collect();
        }
        if records.is_empty() && gone.is_empty() {
            return Ok(());
        }
        self.write(records, &gone)
    }

    fn get(&self, path: &RecordPath) -> Result<Option<AnyRecord>, StoreError> {
        self.one(path)
    }

    fn needs(&self) -> Result<Vec<Need>, StoreError> {
        collect(
            self.records(RecordType::Need, ROOT)?,
            |record| match record {
                AnyRecord::Need(need) => Some(need),
                _ => None,
            },
        )
    }

    fn offers(&self) -> Result<Vec<Offer>, StoreError> {
        collect(
            self.records(RecordType::Offer, ROOT)?,
            |record| match record {
                AnyRecord::Offer(offer) => Some(offer),
                _ => None,
            },
        )
    }

    fn units(&self) -> Result<Vec<Unit>, StoreError> {
        collect(
            self.records(RecordType::Unit, ROOT)?,
            |record| match record {
                AnyRecord::Unit(unit) => Some(unit),
                _ => None,
            },
        )
    }

    fn targets(&self) -> Result<Vec<Target>, StoreError> {
        collect(
            self.records(RecordType::Target, ROOT)?,
            |record| match record {
                AnyRecord::Target(target) => Some(target),
                _ => None,
            },
        )
    }

    fn bindings(&self, target: &TargetName) -> Result<Vec<StoredBinding>, StoreError> {
        let prefix = format!("{ROOT}bindings/{target}/");
        let found = self.records(RecordType::Binding, &prefix)?;
        Ok(found
            .into_iter()
            .filter_map(|record| match record {
                AnyRecord::Binding(binding) if binding.target() == target => {
                    Some(StoredBinding::new(self.id.clone(), binding))
                }
                _ => None,
            })
            .collect())
    }
}

/// Takes the records `pick` accepts. A query for one type returns only
/// records of that type, so each is accepted.
fn collect<R>(
    records: Vec<AnyRecord>,
    pick: impl Fn(AnyRecord) -> Option<R>,
) -> Result<Vec<R>, StoreError> {
    records
        .into_iter()
        .map(|record| {
            pick(record).ok_or_else(|| {
                StoreError::new(
                    Reason::Corrupt,
                    "a record is not of the type its place says",
                )
            })
        })
        .collect()
}
