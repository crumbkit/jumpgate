//! The reader of an app's declarations: `mxv.toml` for what the app runs,
//! provides and wants, and `starmap.toml` for the secrets its processes
//! read.
//!
//! The reader runs no process and opens no file. The caller gives each
//! repository as a [`RepoId`] and a [`Checkout`] that returns the text of a
//! file, so a command reads a directory and a test reads a map.
//!
//! [`read`] returns the records it found and every refusal. It never stops
//! at the first refusal: a refused want leaves the rest of its file read,
//! and a refused repository leaves every other repository read.
//!
//! # What an app provides
//!
//! | `mxv.toml` | Offer | Kind | Offer name |
//! | --- | --- | --- | --- |
//! | a name in `ports` | a port | address | the port's name |
//! | a name in `routes` | a route | address | the route's name |
//! | a process or container | a unit | app | the unit's name |
//!
//! A want names what an app provides by its bare name, `provides = "api"`.
//! When a port, a route or a unit of one app share a name, the bare name is
//! ambiguous: each of their offers is named with the kind as a suffix
//! (`api-port`, `api-route`, `api-unit`), a want of the bare name is
//! refused with `jumpgate::declaration::provider`, and a want names the
//! kind by writing the suffixed name. An offer name is lowercase, so a
//! port, route or unit name with an uppercase letter is refused with
//! `jumpgate::declaration::name`.

mod mxv;
mod needs;
mod secrets;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::error::Code;
use crate::file_path::RepoFilePath;
use crate::keyword::NeedKind;
use crate::name::OfferName;
use crate::path::{NeedPath, OfferPath, UnitPath};
use crate::record::{Need, Offer, Unit};
use crate::repo::RepoId;

pub use crate::declaration::mxv::MXV_FILE;
pub use crate::declaration::secrets::STARMAP_FILE;

/// The most bytes of a declaration file that the reader reads. A longer
/// file is refused with [`Reason::TooLarge`].
pub const MAX_DECLARATION_BYTES: usize = 1024 * 1024;

/// The files of one repository's checkout.
pub trait Checkout {
    /// Returns the text of `path`, or `None` when the checkout has no such
    /// file.
    ///
    /// # Errors
    ///
    /// Returns a [`CheckoutError`] when the file exists and cannot be read.
    fn read(&self, path: &RepoFilePath) -> Result<Option<String>, CheckoutError>;
}

/// A file that exists in a checkout and cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutError {
    message: Cow<'static, str>,
}

impl CheckoutError {
    /// Builds the error with `message`, which says why the file cannot be
    /// read and never quotes its content.
    #[must_use]
    pub fn new(message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CheckoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CheckoutError {}

/// One repository to read: its id and the files of its checkout.
#[derive(Clone, Copy)]
pub struct Repository<'a> {
    id: &'a RepoId,
    checkout: &'a dyn Checkout,
}

impl fmt::Debug for Repository<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Repository")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl<'a> Repository<'a> {
    /// Names the repository `id`, whose files `checkout` returns.
    #[must_use]
    pub const fn new(id: &'a RepoId, checkout: &'a dyn Checkout) -> Self {
        Self { id, checkout }
    }

    /// Returns the repository's id.
    #[must_use]
    pub const fn id(&self) -> &RepoId {
        self.id
    }

    /// Reads `file`, refusing a file that is unreadable or too long.
    pub(crate) fn text(&self, file: &RepoFilePath) -> Result<Option<String>, Refusal> {
        match self.checkout.read(file) {
            Err(error) => Err(Refusal::new(
                self.id,
                file,
                None,
                Reason::Unreadable,
                format!("the file cannot be read: {error}"),
            )),
            Ok(Some(text)) if text.len() > MAX_DECLARATION_BYTES => Err(Refusal::new(
                self.id,
                file,
                None,
                Reason::TooLarge,
                format!(
                    "the file has {} bytes, and the reader reads at most {MAX_DECLARATION_BYTES}",
                    text.len()
                ),
            )),
            Ok(text) => Ok(text),
        }
    }
}

/// Why the reader refused a declaration. Each reason has the code
/// `jumpgate::declaration::<name>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Reason {
    /// The repository has no `mxv.toml`.
    Missing,
    /// A file exists and the checkout cannot read it.
    Unreadable,
    /// A file is longer than [`MAX_DECLARATION_BYTES`].
    TooLarge,
    /// A file is not valid TOML, or a table of `mxv.toml` that the reader
    /// uses has a value of the wrong type.
    Syntax,
    /// A want is not a table, holds a key it does not have, lacks a key or
    /// holds a bad value.
    Want,
    /// A name, such as an app, unit, port, route or need name, breaks the
    /// rule of the record that holds it.
    Name,
    /// A `starmap.toml` does not parse as a starmap file, or a secret's
    /// reference is not one jumpgate records.
    Starmap,
    /// Two declarations of one app would write the same record, such as two
    /// variables that make one need name, or a want named like a secret.
    Clash,
    /// A want names a repository that was read and does not provide the
    /// name.
    Provider,
    /// A want or a secret has no unit to receive it.
    Consumers,
    /// A process's `cwd` is not a path within the checkout.
    Directory,
}

impl Reason {
    /// Every reason.
    pub const ALL: &'static [Self] = &[
        Self::Missing,
        Self::Unreadable,
        Self::TooLarge,
        Self::Syntax,
        Self::Want,
        Self::Name,
        Self::Starmap,
        Self::Clash,
        Self::Provider,
        Self::Consumers,
        Self::Directory,
    ];

    /// Returns the reason's failure code, `jumpgate::declaration::<name>`.
    #[must_use]
    pub const fn code(self) -> Code {
        Code::new(
            "declaration",
            match self {
                Self::Missing => "missing",
                Self::Unreadable => "unreadable",
                Self::TooLarge => "too_large",
                Self::Syntax => "syntax",
                Self::Want => "want",
                Self::Name => "name",
                Self::Starmap => "starmap",
                Self::Clash => "clash",
                Self::Provider => "provider",
                Self::Consumers => "consumers",
                Self::Directory => "directory",
            },
        )
    }
}

/// A declaration the reader refused, with where it is and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    repo: RepoId,
    file: RepoFilePath,
    line: Option<usize>,
    reason: Reason,
    message: String,
}

impl Refusal {
    pub(crate) fn new(
        repo: &RepoId,
        file: &RepoFilePath,
        line: Option<usize>,
        reason: Reason,
        message: impl Into<String>,
    ) -> Self {
        Self {
            repo: repo.clone(),
            file: file.clone(),
            line,
            reason,
            message: message.into(),
        }
    }

    /// Returns the repository of the file.
    #[must_use]
    pub const fn repo(&self) -> &RepoId {
        &self.repo
    }

    /// Returns the file.
    #[must_use]
    pub const fn file(&self) -> &RepoFilePath {
        &self.file
    }

    /// Returns the line, counted from 1, where the refused declaration
    /// starts, when the reader knows it.
    #[must_use]
    pub const fn line(&self) -> Option<usize> {
        self.line
    }

    /// Returns why the declaration was refused.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        self.reason
    }

    /// Returns the failure code.
    #[must_use]
    pub const fn code(&self) -> Code {
        self.reason.code()
    }

    /// Returns what is wrong. It never quotes a secret.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// A `starmap.toml` that no process of its app reads, so it gives no need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    repo: RepoId,
    file: RepoFilePath,
}

impl Unread {
    /// Returns the repository of the file.
    #[must_use]
    pub const fn repo(&self) -> &RepoId {
        &self.repo
    }

    /// Returns the file.
    #[must_use]
    pub const fn file(&self) -> &RepoFilePath {
        &self.file
    }
}

/// Everything [`read`] found: the records, the files it left unused and the
/// refusals.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declarations {
    refusals: Vec<Refusal>,
    needs: Vec<Need>,
    offers: Vec<Offer>,
    units: Vec<Unit>,
    unread: Vec<Unread>,
}

impl Declarations {
    /// Returns every refusal, in the order of the repositories read.
    #[must_use]
    pub fn refusals(&self) -> &[Refusal] {
        &self.refusals
    }

    /// Returns the needs, in the order of their paths.
    #[must_use]
    pub fn needs(&self) -> &[Need] {
        &self.needs
    }

    /// Returns the offers, in the order of their paths.
    #[must_use]
    pub fn offers(&self) -> &[Offer] {
        &self.offers
    }

    /// Returns the units, in the order of their paths.
    #[must_use]
    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    /// Returns each `starmap.toml` that no process reads.
    #[must_use]
    pub fn unread(&self) -> &[Unread] {
        &self.unread
    }
}

/// The records and refusals gathered while reading, which [`read`] turns
/// into [`Declarations`].
#[derive(Default)]
pub(crate) struct Gathered {
    pub(crate) refusals: Vec<Refusal>,
    pub(crate) needs: BTreeMap<NeedPath, Need>,
    pub(crate) offers: BTreeMap<OfferPath, Offer>,
    pub(crate) units: BTreeMap<UnitPath, Unit>,
    pub(crate) unread: Vec<Unread>,
}

impl Gathered {
    /// Keeps `need`, or refuses it when its app already has a need of the
    /// same name.
    pub(crate) fn add_need(&mut self, need: Need, file: &RepoFilePath, line: Option<usize>) {
        let path = need.path();
        match self.needs.entry(path) {
            Entry::Occupied(_) => self.refusals.push(Refusal::new(
                need.declared_in().repo(),
                file,
                line,
                Reason::Clash,
                format!("the app already has a need named {}", need.name()),
            )),
            Entry::Vacant(slot) => {
                slot.insert(need);
            }
        }
    }

    /// Keeps `offer`, or refuses it when another offer has its path.
    pub(crate) fn add_offer(&mut self, offer: Offer, repo: &RepoId, file: &RepoFilePath) {
        let path = offer.path();
        match self.offers.get(&path) {
            None => {
                self.offers.insert(path, offer);
            }
            Some(kept) if *kept == offer => {}
            Some(_) => self.refusals.push(Refusal::new(
                repo,
                file,
                None,
                Reason::Clash,
                format!("two different offers would have the path {path}"),
            )),
        }
    }
}

/// Reads the declarations of `repositories`.
///
/// Each repository's app is the last segment of its id. A want learns the
/// kind of its need from the offer it asks for, which the reader finds
/// among the repositories it reads. A want of a repository that is not
/// among them keeps the kind address and is left for the matcher to report
/// as unbound.
#[must_use]
pub fn read(repositories: &[Repository<'_>]) -> Declarations {
    let mut parsed: Vec<mxv::Mxv> = repositories.iter().map(mxv::parse).collect();
    refuse_repeated_apps(&mut parsed);
    let provided = mxv::provided(&parsed);
    let mut gathered = Gathered::default();
    let mut found_secrets = Vec::new();
    for (repository, mut mxv) in repositories.iter().zip(parsed) {
        mxv.keep(&mut gathered);
        found_secrets.extend(secrets::read(repository, &mxv, &mut gathered));
        needs::from_wants(&mxv, &provided, &mut gathered);
    }
    secrets::finish(found_secrets, &mut gathered);
    Declarations {
        refusals: gathered.refusals,
        needs: gathered.needs.into_values().collect(),
        offers: gathered.offers.into_values().collect(),
        units: gathered.units.into_values().collect(),
        unread: gathered.unread,
    }
}

/// Refuses each repository whose app is the app of an earlier repository,
/// so two repositories whose ids end in one segment never write one app.
fn refuse_repeated_apps(parsed: &mut [mxv::Mxv]) {
    let mut seen: BTreeMap<crate::name::AppName, RepoId> = BTreeMap::new();
    for mxv in parsed.iter_mut() {
        let Some(app) = mxv.app.clone() else {
            continue;
        };
        match seen.entry(app) {
            Entry::Vacant(slot) => {
                slot.insert(mxv.repo.clone());
            }
            Entry::Occupied(first) => {
                let refusal = Refusal::new(
                    &mxv.repo,
                    &RepoFilePath::known(MXV_FILE),
                    None,
                    Reason::Clash,
                    format!(
                        "the app {} is already the app of {}, since an app is the last segment of its repository id",
                        first.key(),
                        first.get()
                    ),
                );
                let mut refusals = std::mem::take(&mut mxv.refusals);
                refusals.push(refusal);
                *mxv = mxv::Mxv::empty(&mxv.repo.clone(), refusals);
            }
        }
    }
}

/// The kinds of the offers each repository provides, by repository and
/// offer name.
pub(crate) type Provided = BTreeMap<RepoId, Offered>;

/// The offers of one repository.
#[derive(Default)]
pub(crate) struct Offered {
    /// The kind of each offer, by offer name.
    pub(crate) kinds: BTreeMap<OfferName, NeedKind>,
    /// The bare names that two kinds of the app share.
    pub(crate) shared: BTreeSet<OfferName>,
}
