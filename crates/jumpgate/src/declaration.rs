//! The reader of an app's declarations: `mxv.toml` for what the app runs,
//! provides and needs, and `starmap.toml` for the secrets its processes
//! read.
//!
//! The reader runs no process and opens no file. The caller gives each
//! repository as a [`RepoId`] and a [`Checkout`] that returns the text of a
//! file, so a command reads a directory and a test reads a map.
//!
//! [`read`] returns the records it found and every refusal. It never stops
//! at the first refusal: a refused need leaves the rest of its file read,
//! and a refused repository leaves every other repository read.
//!
//! # What an app provides
//!
//! | `mxv.toml` | Provide | Kind | Provide name |
//! | --- | --- | --- | --- |
//! | a name in `ports` | a port | address | the port's name |
//! | a name in `routes` | a route | address | the route's name |
//! | a process or container | a unit | app | the unit's name |
//!
//! A need names what an app provides by its bare name, `from = "api"`.
//! When a port, a route or a unit of one app share a name, the bare name is
//! ambiguous: each of their provides is named with the kind as a suffix
//! (`api-port`, `api-route`, `api-unit`), a need of the bare name is
//! refused with `jumpgate::declaration::provider`, and a need names the
//! kind by writing the suffixed name. A provide name is lowercase, so a
//! port, route or unit name with an uppercase letter is refused with
//! `jumpgate::declaration::name`.

mod mxv;
mod needs;
mod secrets;
mod services;
mod stale;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::error::Code;
use crate::file_path::RepoFilePath;
use crate::keyword::NeedKind;
use crate::name::{AppName, ProvideName, ServiceName};
use crate::path::{NeedPath, ProvidePath, UnitPath};
use crate::record::{Need, Provide, Unit};
use crate::repo::RepoId;

pub use crate::declaration::mxv::MXV_FILE;
pub use crate::declaration::secrets::STARMAP_FILE;
pub use crate::declaration::stale::{named_by_bindings, stale};

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

    /// Returns the absolute directory of the checkout on this machine. A
    /// `[provides.<name>]` table of kind `socket` names a path relative to
    /// the checkout, and the reader needs the directory to write the
    /// absolute path.
    ///
    /// # Errors
    ///
    /// Returns a [`CheckoutError`] that says why the checkout has no
    /// directory, such as that it is missing or that its path is not UTF-8
    /// text. The default says the checkout is not a directory.
    fn root(&self) -> Result<&str, CheckoutError> {
        Err(CheckoutError::new("the checkout is not a directory"))
    }

    /// Returns every `starmap.toml` that the checkout tracks, at any depth.
    /// The reader lists those that no process reads. The default lists none.
    ///
    /// # Errors
    ///
    /// Returns a [`CheckoutError`] when the checkout cannot list its files.
    fn starmap_files(&self) -> Result<Vec<RepoFilePath>, CheckoutError> {
        Ok(Vec::new())
    }
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

    /// Returns the absolute directory of the checkout, or why it has none.
    pub(crate) fn root(&self) -> Result<&str, CheckoutError> {
        self.checkout.root()
    }

    /// Returns the tracked `starmap.toml` files of the checkout.
    pub(crate) fn starmap_files(&self) -> Result<Vec<RepoFilePath>, Refusal> {
        self.checkout.starmap_files().map_err(|error| {
            Refusal::new(
                self.id,
                &RepoFilePath::known(crate::declaration::mxv::MXV_FILE),
                None,
                Reason::Unreadable,
                format!("the checkout cannot list its starmap.toml files: {error}"),
            )
        })
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
    /// A need is not a table, holds a key it does not have, lacks a key or
    /// holds a bad value.
    Need,
    /// A provide is not a table, holds a key it does not have, lacks a key,
    /// names a unit the app lacks, or holds a bad value.
    Provide,
    /// The absolute path of a socket provide is longer than a Unix socket
    /// path can be, so nothing could bind the socket.
    SocketPath,
    /// The file declares `[wants]`, which is now spelled `[needs]`.
    UseNeeds,
    /// The file declares `[offers]`, which is now spelled `[provides]`.
    UseProvides,
    /// A name, such as an app, unit, port, route or need name, breaks the
    /// rule of the record that holds it.
    Name,
    /// A `starmap.toml` does not parse as a starmap file, or a secret's
    /// reference is not one jumpgate records.
    Starmap,
    /// Two declarations of one app would write the same record, such as two
    /// variables that make one need name, or a need named like a secret.
    Clash,
    /// A need names a repository that was read and does not provide the
    /// name.
    Provider,
    /// A need or a secret has no unit to receive it.
    Consumers,
    /// A process's `cwd` is not a path within the checkout.
    Directory,
    /// A process lists a service in `needs` that its app does not declare.
    Service,
    /// A process's `starmap` key names a file that the checkout does not
    /// hold.
    StarmapMissing,
}

impl Reason {
    /// Every reason.
    pub const ALL: &'static [Self] = &[
        Self::Missing,
        Self::Unreadable,
        Self::TooLarge,
        Self::Syntax,
        Self::Need,
        Self::Provide,
        Self::SocketPath,
        Self::UseNeeds,
        Self::UseProvides,
        Self::Name,
        Self::Starmap,
        Self::Clash,
        Self::Provider,
        Self::Consumers,
        Self::Directory,
        Self::Service,
        Self::StarmapMissing,
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
                Self::Need => "need",
                Self::Provide => "provide",
                Self::SocketPath => "socket_path",
                Self::UseNeeds => "use_needs",
                Self::UseProvides => "use_provides",
                Self::Name => "name",
                Self::Starmap => "starmap",
                Self::Clash => "clash",
                Self::Provider => "provider",
                Self::Consumers => "consumers",
                Self::Directory => "directory",
                Self::Service => "service",
                Self::StarmapMissing => "starmap_missing",
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

/// A service that an app declares in `mxv.toml` and no process lists in
/// `needs`, so it gives no need and no provide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnusedService {
    repo: RepoId,
    service: ServiceName,
}

impl UnusedService {
    /// Returns the repository of the `mxv.toml` that declares the service.
    #[must_use]
    pub const fn repo(&self) -> &RepoId {
        &self.repo
    }

    /// Returns the service's name.
    #[must_use]
    pub const fn service(&self) -> &ServiceName {
        &self.service
    }
}

/// Everything [`read`] found: the records, the files it left unused and the
/// refusals.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declarations {
    read_apps: Vec<AppName>,
    refusals: Vec<Refusal>,
    needs: Vec<Need>,
    provides: Vec<Provide>,
    units: Vec<Unit>,
    unread: Vec<Unread>,
    unused: Vec<UnusedService>,
}

impl Declarations {
    /// Refuses each app whose service provide differs from the provide that the
    /// `store` holds at its path while a stored need of an app outside this
    /// run still requests that provide. The refused app gets a `clash`
    /// refusal naming the other app, and none of its records stay in
    /// `self`, so they are not written.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::store::StoreError`] when a record of the store cannot be read.
    pub fn refuse_service_clashes(
        &mut self,
        store: &(impl crate::store::Store + ?Sized),
    ) -> Result<(), crate::store::StoreError> {
        services::refuse_against_store(self, store)
    }

    /// Returns the apps whose declarations were read with no refusal, in
    /// the order of the repositories. The records of these apps are
    /// complete, so a record of one of them that is not among
    /// [`Declarations::needs`], [`Declarations::provides`] and
    /// [`Declarations::units`] is stale.
    #[must_use]
    pub fn read_apps(&self) -> &[AppName] {
        &self.read_apps
    }

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

    /// Returns the provides, in the order of their paths.
    #[must_use]
    pub fn provides(&self) -> &[Provide] {
        &self.provides
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

    /// Returns each service that an app declares and no process lists in
    /// `needs`.
    #[must_use]
    pub fn unused(&self) -> &[UnusedService] {
        &self.unused
    }
}

/// The records and refusals gathered while reading, which [`read`] turns
/// into [`Declarations`].
#[derive(Default)]
pub(crate) struct Gathered {
    pub(crate) refusals: Vec<Refusal>,
    pub(crate) needs: BTreeMap<NeedPath, Need>,
    pub(crate) provides: BTreeMap<ProvidePath, Provide>,
    pub(crate) units: BTreeMap<UnitPath, Unit>,
    pub(crate) unread: Vec<Unread>,
    pub(crate) unused: Vec<UnusedService>,
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

    /// Keeps `provide`, or refuses it when another provide has its path.
    pub(crate) fn add_provide(&mut self, provide: Provide, repo: &RepoId, file: &RepoFilePath) {
        let path = provide.path();
        match self.provides.get(&path) {
            None => {
                self.provides.insert(path, provide);
            }
            Some(kept) if *kept == provide => {}
            Some(_) => self.refusals.push(Refusal::new(
                repo,
                file,
                None,
                Reason::Clash,
                format!("two different provides would have the path {path}"),
            )),
        }
    }
}

/// Reads the declarations of `repositories`.
///
/// Each repository's app is the last segment of its id. A need learns the
/// kind of its need from the provide it asks for, which the reader finds
/// among the repositories it reads. A need of a repository that is not
/// among them keeps the kind address and is left for the matcher to report
/// as unbound.
#[must_use]
pub fn read(repositories: &[Repository<'_>]) -> Declarations {
    let mut parsed: Vec<mxv::Mxv> = repositories.iter().map(mxv::parse).collect();
    refuse_repeated_apps(&mut parsed);
    let by_repo = mxv::by_repo(&parsed);
    let service_clashes = services::clashes(&parsed);
    let mut gathered = Gathered::default();
    let mut found_secrets = Vec::new();
    let apps: Vec<(RepoId, AppName)> = parsed
        .iter()
        .filter_map(|mxv| mxv.app.clone().map(|app| (mxv.repo.clone(), app)))
        .collect();
    for (repository, mut mxv) in repositories.iter().zip(parsed) {
        mxv.keep(&mut gathered);
        found_secrets.extend(secrets::read(repository, &mxv, &mut gathered));
        needs::from_needs(&mxv, &by_repo, &mut gathered);
        services::read(&mxv, &service_clashes, &mut gathered);
    }
    secrets::finish(found_secrets, &mut gathered);
    let read_apps = apps
        .into_iter()
        .filter(|(repo, _)| {
            gathered
                .refusals
                .iter()
                .all(|refusal| refusal.repo() != repo)
        })
        .map(|(_, app)| app)
        .collect();
    Declarations {
        read_apps,
        refusals: gathered.refusals,
        needs: gathered.needs.into_values().collect(),
        provides: gathered.provides.into_values().collect(),
        units: gathered.units.into_values().collect(),
        unread: gathered.unread,
        unused: gathered.unused,
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

/// The kinds of the provides each repository provides, by repository and
/// provide name.
pub(crate) type ProvidesByRepo = BTreeMap<RepoId, RepoProvides>;

/// The provides of one repository.
#[derive(Default)]
pub(crate) struct RepoProvides {
    /// The kind of each provide, by provide name.
    pub(crate) kinds: BTreeMap<ProvideName, NeedKind>,
    /// The bare names that two kinds of the app share.
    pub(crate) shared: BTreeSet<ProvideName>,
}
