//! The records that a later run of the reader leaves stale.
//!
//! A run of the reader replaces the records of each app it read. A record
//! of such an app that the run did not produce comes from a declaration
//! that has since changed, such as a removed want or a port that a new unit
//! of the same name renamed from `api` to `api-port`, and it would mislead
//! the matcher if it stayed. A workspace offer of an mxv service belongs to
//! no one app, so it is stale when the run does not produce it and no need
//! that stays in the store requests it.

use std::collections::{BTreeMap, BTreeSet};

use crate::declaration::Declarations;
use crate::keyword::ProviderKind;
use crate::name::AppName;
use crate::path::{OfferPath, RecordPath};
use crate::record::{MXV, Need, Record, Request};
use crate::store::{Store, StoreError};

#[cfg(test)]
mod tests;

/// Returns the paths of the needs, units, app-provided offers and mxv
/// workspace offers in `store` that belong to an app of
/// `declarations.read_apps()` and that `declarations` does not produce.
///
/// Apps the run did not read, and apps whose declaration the run refused,
/// keep every record, and so does a workspace offer that one of their needs
/// requests. A secret-store offer is never stale, because several apps and
/// other commands may rely on it.
///
/// # Errors
///
/// Returns a [`StoreError`] when a record of the store cannot be read.
pub fn stale(
    declarations: &Declarations,
    store: &(impl Store + ?Sized),
) -> Result<Vec<RecordPath>, StoreError> {
    let apps: BTreeSet<&AppName> = declarations.read_apps().iter().collect();
    let produced: BTreeSet<RecordPath> = declarations
        .needs()
        .iter()
        .map(Record::record_path)
        .chain(declarations.offers().iter().map(Record::record_path))
        .chain(declarations.units().iter().map(Record::record_path))
        .collect();
    let mut found: Vec<RecordPath> = Vec::new();
    let mut requested: BTreeSet<OfferPath> = BTreeSet::new();
    for need in store.needs()? {
        if apps.contains(need.app()) {
            found.push(need.record_path());
        } else if let Some(Request::Bind(path)) = need.requests() {
            requested.insert(path.clone());
        }
    }
    for offer in store.offers()? {
        let by_workspace = offer.provider().kind() == ProviderKind::Workspace
            && offer.provider().name().as_str() == MXV
            && !requested.contains(&offer.path());
        if by_workspace {
            found.push(offer.record_path());
        }
        let by_app = offer.provider().kind() == ProviderKind::App
            && apps
                .iter()
                .any(|app| app.as_str() == offer.provider().name().as_str());
        if by_app {
            found.push(offer.record_path());
        }
    }
    for unit in store.units()? {
        if apps.contains(unit.app()) {
            found.push(unit.record_path());
        }
    }
    found.retain(|path| !produced.contains(path));
    found.sort();
    Ok(found)
}

/// Returns the path of each need and offer that a binding of the store
/// names, with the path of the first binding that names it.
///
/// # Errors
///
/// Returns a [`StoreError`] when a record of the store cannot be read.
pub fn named_by_bindings(
    store: &(impl Store + ?Sized),
) -> Result<BTreeMap<String, String>, StoreError> {
    let mut named = BTreeMap::new();
    for target in store.targets()? {
        for stored in store.bindings(target.name())? {
            let binding = stored.binding().record_path().to_string();
            named
                .entry(stored.binding().need().to_string())
                .or_insert_with(|| binding.clone());
            named
                .entry(stored.binding().offer().to_string())
                .or_insert(binding);
        }
    }
    Ok(named)
}

/// Returns the needs of `store` that the run neither replaces nor deletes.
///
/// A need stays when its app was not read, or when [`stale`] would delete it
/// but a binding names it, which is what `needs` keeps. Each stays bound to
/// the offer it requests.
pub(crate) fn surviving_needs(
    declarations: &Declarations,
    store: &(impl Store + ?Sized),
) -> Result<Vec<Need>, StoreError> {
    let apps: BTreeSet<&AppName> = declarations.read_apps().iter().collect();
    let leaving: BTreeSet<RecordPath> = stale(declarations, store)?.into_iter().collect();
    let named = named_by_bindings(store)?;
    Ok(store
        .needs()?
        .into_iter()
        .filter(|need| {
            let path = need.record_path();
            !apps.contains(need.app())
                || (leaving.contains(&path) && named.contains_key(&path.to_string()))
        })
        .collect())
}
