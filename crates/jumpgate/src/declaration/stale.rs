//! The records that a later run of the reader leaves stale.
//!
//! A run of the reader replaces the records of each app it read. A record
//! of such an app that the run did not produce comes from a declaration
//! that has since changed, such as a removed want or a port that a new unit
//! of the same name renamed from `api` to `api-port`, and it would mislead
//! the matcher if it stayed.

use std::collections::BTreeSet;

use crate::declaration::Declarations;
use crate::keyword::ProviderKind;
use crate::name::AppName;
use crate::path::RecordPath;
use crate::record::Record;
use crate::store::{Store, StoreError};

#[cfg(test)]
mod tests;

/// Returns the paths of the needs, units and app-provided offers in `store`
/// that belong to an app of `declarations.read_apps()` and that
/// `declarations` does not produce.
///
/// Apps the run did not read, and apps whose declaration the run refused,
/// keep every record. A secret-store offer is never stale, because several
/// apps and other commands may rely on it.
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
    for need in store.needs()? {
        if apps.contains(need.app()) {
            found.push(need.record_path());
        }
    }
    for offer in store.offers()? {
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
