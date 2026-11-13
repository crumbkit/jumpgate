//! The reader of `mxv.toml`: the app's units, the offers it provides and
//! the wants it declares.

use std::collections::BTreeMap;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::declaration::{Gathered, Provided, Reason, Refusal, Repository};
use crate::distinct::Distinct;
use crate::file_path::RepoFilePath;
use crate::keyword::{NeedKind, ProviderKind, Scope, UnitKind};
use crate::name::{AppName, NeedName, OfferName, PortName, ProviderName, UnitName};
use crate::record::{
    Offer, PortReference, Provider, Reference, RouteReference, Unit, UnitReference,
};
use crate::repo::RepoId;
use crate::variable::VariableName;

/// The name of the file that declares what an app runs, provides and wants.
pub const MXV_FILE: &str = "mxv.toml";

/// The suffix of the name of a route's offer.
const ROUTE_SUFFIX: &str = "-route";

/// The suffix of the name of a unit's offer.
const UNIT_SUFFIX: &str = "-unit";

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

/// One `[wants.<name>]` table, parsed.
pub(crate) struct WantDeclaration {
    pub(crate) name: NeedName,
    pub(crate) repo: RepoId,
    pub(crate) provides: OfferName,
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
    pub(crate) offers: Vec<Offer>,
    pub(crate) wants: Vec<WantDeclaration>,
    pub(crate) refusals: Vec<Refusal>,
}

impl Mxv {
    fn empty(repo: &RepoId, refusals: Vec<Refusal>) -> Self {
        Self {
            repo: repo.clone(),
            app: None,
            units: Vec::new(),
            offers: Vec::new(),
            wants: Vec::new(),
            refusals,
        }
    }

    /// Moves the refusals, offers and units into `gathered`.
    pub(crate) fn keep(&mut self, gathered: &mut Gathered) {
        let file = RepoFilePath::known(MXV_FILE);
        gathered.refusals.append(&mut self.refusals);
        for offer in std::mem::take(&mut self.offers) {
            gathered.add_offer(offer, &self.repo, &file);
        }
        for parsed in &self.units {
            gathered
                .units
                .insert(parsed.unit.path(), parsed.unit.clone());
        }
    }
}

/// Returns the kind of each offer that the parsed repositories provide.
pub(crate) fn provided(parsed: &[Mxv]) -> Provided {
    let mut provided = Provided::new();
    for mxv in parsed.iter().filter(|mxv| mxv.app.is_some()) {
        let names = provided.entry(mxv.repo.clone()).or_default();
        for offer in &mxv.offers {
            names.insert(offer.name().clone(), offer.kind());
        }
    }
    provided
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
    wants: BTreeMap<Spanned<String>, Spanned<toml::Value>>,
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
        mxv: Mxv::empty(repo, Vec::new()),
    };
    reader.units(&raw);
    reader.ports(&raw);
    reader.wants(&raw.wants);
    reader.mxv.app = Some(app.clone());
    reader.mxv
}

/// Returns the app of `repo`, its last segment.
fn app_of(repo: &RepoId) -> Result<AppName, String> {
    let last = repo.as_str().rsplit('/').next().unwrap_or_default();
    last.parse().map_err(|error: crate::RecordError| {
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
    mxv: Mxv,
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
        self.unit_offer(&unit_name, offset);
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

    fn unit_offer(&mut self, unit: &UnitName, offset: Option<usize>) {
        let Some(provider) = self.provider() else {
            return;
        };
        let name = format!("{unit}{UNIT_SUFFIX}");
        self.offer(offset, &name, |app, name| {
            Offer::new(
                provider,
                NeedKind::App,
                name,
                Reference::Unit(UnitReference::new(app, unit.clone())),
                Scope::AppInstance,
            )
        });
    }

    /// Reads the ports and the routes.
    fn ports(&mut self, raw: &Raw) {
        for port in &raw.ports {
            self.port_offer(port, "", false);
        }
        for route in &raw.routes {
            self.port_offer(route, ROUTE_SUFFIX, true);
        }
    }

    fn port_offer(&mut self, port: &Spanned<String>, suffix: &str, route: bool) {
        let offset = Some(port.span().start);
        let Some(provider) = self.provider() else {
            return;
        };
        let name = format!("{}{suffix}", port.as_ref());
        let Ok(port_name) = port.as_ref().parse::<PortName>() else {
            self.refuse(offset, Reason::Name, "a port or route name is not a name");
            return;
        };
        self.offer(offset, &name, |app, name| {
            let reference = if route {
                Reference::Route(RouteReference::new(app, port_name))
            } else {
                Reference::Port(PortReference::new(app, port_name))
            };
            Offer::new(
                provider,
                NeedKind::Address,
                name,
                reference,
                Scope::AppInstance,
            )
        });
    }

    /// Builds the offer called `name` with `build` and keeps it, or refuses
    /// the name.
    fn offer(
        &mut self,
        offset: Option<usize>,
        name: &str,
        build: impl FnOnce(AppName, OfferName) -> Result<Offer, crate::RecordError>,
    ) {
        let offer = name
            .parse::<OfferName>()
            .and_then(|name| build(self.app.clone(), name));
        match offer {
            Ok(offer) => self.mxv.offers.push(offer),
            Err(error) => self.refuse(
                offset,
                Reason::Name,
                format!(
                    "an offer name is lowercase letters, digits, '-' and '_', so a port, route or unit name with another character is refused: {}",
                    error
                ),
            ),
        }
    }

    /// Reads the `[wants.<name>]` tables.
    fn wants(&mut self, wants: &BTreeMap<Spanned<String>, Spanned<toml::Value>>) {
        for (name, value) in wants {
            let line = Some(name.span().start);
            match want(name.as_ref(), value, self.text) {
                Ok(want) => self.mxv.wants.push(want),
                Err(message) => self.refuse(line, Reason::Want, message),
            }
        }
    }
}

/// The keys a want holds.
const WANT_KEYS: [&str; 4] = ["repo", "provides", "as", "optional"];

/// Reads one want, or says why it is refused. The message never quotes a
/// value of the file.
fn want(name: &str, value: &Spanned<toml::Value>, text: &str) -> Result<WantDeclaration, String> {
    let toml::Value::Table(table) = value.as_ref() else {
        return Err("a want is a table with the keys repo, provides, as and optional".to_owned());
    };
    if table.keys().any(|key| !WANT_KEYS.contains(&key.as_str())) {
        return Err("a want holds a key other than repo, provides, as and optional".to_owned());
    }
    let string = |key: &str| -> Result<&str, String> {
        table
            .get(key)
            .ok_or_else(|| format!("a want needs the key {key}"))?
            .as_str()
            .ok_or_else(|| format!("the key {key} of a want is a string"))
    };
    let optional = match table.get("optional") {
        None => false,
        Some(toml::Value::Boolean(optional)) => *optional,
        Some(_) => return Err("the key optional of a want is true or false".to_owned()),
    };
    let span: Range<usize> = value.span();
    Ok(WantDeclaration {
        name: name.parse().map_err(|error: crate::RecordError| {
            format!("the name of a want is refused: {}", error)
        })?,
        repo: string("repo")?
            .parse()
            .map_err(|error: crate::RecordError| {
                format!("the key repo of a want is refused: {}", error)
            })?,
        provides: string("provides")?
            .parse()
            .map_err(|error: crate::RecordError| {
                format!("the key provides of a want is refused: {}", error)
            })?,
        delivers_as: string("as")?.parse().map_err(|error: crate::RecordError| {
            format!("the key as of a want is refused: {}", error)
        })?,
        optional,
        line: line_at(text, span.start),
    })
}
