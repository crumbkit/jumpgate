//! `jumpgate needs`: read the declarations of some checkouts and write the
//! needs, provides and units they give to a store.

use clap::Args;
use crumbkit::{Code, Document, Failure, Prose, Recovery};
use jumpgate::RecordPath;
use jumpgate::declaration::{Refusal, Repository, named_by_bindings, read, stale};
use jumpgate::store::{AnyRecord, StoreError};
use jumpgate::{Record as _, RepoId};
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt;

use crate::checkout::DirectoryCheckout;
use crate::codes;
use crate::git::{self, GitError};
use crate::remote;
use crate::repo_arg::RepoArg;
use crate::store_arg::StoreArgs;

#[cfg(test)]
mod tests;

/// The document that `jumpgate needs` prints.
pub(crate) const NEEDS: Document<Prose<Report, NeedsFailure>> = Document::new(
    "jumpgate",
    "needs",
    1,
    "The needs, provides and units that jumpgate read from the declarations and wrote to the store.",
);

/// The arguments of `jumpgate needs`.
#[derive(Debug, Args)]
pub(crate) struct NeedsArgs {
    /// A checkout to read, as a path or as `<host/owner/name>=<path>`. The
    /// repository's id is the normalised URL of the checkout's git remote
    /// `origin` unless the value gives one. Repeat the option for each
    /// repository of the stack. A comma is part of a path, so one use names
    /// one checkout.
    #[arg(long = "repo", value_name = "REPO", required = true, action = clap::ArgAction::Append)]
    pub(crate) repo: Vec<RepoArg>,

    /// The store to write the records to: a directory of JSON files, or a
    /// board.
    #[command(flatten)]
    pub(crate) store: StoreArgs,
}

/// What `jumpgate needs` read and wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub(crate) struct Report {
    /// Where the records were written: a directory, or a board.
    store: String,
    /// The id of the board the store writes to.
    board: String,
    /// The path of each need written.
    needs: Vec<String>,
    /// The path of each provide written.
    provides: Vec<String>,
    /// The path of each unit written.
    units: Vec<String>,
    /// The path of each record of a read app that no declaration gives any
    /// more, which the command deleted.
    removed: Vec<String>,
    /// The path of each stale record that the command kept, because a binding
    /// in the store still names it and a delete would leave the binding
    /// pointing at nothing. Each is also a failure of the result, so the
    /// command exits with 1 until the binding is removed.
    kept: Vec<String>,
    /// The `starmap.toml` files that no process reads, as `repository:file`.
    unread: Vec<String>,
    /// The services that an `mxv.toml` declares and no process lists in
    /// `needs`, as `repository:service`.
    unused: Vec<String>,
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "Wrote {} needs, {} provides and {} units to {} (board {}).",
            self.needs.len(),
            self.provides.len(),
            self.units.len(),
            self.store,
            self.board
        )?;
        for (heading, paths) in [
            ("Needs", &self.needs),
            ("Provides", &self.provides),
            ("Units", &self.units),
        ] {
            if !paths.is_empty() {
                writeln!(formatter, "{heading}:")?;
                for path in paths {
                    writeln!(formatter, "  {path}")?;
                }
            }
        }
        if !self.removed.is_empty() {
            writeln!(formatter, "Removed, because no declaration gives them now:")?;
            for path in &self.removed {
                writeln!(formatter, "  {path}")?;
            }
        }
        if !self.kept.is_empty() {
            writeln!(formatter, "Kept, because a binding still names them:")?;
            for path in &self.kept {
                writeln!(formatter, "  {path}")?;
            }
        }
        if !self.unread.is_empty() {
            writeln!(formatter, "No process reads:")?;
            for file in &self.unread {
                writeln!(formatter, "  {file}")?;
            }
        }
        if !self.unused.is_empty() {
            writeln!(
                formatter,
                "Unused services, which no process lists in needs:"
            )?;
            for service in &self.unused {
                writeln!(formatter, "  {service}")?;
            }
        }
        Ok(())
    }
}

/// Why `jumpgate needs` refused or failed.
#[derive(Debug)]
pub(crate) enum NeedsFailure {
    /// A `--repo` path is not a directory.
    RepoPath {
        /// The path as typed.
        path: String,
    },
    /// git did not give the URL of the checkout's `origin`.
    Git {
        /// The checkout's path.
        path: String,
        /// What went wrong.
        error: GitError,
    },
    /// The checkout's `origin` does not name `host/owner/name`.
    Remote {
        /// The checkout's path.
        path: String,
    },
    /// Two `--repo` values name one repository.
    Duplicate {
        /// The repository's id.
        id: String,
    },
    /// A record that no declaration gives any more is still named by a
    /// binding, so the command kept it.
    Kept {
        /// The path of the record that was kept.
        path: String,
        /// The path of the binding that names it.
        binding: String,
    },
    /// The store refused a read or a write.
    Store(StoreError),
    /// The reader refused a declaration.
    Declaration(Refusal),
}

impl fmt::Display for NeedsFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepoPath { path } => write!(formatter, "{path} is not a directory"),
            Self::Git { path, error } => match error {
                GitError::Start => write!(formatter, "git did not start for {path}"),
                GitError::Failed => {
                    write!(
                        formatter,
                        "{path} is not a git checkout with a remote named origin"
                    )
                }
                GitError::TimedOut => write!(formatter, "git did not answer for {path} in time"),
                GitError::Output => {
                    write!(
                        formatter,
                        "git printed something other than a URL for {path}"
                    )
                }
            },
            Self::Remote { path } => write!(
                formatter,
                "the remote origin of {path} is not a URL of the form host/owner/name"
            ),
            Self::Duplicate { id } => write!(formatter, "two --repo values name {id}"),
            Self::Kept { path, binding } => write!(
                formatter,
                "{path} is no longer declared, and the binding {binding} still names it, so it was kept"
            ),
            Self::Store(error) => formatter.write_str(error.message()),
            Self::Declaration(refusal) => {
                write!(formatter, "{}:{}", refusal.repo(), refusal.file())?;
                if let Some(line) = refusal.line() {
                    write!(formatter, ":{line}")?;
                }
                write!(formatter, ": {}", refusal.message())
            }
        }
    }
}

impl std::error::Error for NeedsFailure {}

impl Failure for NeedsFailure {
    fn code(&self) -> Code<'_> {
        match self {
            Self::RepoPath { .. } => codes::REPO_PATH,
            Self::Git { .. } => codes::REPO_GIT,
            Self::Remote { .. } => codes::REPO_REMOTE,
            Self::Duplicate { .. } => codes::REPO_DUPLICATE,
            Self::Kept { .. } => codes::NEED_KEPT,
            Self::Store(error) => codes::of_library(error.code()),
            Self::Declaration(refusal) => codes::of_library(refusal.code()),
        }
    }

    fn recovery(&self) -> Recovery {
        Recovery::hint(match self {
            Self::RepoPath { .. } => "pass the path of a checkout as --repo <path>",
            Self::Git { .. } => {
                "run git remote add origin <url> in the checkout, or pass --repo <host/owner/name>=<path>"
            }
            Self::Remote { .. } => "pass --repo <host/owner/name>=<path> to name the repository",
            Self::Duplicate { .. } => "pass each repository once",
            Self::Kept { .. } => {
                "remove the binding the message names, with board for a board store, then run the command again"
            }
            Self::Store(_) => {
                "fix the record or the board the message names, then run the command again"
            }
            Self::Declaration(_) => {
                "fix the declaration the message names, then run the command again"
            }
        })
    }
}

/// Reads the declarations of the checkouts that `args` names and writes
/// what they give to the store.
///
/// A refused declaration does not stop the run: the records of everything
/// else are written, and each refusal is a failure of the result, so the
/// command exits with 1.
pub(crate) fn run(args: &NeedsArgs) -> Result<Prose<Report, NeedsFailure>, NeedsFailure> {
    let checkouts = checkouts(&args.repo)?;
    let repositories: Vec<Repository<'_>> = checkouts
        .iter()
        .map(|(id, checkout)| Repository::new(id, checkout))
        .collect();
    let mut declarations = read(&repositories);
    let chosen = args.store.open().map_err(NeedsFailure::Store)?;
    let store = chosen.store();
    declarations
        .refuse_service_clashes(store)
        .map_err(NeedsFailure::Store)?;
    let mut batch: Vec<AnyRecord> = Vec::new();
    batch.extend(declarations.needs().iter().cloned().map(AnyRecord::from));
    batch.extend(declarations.provides().iter().cloned().map(AnyRecord::from));
    batch.extend(declarations.units().iter().cloned().map(AnyRecord::from));
    // A record that an app declared before and no longer declares is stale.
    // Only an app that was read without a refusal has any removed, so a
    // declaration that is refused loses nothing.
    let leftover = stale(&declarations, store).map_err(NeedsFailure::Store)?;
    // A need or a provide that a binding of the store still names stays, so
    // the store never holds a binding that points at nothing.
    let named = named_by_bindings(store).map_err(NeedsFailure::Store)?;
    let (kept, gone): (Vec<RecordPath>, Vec<RecordPath>) = leftover
        .into_iter()
        .partition(|path| named.contains_key(&path.to_string()));
    store.apply(&batch, &gone).map_err(NeedsFailure::Store)?;
    let report = Report {
        store: chosen.place().to_owned(),
        board: store.board().to_string(),
        needs: declarations
            .needs()
            .iter()
            .map(|need| need.record_path().to_string())
            .collect(),
        provides: declarations
            .provides()
            .iter()
            .map(|provide| provide.record_path().to_string())
            .collect(),
        units: declarations
            .units()
            .iter()
            .map(|unit| unit.record_path().to_string())
            .collect(),
        removed: gone.iter().map(ToString::to_string).collect(),
        kept: kept.iter().map(ToString::to_string).collect(),
        unread: declarations
            .unread()
            .iter()
            .map(|unread| format!("{}:{}", unread.repo(), unread.file()))
            .collect(),
        unused: declarations
            .unused()
            .iter()
            .map(|unused| format!("{}:{}", unused.repo(), unused.service()))
            .collect(),
    };
    let mut failures: Vec<NeedsFailure> = kept
        .iter()
        .filter_map(|path| {
            named
                .get(&path.to_string())
                .map(|binding| NeedsFailure::Kept {
                    path: path.to_string(),
                    binding: binding.clone(),
                })
        })
        .collect();
    failures.extend(
        declarations
            .refusals()
            .iter()
            .cloned()
            .map(NeedsFailure::Declaration),
    );
    Ok(Prose::with_failures(report, failures))
}

/// Returns the id and the checkout of each `--repo` value.
fn checkouts(args: &[RepoArg]) -> Result<Vec<(RepoId, DirectoryCheckout)>, NeedsFailure> {
    let mut found: Vec<(RepoId, DirectoryCheckout)> = Vec::new();
    for arg in args {
        let shown = arg.path().display().to_string();
        if !arg.path().is_dir() {
            return Err(NeedsFailure::RepoPath { path: shown });
        }
        let id = match arg.id() {
            Some(id) => id.clone(),
            None => {
                let url = git::origin_url(arg.path()).map_err(|error| NeedsFailure::Git {
                    path: shown.clone(),
                    error,
                })?;
                remote::repo_id(&url).ok_or(NeedsFailure::Remote { path: shown })?
            }
        };
        if found.iter().any(|(known, _)| *known == id) {
            return Err(NeedsFailure::Duplicate { id: id.to_string() });
        }
        found.push((id, DirectoryCheckout::new(arg.path().to_path_buf())));
    }
    Ok(found)
}
