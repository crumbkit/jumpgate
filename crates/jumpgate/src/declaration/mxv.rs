//! The reader of `mxv.toml`: the app's units, the provides it provides and
//! the needs it declares.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::declaration::{
    CheckoutError, Gathered, ProvidesByRepo, Reason, Refusal, RepoProvides, Repository,
};
use crate::distinct::Distinct;
use crate::file_path::{AbsoluteFilePath, RepoFilePath};
use crate::keyword::{NeedKind, ProviderKind, Scope, UnitKind};
use crate::name::{AppName, NeedName, PortName, ProvideName, ProviderName, TaskName, UnitName};
use crate::record::{
    PortReference, Provide, Provider, Reference, RouteReference, SocketReference, Unit,
    UnitReference,
};
use crate::repo::RepoId;
use crate::variable::VariableName;

/// The name of the file that declares what an app runs, provides and needs.
pub const MXV_FILE: &str = "mxv.toml";

/// The kind of thing a provide of an app stands for. It is the suffix of the
/// provide's name when two kinds of one app share a name.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Port,
    Route,
    Unit,
}

impl Kind {
    const fn suffix(self) -> &'static str {
        match self {
            Self::Port => "-port",
            Self::Route => "-route",
            Self::Unit => "-unit",
        }
    }
}

/// A provide whose name waits until every name of the app is known.
struct Pending {
    base: String,
    kind: Kind,
    offset: Option<usize>,
    reference: Reference,
}

/// A process or container, and the `starmap.toml` its process reads.
pub(crate) struct ParsedUnit {
    pub(crate) unit: Unit,
    /// The `starmap.toml` that a process reads, which sits in the process's
    /// directory. A container reads none.
    pub(crate) starmap: Option<RepoFilePath>,
}

impl ParsedUnit {
    /// Returns the unit's name.
    pub(crate) fn name(&self) -> UnitName {
        self.unit.name().clone()
    }
}

/// One `[needs.<name>]` table, parsed.
pub(crate) struct NeedDeclaration {
    pub(crate) name: NeedName,
    pub(crate) repo: RepoId,
    pub(crate) from: ProvideName,
    pub(crate) delivers_as: VariableName,
    pub(crate) optional: bool,
    pub(crate) line: usize,
}

/// What one repository's `mxv.toml` declares.
pub(crate) struct Mxv {
    pub(crate) repo: RepoId,
    /// The app, which is `None` when the repository has no usable
    /// `mxv.toml`.
    pub(crate) app: Option<AppName>,
    pub(crate) units: Vec<ParsedUnit>,
    /// The names of the app's tasks, which are its commands.
    pub(crate) tasks: Vec<TaskName>,
    pub(crate) provides: Vec<Provide>,
    /// The bare names that two kinds of the app share. Their provides carry
    /// the kind as a suffix.
    pub(crate) shared: BTreeSet<ProvideName>,
    pub(crate) needs: Vec<NeedDeclaration>,
    pub(crate) refusals: Vec<Refusal>,
}

impl Mxv {
    pub(crate) fn empty(repo: &RepoId, refusals: Vec<Refusal>) -> Self {
        Self {
            repo: repo.clone(),
            app: None,
            units: Vec::new(),
            tasks: Vec::new(),
            provides: Vec::new(),
            shared: BTreeSet::new(),
            needs: Vec::new(),
            refusals,
        }
    }

    /// Moves the refusals, provides and units into `gathered`.
    pub(crate) fn keep(&mut self, gathered: &mut Gathered) {
        let file = RepoFilePath::known(MXV_FILE);
        gathered.refusals.append(&mut self.refusals);
        for provide in std::mem::take(&mut self.provides) {
            gathered.add_provide(provide, &self.repo, &file);
        }
        for parsed in &self.units {
            gathered
                .units
                .insert(parsed.unit.path(), parsed.unit.clone());
        }
    }
}

/// Returns the kind of each provide that the parsed repositories provide.
pub(crate) fn by_repo(parsed: &[Mxv]) -> ProvidesByRepo {
    let mut by_repo = ProvidesByRepo::new();
    for mxv in parsed.iter().filter(|mxv| mxv.app.is_some()) {
        let repo_provides: &mut RepoProvides = by_repo.entry(mxv.repo.clone()).or_default();
        for provide in &mxv.provides {
            repo_provides
                .kinds
                .insert(provide.name().clone(), provide.kind());
        }
        repo_provides.shared.clone_from(&mxv.shared);
    }
    by_repo
}

/// The tables of `mxv.toml` that the reader uses. It reads no other key, so
/// a table of another tool, and every key that mxv owns, passes through.
#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    ports: Vec<Spanned<String>>,
    #[serde(default)]
    routes: Vec<Spanned<String>>,
    #[serde(default)]
    processes: BTreeMap<Spanned<String>, RawUnit>,
    #[serde(default)]
    containers: BTreeMap<Spanned<String>, RawUnit>,
    #[serde(default)]
    needs: BTreeMap<Spanned<String>, Spanned<toml::Value>>,
    #[serde(default)]
    provides: BTreeMap<Spanned<String>, Spanned<toml::Value>>,
    #[serde(default)]
    tasks: BTreeMap<Spanned<String>, toml::Value>,
    /// The spelling that `[needs]` replaced.
    wants: Option<Spanned<toml::Value>>,
    /// The spelling that `[provides]` replaced.
    offers: Option<Spanned<toml::Value>>,
}

/// The keys of a process or container that the reader uses.
#[derive(Deserialize)]
struct RawUnit {
    cwd: Option<String>,
    #[serde(default)]
    depends_on: Vec<Spanned<String>>,
    listen: Option<String>,
    #[serde(default)]
    ports: BTreeMap<String, String>,
}

/// Returns the line, counted from 1, at byte `offset` of `text`.
pub(crate) fn line_at(text: &str, offset: usize) -> usize {
    text.get(..offset.min(text.len()))
        .map_or(0, |before| before.matches('\n').count())
        + 1
}

/// Reads the `mxv.toml` of `repository`.
pub(crate) fn parse(repository: &Repository<'_>) -> Mxv {
    let repo = repository.id();
    let file = RepoFilePath::known(MXV_FILE);
    let text = match repository.text(&file) {
        Ok(Some(text)) => text,
        Ok(None) => {
            let refusal = Refusal::new(
                repo,
                &file,
                None,
                Reason::Missing,
                "the repository has no mxv.toml at its root",
            );
            return Mxv::empty(repo, vec![refusal]);
        }
        Err(refusal) => return Mxv::empty(repo, vec![refusal]),
    };
    let app = match app_of(repo) {
        Ok(app) => app,
        Err(message) => {
            let refusal = Refusal::new(repo, &file, None, Reason::Name, message);
            return Mxv::empty(repo, vec![refusal]);
        }
    };
    let raw: Raw = match toml::from_str(&text) {
        Ok(raw) => raw,
        Err(error) => {
            let line = error.span().map(|span| line_at(&text, span.start));
            let message = format!("mxv.toml is not valid: {}", error.message());
            return Mxv::empty(
                repo,
                vec![Refusal::new(repo, &file, line, Reason::Syntax, message)],
            );
        }
    };
    let mut reader = Reader {
        repo,
        file: &file,
        text: &text,
        app: &app,
        root: repository.root(),
        mxv: Mxv::empty(repo, Vec::new()),
        pending: Vec::new(),
    };
    reader.old_spellings(&raw);
    reader.units(&raw);
    reader.tasks(&raw.tasks);
    reader.ports(&raw);
    reader.name_provides();
    reader.provides(&raw.provides);
    reader.needs(&raw.needs);
    reader.mxv.app = Some(app.clone());
    reader.mxv
}

/// Returns the app of `repo`, its last segment.
fn app_of(repo: &RepoId) -> Result<AppName, String> {
    repo.app().map_err(|error| {
        format!(
            "the last segment of the repository id is not an app name: {}",
            error
        )
    })
}

/// The state of one `mxv.toml` while the reader walks it.
struct Reader<'a> {
    repo: &'a RepoId,
    file: &'a RepoFilePath,
    text: &'a str,
    app: &'a AppName,
    /// The absolute directory of the checkout, if it has one.
    root: Result<&'a str, CheckoutError>,
    mxv: Mxv,
    pending: Vec<Pending>,
}

impl Reader<'_> {
    fn refuse(&mut self, offset: Option<usize>, reason: Reason, message: impl Into<String>) {
        let line = offset.map(|offset| line_at(self.text, offset));
        self.mxv
            .refusals
            .push(Refusal::new(self.repo, self.file, line, reason, message));
    }

    fn provider(&self) -> Option<Provider> {
        let name: ProviderName = self.app.as_str().parse().ok()?;
        Some(Provider::new(ProviderKind::App, name))
    }

    /// Refuses the `[wants]` and `[offers]` spellings, and names the ones
    /// that replaced them.
    fn old_spellings(&mut self, raw: &Raw) {
        if let Some(wants) = &raw.wants {
            self.refuse(
                Some(wants.span().start),
                Reason::UseNeeds,
                "mxv.toml declares [wants], which is now spelled [needs.<name>], with the key from in place of provides",
            );
        }
        if let Some(offers) = &raw.offers {
            self.refuse(
                Some(offers.span().start),
                Reason::UseProvides,
                "mxv.toml declares [offers], which is now spelled [provides.<name>], and each provide is a socket with the new keys kind, unit and path",
            );
        }
    }

    /// Reads the names of the tasks, which are the app's commands.
    fn tasks(&mut self, tasks: &BTreeMap<Spanned<String>, toml::Value>) {
        for name in tasks.keys() {
            match name.as_ref().parse::<TaskName>() {
                Ok(task) => self.mxv.tasks.push(task),
                Err(error) => self.refuse(
                    Some(name.span().start),
                    Reason::Name,
                    format!("a task name is refused: {error}"),
                ),
            }
        }
    }

    /// Reads the `[provides.<name>]` tables.
    fn provides(&mut self, provides: &BTreeMap<Spanned<String>, Spanned<toml::Value>>) {
        for (name, value) in provides {
            let line = Some(name.span().start);
            match self.provide(name.as_ref(), value) {
                Ok(provide) => match too_long(&provide) {
                    Some(message) => self.refuse(line, Reason::SocketPath, message),
                    None => self.mxv.provides.push(provide),
                },
                Err(message) => self.refuse(line, Reason::Provide, message),
            }
        }
    }

    /// Reads one provide, or says why it is refused. The message never
    /// quotes a value of the file.
    fn provide(&self, name: &str, value: &Spanned<toml::Value>) -> Result<Provide, String> {
        let toml::Value::Table(table) = value.as_ref() else {
            return Err("a provide is a table with the keys kind, unit and path".to_owned());
        };
        if table
            .keys()
            .any(|key| !PROVIDE_KEYS.contains(&key.as_str()))
        {
            return Err("a provide holds a key other than kind, unit and path".to_owned());
        }
        let string = |key: &str| -> Result<&str, String> {
            table
                .get(key)
                .ok_or_else(|| format!("a provide needs the key {key}"))?
                .as_str()
                .ok_or_else(|| format!("the key {key} of a provide is a string"))
        };
        if string("kind")? != "socket" {
            return Err("the key kind of a provide is socket, the only kind so far".to_owned());
        }
        let unit: UnitName = string("unit")?
            .parse()
            .map_err(|error: crate::RecordError| {
                format!("the key unit of a provide is refused: {error}")
            })?;
        if !self
            .mxv
            .units
            .iter()
            .any(|parsed| parsed.unit.name() == &unit)
        {
            return Err(
                "the key unit of a provide names a process or container the app does not declare"
                    .to_owned(),
            );
        }
        let relative: RepoFilePath =
            string("path")?
                .parse()
                .map_err(|error: crate::RecordError| {
                    format!("the key path of a provide is refused: {error}")
                })?;
        let root = self.root.as_ref().map_err(|error| {
            format!("the path of a socket has no absolute form, because {error}")
        })?;
        let path: AbsoluteFilePath = format!("{}/{}", root.trim_end_matches('/'), relative)
            .parse()
            .map_err(|error: crate::RecordError| {
                format!("the directory of the checkout and the key path make a path that is refused: {error}")
            })?;
        let provider = self
            .provider()
            .ok_or_else(|| "the app is not a provider name".to_owned())?;
        let provide_name: ProvideName = name.parse().map_err(|error: crate::RecordError| {
            format!("the name of a provide is refused: {error}")
        })?;
        Provide::new(
            provider,
            NeedKind::Socket,
            provide_name,
            Reference::Socket(SocketReference::new(self.app.clone(), unit, path)),
            Scope::AppInstance,
        )
        .map_err(|error| format!("the provide is refused: {error}"))
    }

    /// Reads the processes and then the containers.
    fn units(&mut self, raw: &Raw) {
        for (name, unit) in &raw.processes {
            self.unit(name, unit, UnitKind::Process);
        }
        for (name, unit) in &raw.containers {
            self.unit(name, unit, UnitKind::Container);
        }
    }

    fn unit(&mut self, name: &Spanned<String>, raw: &RawUnit, kind: UnitKind) {
        let offset = Some(name.span().start);
        let unit_name: UnitName = match name.as_ref().parse() {
            Ok(unit_name) => unit_name,
            Err(error) => {
                self.refuse(
                    offset,
                    Reason::Name,
                    format!("a unit name is refused: {}", error),
                );
                return;
            }
        };
        let Some(unit) = self.build_unit(name, &unit_name, raw, kind) else {
            return;
        };
        let starmap = match kind {
            UnitKind::Process => self.starmap_of(offset, raw.cwd.as_deref()),
            UnitKind::Container => None,
        };
        self.unit_provide(&unit_name, offset);
        self.mxv.units.push(ParsedUnit { unit, starmap });
    }

    fn build_unit(
        &mut self,
        name: &Spanned<String>,
        unit_name: &UnitName,
        raw: &RawUnit,
        kind: UnitKind,
    ) -> Option<Unit> {
        let offset = Some(name.span().start);
        let unit = match Unit::new(self.app.clone(), unit_name.clone(), self.repo.clone(), kind) {
            Ok(unit) => unit,
            Err(error) => {
                self.refuse(
                    offset,
                    Reason::Name,
                    format!("a unit name is refused: {}", error),
                );
                return None;
            }
        };
        let mut after = Vec::new();
        for entry in &raw.depends_on {
            match entry.as_ref().parse::<UnitName>() {
                Ok(parsed) if !after.contains(&parsed) => after.push(parsed),
                Ok(_) => {}
                Err(error) => {
                    self.refuse(
                        Some(entry.span().start),
                        Reason::Name,
                        format!("a unit in depends_on is refused: {}", error),
                    );
                    return None;
                }
            }
        }
        let mut served: Vec<PortName> = Vec::new();
        for port in raw.listen.iter().chain(raw.ports.values()) {
            match port.parse::<PortName>() {
                Ok(parsed) if !served.contains(&parsed) => served.push(parsed),
                Ok(_) => {}
                Err(error) => {
                    self.refuse(
                        offset,
                        Reason::Name,
                        format!("a port of the unit is refused: {}", error),
                    );
                    return None;
                }
            }
        }
        // Both lists are built without a repeat, so `Distinct::new` cannot refuse them.
        let after = Distinct::new(after).ok()?;
        let served = Distinct::new(served).ok()?;
        Some(unit.with_starts_after(after).with_ports(served))
    }

    /// Returns the `starmap.toml` of a process whose directory is `cwd`.
    fn starmap_of(&mut self, offset: Option<usize>, cwd: Option<&str>) -> Option<RepoFilePath> {
        let directory = cwd
            .map(|cwd| cwd.trim_start_matches("./").trim_end_matches('/'))
            .filter(|cwd| !matches!(*cwd, "" | "."));
        let path = directory.map_or_else(
            || crate::declaration::secrets::STARMAP_FILE.to_owned(),
            |directory| format!("{directory}/{}", crate::declaration::secrets::STARMAP_FILE),
        );
        match path.parse() {
            Ok(path) => Some(path),
            Err(_) => {
                self.refuse(
                    offset,
                    Reason::Directory,
                    "a process's cwd is a path within the checkout, with no '..' segment and no leading '/'",
                );
                None
            }
        }
    }

    fn unit_provide(&mut self, unit: &UnitName, offset: Option<usize>) {
        self.pending.push(Pending {
            base: unit.to_string(),
            kind: Kind::Unit,
            offset,
            reference: Reference::Unit(UnitReference::new(self.app.clone(), unit.clone())),
        });
    }

    /// Reads the ports and the routes.
    fn ports(&mut self, raw: &Raw) {
        for port in &raw.ports {
            self.port_provide(port, Kind::Port);
        }
        for route in &raw.routes {
            self.port_provide(route, Kind::Route);
        }
    }

    fn port_provide(&mut self, port: &Spanned<String>, kind: Kind) {
        let offset = Some(port.span().start);
        let Ok(port_name) = port.as_ref().parse::<PortName>() else {
            self.refuse(offset, Reason::Name, "a port or route name is not a name");
            return;
        };
        let reference = match kind {
            Kind::Route => Reference::Route(RouteReference::new(self.app.clone(), port_name)),
            Kind::Port | Kind::Unit => {
                Reference::Port(PortReference::new(self.app.clone(), port_name))
            }
        };
        self.pending.push(Pending {
            base: port.as_ref().clone(),
            kind,
            offset,
            reference,
        });
    }

    /// Names and builds the provides. A provide takes the bare name of its
    /// port, route or unit, so a need writes `from = "api"`. When two
    /// kinds of the app share a name, each of their provides takes the kind
    /// as a suffix, and a need writes the suffixed name.
    fn name_provides(&mut self) {
        let mut kinds: BTreeMap<String, BTreeSet<Kind>> = BTreeMap::new();
        for pending in &self.pending {
            kinds
                .entry(pending.base.clone())
                .or_default()
                .insert(pending.kind);
        }
        let Some(provider) = self.provider() else {
            return;
        };
        for pending in std::mem::take(&mut self.pending) {
            let shared = kinds.get(&pending.base).is_some_and(|set| set.len() > 1);
            let name = if shared {
                format!("{}{}", pending.base, pending.kind.suffix())
            } else {
                pending.base.clone()
            };
            if shared && let Ok(bare) = pending.base.parse::<ProvideName>() {
                self.mxv.shared.insert(bare);
            }
            let provide = name.parse::<ProvideName>().and_then(|name| {
                Provide::new(
                    provider.clone(),
                    match pending.kind {
                        Kind::Unit => NeedKind::App,
                        Kind::Port | Kind::Route => NeedKind::Address,
                    },
                    name,
                    pending.reference,
                    Scope::AppInstance,
                )
            });
            match provide {
                Ok(provide) => self.mxv.provides.push(provide),
                Err(error) => self.refuse(
                    pending.offset,
                    Reason::Name,
                    format!(
                        "a provide name is lowercase letters, digits, '-' and '_', so a port, route or unit name with another character is refused: {error}"
                    ),
                ),
            }
        }
    }

    /// Reads the `[needs.<name>]` tables.
    fn needs(&mut self, needs: &BTreeMap<Spanned<String>, Spanned<toml::Value>>) {
        for (name, value) in needs {
            let line = Some(name.span().start);
            match need(name.as_ref(), value, self.text) {
                Ok(need) => self.mxv.needs.push(need),
                Err(message) => self.refuse(line, Reason::Need, message),
            }
        }
    }
}

/// The keys a provide holds.
const PROVIDE_KEYS: [&str; 3] = ["kind", "unit", "path"];

/// The keys a need holds.
const NEED_KEYS: [&str; 4] = ["repo", "from", "as", "optional"];

/// Reads one need, or says why it is refused. The message never quotes a
/// value of the file.
fn need(name: &str, value: &Spanned<toml::Value>, text: &str) -> Result<NeedDeclaration, String> {
    let toml::Value::Table(table) = value.as_ref() else {
        return Err("a need is a table with the keys repo, from, as and optional".to_owned());
    };
    if table.keys().any(|key| !NEED_KEYS.contains(&key.as_str())) {
        return Err("a need holds a key other than repo, from, as and optional".to_owned());
    }
    let string = |key: &str| -> Result<&str, String> {
        table
            .get(key)
            .ok_or_else(|| format!("a need needs the key {key}"))?
            .as_str()
            .ok_or_else(|| format!("the key {key} of a need is a string"))
    };
    let optional = match table.get("optional") {
        None => false,
        Some(toml::Value::Boolean(optional)) => *optional,
        Some(_) => return Err("the key optional of a need is true or false".to_owned()),
    };
    let span: Range<usize> = value.span();
    Ok(NeedDeclaration {
        name: name.parse().map_err(|error: crate::RecordError| {
            format!("the name of a need is refused: {}", error)
        })?,
        repo: string("repo")?
            .parse()
            .map_err(|error: crate::RecordError| {
                format!("the key repo of a need is refused: {}", error)
            })?,
        from: string("from")?
            .parse()
            .map_err(|error: crate::RecordError| {
                format!("the key from of a need is refused: {}", error)
            })?,
        delivers_as: string("as")?.parse().map_err(|error: crate::RecordError| {
            format!("the key as of a need is refused: {}", error)
        })?,
        optional,
        line: line_at(text, span.start),
    })
}

/// The most bytes of a socket's path that the platform's `sun_path` holds,
/// which is its size without the closing NUL byte: 104 on macOS and 108 on
/// Linux.
#[cfg(target_os = "macos")]
pub(crate) const MAX_SOCKET_PATH_BYTES: usize = 103;
/// The most bytes of a socket's path that the platform's `sun_path` holds,
/// which is its size without the closing NUL byte: 104 on macOS and 108 on
/// Linux.
#[cfg(not(target_os = "macos"))]
pub(crate) const MAX_SOCKET_PATH_BYTES: usize = 107;

/// Says why the socket of `provide` cannot be bound, when its path is longer
/// than [`MAX_SOCKET_PATH_BYTES`].
fn too_long(provide: &Provide) -> Option<String> {
    let Reference::Socket(socket) = provide.reference() else {
        return None;
    };
    let length = socket.path().as_str().len();
    (length > MAX_SOCKET_PATH_BYTES).then(|| {
        format!(
            "the path of the socket is {length} bytes, and this platform binds a socket path of at most {MAX_SOCKET_PATH_BYTES} bytes, so the checkout is too deep or the key path too long"
        )
    })
}
