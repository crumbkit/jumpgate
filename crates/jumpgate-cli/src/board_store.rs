//! The store that keeps the five record types in a board.
//!
//! [`BoardStore`] implements the library's [`Store`] by running the `board`
//! command: one `board query` for each read, and one `board put --batch` for
//! each write. It reads and writes no file of the board, and the library
//! depends on no crate of Board.

use std::collections::BTreeSet;
use std::fmt;

use jumpgate::record::{self, Record};
use jumpgate::store::{AnyRecord, Reason, Store, StoreError, StoredAt, StoredBinding};
use jumpgate::{
    Binding, BoardId, Need, NeedKind, Offer, RecordError, RecordPath, RecordType, Target,
    TargetName, Unit, WhenUnbound,
};
use serde_json::{Value, json};

use crate::board::{Board, BoardError};

#[cfg(test)]
mod tests;

/// Every jumpgate record is at this path or below it.
const ROOT: &str = "/jumpgate/";

/// The path at which a board holds the schema of a record type.
struct SchemaPath(String);

impl SchemaPath {
    /// Returns the path of the schema of `record_type`, such as
    /// `/.board/types/jumpgate/need/v1.schema`.
    fn of(record_type: RecordType) -> Self {
        Self(format!("/.board/types/{}.schema", record_type.as_str()))
    }
}

impl fmt::Display for SchemaPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Which paths of the board a read selects. Both compare the characters of
/// the path as they are, which `LIKE` does not: it reads an underscore as
/// any one character and ignores the case of ASCII letters.
enum Matching<'a> {
    /// Every path that starts with this text.
    Prefix(&'a str),
    /// The one path.
    Exactly(&'a str),
}

impl Matching<'_> {
    /// Returns the SQL condition on `logical` that selects the paths.
    fn condition(&self) -> String {
        match self {
            Self::Prefix(prefix) => format!(
                "substr(logical, 1, {}) = {}",
                prefix.chars().count(),
                literal(prefix)
            ),
            Self::Exactly(path) => format!("logical = {}", literal(path)),
        }
    }
}

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
            .query(sql)
            .map_err(|error| refused("board query", &error))?;
        let value: Value = serde_json::from_str(&text).map_err(|_| unreadable())?;
        if value.get("truncated").and_then(Value::as_bool) != Some(false) {
            return Err(StoreError::new(
                Reason::Board,
                "board query stopped at its row limit or did not say it had not, so the store would be read in part",
            ));
        }
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
    /// ordered by path, with `read` and checks each sits at its own path.
    fn records_with<T>(
        &self,
        record_type: RecordType,
        matching: &Matching<'_>,
        read: impl Fn(&str) -> Result<T, RecordError>,
        path_of: impl Fn(&T) -> RecordPath,
    ) -> Result<Vec<T>, StoreError> {
        let sql = format!(
            "select logical, body from records where type = {} and {} order by logical",
            literal(record_type.as_str()),
            matching.condition(),
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
                let record = read(&body).map_err(|error| {
                    StoreError::corrupt(&StoredAt::new(logical.clone()), &error)
                })?;
                if path_of(&record).to_string() == logical {
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

    /// Reads every record of the type `R` whose path starts with `prefix`.
    fn records<R: Record>(
        &self,
        record_type: RecordType,
        prefix: &str,
    ) -> Result<Vec<R>, StoreError> {
        self.records_with(
            record_type,
            &Matching::Prefix(prefix),
            R::from_json,
            Record::record_path,
        )
    }

    /// Returns the paths of the records the board holds below `/jumpgate/`.
    fn held(&self) -> Result<BTreeSet<String>, StoreError> {
        let sql = format!(
            "select logical from entries where kind = 'record' and {}",
            Matching::Prefix(ROOT).condition(),
        );
        Ok(self.rows(&sql)?.into_iter().map(|(path, _)| path).collect())
    }

    /// Writes one batch that registers the five types, writes `records` and
    /// deletes `gone`, which `held` says exist.
    fn write(&self, records: &[AnyRecord], gone: &[&RecordPath]) -> Result<(), StoreError> {
        let mut lines: Vec<String> = Vec::new();
        for record_type in RecordType::ALL {
            let schema = board_schema(*record_type)?;
            lines.push(
                json!({ "path": SchemaPath::of(*record_type).to_string(), "record": schema })
                    .to_string(),
            );
        }
        for record in records {
            if let AnyRecord::Need(need) = record {
                refuse_secret_fallback(&record.path().to_string(), need)?;
            }
            let body: Value = serde_json::from_str(&record.to_json()?).map_err(|_| unreadable())?;
            lines.push(json!({ "path": record.path().to_string(), "record": body }).to_string());
        }
        for path in gone {
            lines.push(json!({ "path": path.to_string(), "remove": true }).to_string());
        }
        let mut input = lines.join("\n");
        input.push('\n');
        self.board
            .put_batch(input.into_bytes())
            .map(drop)
            .map_err(|error| refused("board put --batch", &error))
    }

    /// Reads the one record at `path`.
    fn one(&self, path: &RecordPath) -> Result<Option<AnyRecord>, StoreError> {
        let record_type = path.record_type();
        let shown = path.to_string();
        let found = self.records_with(
            record_type,
            &Matching::Exactly(&shown),
            |text| AnyRecord::from_json(record_type, text),
            AnyRecord::path,
        )?;
        let record = found.into_iter().find(|record| record.path() == *path);
        if let Some(AnyRecord::Need(need)) = &record {
            refuse_secret_fallback(&shown, need)?;
        }
        Ok(record)
    }
}

/// Refuses a secret need that falls back to a literal when it is unbound.
/// The record type allows it, and jumpgate's own reader never writes one,
/// but a secret's literal must never sit in a board, so the store neither
/// reads one from the board nor writes one to it. The message names the
/// record and quotes no value.
fn refuse_secret_fallback(path: &str, need: &Need) -> Result<(), StoreError> {
    if need.kind() == NeedKind::Secret && matches!(need.when_unbound(), WhenUnbound::Fallback(_)) {
        return Err(StoreError::new(
            Reason::Corrupt,
            format!(
                "{path} is a secret need that falls back to a literal, and no secret has a fallback"
            ),
        ));
    }
    Ok(())
}

/// Returns the schema that a board registers for `record_type`, which is the
/// library's schema. The need schema also carries a rule that forbids a
/// fallback on a secret, so the board refuses such a need from every writer
/// and not only from this store.
fn board_schema(record_type: RecordType) -> Result<Value, StoreError> {
    let mut schema = serde_json::to_value(record::schema(record_type))
        .map_err(|_| StoreError::new(Reason::Corrupt, "a schema cannot be written as JSON"))?;
    if let (RecordType::Need, Some(root)) = (record_type, schema.as_object_mut()) {
        root.insert(
            "if".to_owned(),
            json!({ "properties": { "kind": { "const": "secret" } }, "required": ["kind"] }),
        );
        root.insert(
            "then".to_owned(),
            json!({ "properties": { "when_unbound": { "type": "string" } } }),
        );
    }
    Ok(schema)
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
        let needs: Vec<Need> = self.records(RecordType::Need, ROOT)?;
        for need in &needs {
            refuse_secret_fallback(&need.record_path().to_string(), need)?;
        }
        Ok(needs)
    }

    fn offers(&self) -> Result<Vec<Offer>, StoreError> {
        self.records(RecordType::Offer, ROOT)
    }

    fn units(&self) -> Result<Vec<Unit>, StoreError> {
        self.records(RecordType::Unit, ROOT)
    }

    fn targets(&self) -> Result<Vec<Target>, StoreError> {
        self.records(RecordType::Target, ROOT)
    }

    fn bindings(&self, target: &TargetName) -> Result<Vec<StoredBinding>, StoreError> {
        let prefix = format!("{ROOT}bindings/{target}/");
        let found: Vec<Binding> = self.records(RecordType::Binding, &prefix)?;
        Ok(found
            .into_iter()
            .filter(|binding| binding.target() == target)
            .map(|binding| StoredBinding::new(self.id.clone(), binding))
            .collect())
    }
}
