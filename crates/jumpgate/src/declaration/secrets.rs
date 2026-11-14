//! The needs and offers that the secrets of `starmap.toml` give.

use std::collections::{BTreeMap, BTreeSet};

use starmap::{Mode, SecretRef, Source, Spec, Unset};

use crate::declaration::mxv::{Mxv, ParsedUnit, line_at};
use crate::declaration::{Gathered, Reason, Refusal, Repository, Unread};
use crate::distinct::Distinct;
use crate::file_path::RepoFilePath;
use crate::keyword::{NeedKind, ProviderKind, Scope};
use crate::name::{NeedName, OfferName, ProviderName, UnitName};
use crate::path::OfferPath;
use crate::record::{
    DeclaredIn, DeliversAs, Need, Offer, Provider, Reference, Request, WhenUnbound,
};
use crate::secret::SecretReference;
use crate::variable::VariableName;

/// The name of the file that declares the variables an app reads.
pub const STARMAP_FILE: &str = "starmap.toml";

/// The secret store that `op://` references name.
const SECRET_STORE: &str = "one-password";

/// The most bytes of an offer name that come from the reference. The rest
/// of the 64 bytes that a name holds is a dash and a digest.
const SLUG_BYTES: usize = 55;

/// A secret's need, and the reference its starmap entry binds, if any.
pub(crate) struct Found {
    need: Need,
    file: RepoFilePath,
    bind: Option<SecretReference>,
}

/// Reads each `starmap.toml` that a process of `mxv` reads, and returns the
/// secrets it declares. The mode is production: a secret that is optional
/// there is omitted when unbound, and any other fails. starmap refuses a
/// default on a withheld variable, which every secret is, so a secret never
/// has a fallback to keep.
pub(crate) fn read(repository: &Repository<'_>, mxv: &Mxv, gathered: &mut Gathered) -> Vec<Found> {
    let Some(app) = &mxv.app else {
        return Vec::new();
    };
    let mut readers: BTreeMap<RepoFilePath, Vec<UnitName>> = BTreeMap::new();
    for ParsedUnit { unit, starmap } in &mxv.units {
        if let Some(file) = starmap {
            readers
                .entry(file.clone())
                .or_default()
                .push(unit.name().clone());
        }
    }
    let root = RepoFilePath::known(STARMAP_FILE);
    if !readers.contains_key(&root) {
        match repository.text(&root) {
            Ok(Some(_)) => gathered.unread.push(Unread {
                repo: repository.id().clone(),
                file: root,
            }),
            Ok(None) => {}
            Err(refusal) => gathered.refusals.push(refusal),
        }
    }
    let mut found = Vec::new();
    for (file, consumers) in readers {
        let text = match repository.text(&file) {
            Ok(Some(text)) => text,
            Ok(None) => continue,
            Err(refusal) => {
                gathered.refusals.push(refusal);
                continue;
            }
        };
        let spec = match Spec::parse(&text) {
            Ok(spec) => spec,
            Err(error) => {
                for problem in error.problems() {
                    gathered.refusals.push(Refusal::new(
                        repository.id(),
                        &file,
                        Some(line_at(&text, problem.span().start)),
                        Reason::Starmap,
                        format!("{}: {problem}", problem.kind().code()),
                    ));
                }
                continue;
            }
        };
        let consumers = Distinct::<UnitName, 1>::new(consumers);
        for (name, variable) in spec.variables() {
            let Source::Secret { bind } = variable.source() else {
                continue;
            };
            let refuse = |message: String| {
                Refusal::new(repository.id(), &file, None, Reason::Starmap, message)
            };
            let Ok(consumers) = consumers.clone() else {
                continue;
            };
            let reference = match bind.as_ref().map(starmap::SecretBind::reference) {
                None => None,
                Some(SecretRef::OnePassword(reference)) => {
                    match reference.as_str().parse::<SecretReference>() {
                        Ok(reference) => Some(reference),
                        Err(error) => {
                            gathered.refusals.push(refuse(format!(
                                "the bind of a secret is not a reference jumpgate records: {error}"
                            )));
                            continue;
                        }
                    }
                }
                Some(_) => {
                    gathered.refusals.push(refuse(
                        "a secret has a bind that jumpgate does not know".to_owned(),
                    ));
                    continue;
                }
            };
            let optional = matches!(variable.unset().in_mode(Mode::Production), Unset::Optional);
            match secret_need(app, repository, &file, name.as_str(), consumers, optional) {
                Ok(need) => found.push(Found {
                    need,
                    file: file.clone(),
                    bind: reference,
                }),
                Err(message) => gathered.refusals.push(refuse(message)),
            }
        }
    }
    found
}

/// Builds the need of the secret `variable` of `app`.
fn secret_need(
    app: &crate::name::AppName,
    repository: &Repository<'_>,
    file: &RepoFilePath,
    variable: &str,
    consumers: Distinct<UnitName, 1>,
    optional: bool,
) -> Result<Need, String> {
    let name: NeedName = variable
        .to_ascii_lowercase()
        .replace('_', "-")
        .parse()
        .map_err(|error: crate::RecordError| {
            format!("the name of a secret is not a need name: {error}")
        })?;
    let delivered: VariableName = variable.parse().map_err(|error: crate::RecordError| {
        format!("the variable of a secret is refused: {error}")
    })?;
    let delivers_as = Distinct::new(vec![delivered]).map_err(|error| error.to_string())?;
    let when_unbound = if optional {
        WhenUnbound::Omit
    } else {
        WhenUnbound::Fail
    };
    Need::new(
        app.clone(),
        name,
        NeedKind::Secret,
        consumers,
        DeclaredIn::new(repository.id().clone(), file.clone()),
        DeliversAs::Variables(delivers_as),
    )
    .with_unbound(optional, when_unbound)
    .map_err(|error| error.to_string())
}

/// Gives each bound secret its offer and its request, and keeps the needs.
pub(crate) fn finish(found: Vec<Found>, gathered: &mut Gathered) {
    let references: BTreeSet<&SecretReference> = found
        .iter()
        .filter_map(|secret| secret.bind.as_ref())
        .collect();
    let names = offer_names(&references);
    for secret in &found {
        let mut need = secret.need.clone();
        if let Some(reference) = &secret.bind {
            let Some(name) = names.get(reference) else {
                continue;
            };
            let Ok(provider_name) = SECRET_STORE.parse::<ProviderName>() else {
                continue;
            };
            let provider = Provider::new(ProviderKind::SecretStore, provider_name.clone());
            let offer = Offer::new(
                provider,
                NeedKind::Secret,
                name.clone(),
                Reference::Secret(reference.clone()),
                Scope::Workspace,
            );
            match offer {
                Ok(offer) => {
                    gathered.add_offer(offer, need.declared_in().repo(), &secret.file);
                    need = need.with_requests(Request::Bind(OfferPath::new(
                        ProviderKind::SecretStore,
                        provider_name,
                        name.clone(),
                    )));
                }
                Err(error) => gathered.refusals.push(Refusal::new(
                    need.declared_in().repo(),
                    &secret.file,
                    None,
                    Reason::Starmap,
                    format!("the offer of a bind is refused: {error}"),
                )),
            }
        }
        let file = secret.file.clone();
        gathered.add_need(need, &file, None);
    }
}

/// Names the offer of each reference: the reference with every character
/// that a name does not hold turned into `-`, and a digest of the reference
/// added. The name comes from the reference alone, so it is the same
/// whatever else a run reads.
fn offer_names(references: &BTreeSet<&SecretReference>) -> BTreeMap<SecretReference, OfferName> {
    let mut names = BTreeMap::new();
    for reference in references {
        let slug = slug(reference.as_str());
        let end = slug.len().min(SLUG_BYTES);
        let kept = slug.get(..end).unwrap_or_default().trim_end_matches('-');
        let name = format!("{kept}-{:08x}", digest(reference.as_str()));
        if let Ok(name) = name.parse() {
            names.insert((*reference).clone(), name);
        }
    }
    names
}

/// Returns `text` in lowercase with each run of characters other than ASCII
/// letters and digits turned into one `-`, and no `-` at either end.
fn slug(text: &str) -> String {
    let rest = text.strip_prefix("op://").unwrap_or(text);
    let mut slug = String::new();
    for character in rest.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "secret".to_owned()
    } else {
        slug.to_owned()
    }
}

/// Returns the 32-bit FNV-1a digest of `text`, which gives the same name to
/// the same reference on every run.
fn digest(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}
