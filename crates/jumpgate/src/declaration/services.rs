//! The needs and provides that the `[service.<name>]` tables of `mxv.toml`
//! give.
//!
//! mxv runs each kind of service in one backend that every app of a
//! workspace shares, so a service name is a name within the workspace. A
//! service that a process lists in `needs` gives one need of its app and one
//! provide of the workspace provider `mxv`. A service that no process lists
//! gives neither.

use std::collections::{BTreeMap, BTreeSet};

use crate::consumer::Consumer;
use crate::declaration::mxv::{MXV_FILE, Mxv, ServiceDeclaration};
use crate::declaration::stale::surviving_needs;
use crate::declaration::{Declarations, Gathered, Reason, Refusal, UnusedService};
use crate::distinct::Distinct;
use crate::file_path::RepoFilePath;
use crate::keyword::{NeedKind, ProviderKind, Scope, ServiceKind};
use crate::name::{AppName, NeedName, ProvideName, ProviderName, ServiceName};
use crate::path::ProvidePath;
use crate::record::{
    DeclaredIn, DeliversAs, MXV, Need, Provide, Provider, Reference, Request, ServiceReference,
    WhenUnbound,
};
use crate::repo::RepoId;
use crate::store::{Store, StoreError};
use crate::variable::VariableName;

/// Returns the service names that two apps declare with different kinds.
/// mxv refuses such a pair, so jumpgate refuses each app that declares one.
pub(crate) fn clashes(parsed: &[Mxv]) -> BTreeSet<ServiceName> {
    let mut kinds: BTreeMap<&ServiceName, BTreeSet<ServiceKind>> = BTreeMap::new();
    for mxv in parsed {
        for service in &mxv.services {
            kinds.entry(&service.name).or_default().insert(service.kind);
        }
    }
    kinds
        .into_iter()
        .filter(|(_, kinds)| kinds.len() > 1)
        .map(|(name, _)| name.clone())
        .collect()
}

/// Writes the needs and provides of the services of `mxv` into `gathered`,
/// and refuses a process that lists a service the app does not declare.
pub(crate) fn read(mxv: &Mxv, clashing: &BTreeSet<ServiceName>, gathered: &mut Gathered) {
    let Some(app) = &mxv.app else {
        return;
    };
    let file = RepoFilePath::known(MXV_FILE);
    let declared: BTreeSet<&str> = mxv
        .services
        .iter()
        .map(|service| service.name.as_str())
        .collect();
    for parsed in &mxv.units {
        for used in &parsed.needs {
            if !declared.contains(used.name.as_str())
                && !mxv.refused_services.contains(used.name.as_str())
            {
                gathered.refusals.push(Refusal::new(
                    &mxv.repo,
                    &file,
                    Some(used.line),
                    Reason::Service,
                    format!(
                        "the process {} lists the service {} in needs, and the app declares no such service",
                        parsed.unit.name(),
                        used.name
                    ),
                ));
            }
        }
    }
    for service in &mxv.services {
        if clashing.contains(&service.name) {
            gathered.refusals.push(Refusal::new(
                &mxv.repo,
                &file,
                None,
                Reason::Clash,
                format!(
                    "the service {} has another kind in another app, and a workspace's backend holds one service of each name",
                    service.name
                ),
            ));
            continue;
        }
        let consumers: Vec<Consumer> = mxv
            .units
            .iter()
            .filter(|parsed| {
                parsed
                    .needs
                    .iter()
                    .any(|used| used.name == service.name.as_str())
            })
            .map(|parsed| Consumer::Unit(parsed.name()))
            .collect();
        match Distinct::<Consumer, 1>::new(consumers) {
            Ok(consumers) => used(mxv, app, &file, service, consumers, gathered),
            Err(_) => gathered.unused.push(UnusedService {
                repo: mxv.repo.clone(),
                service: service.name.clone(),
            }),
        }
    }
}

/// Writes the need of `service`, which `consumers` list, and its provide.
fn used(
    mxv: &Mxv,
    app: &crate::name::AppName,
    file: &RepoFilePath,
    service: &ServiceDeclaration,
    consumers: Distinct<Consumer, 1>,
    gathered: &mut Gathered,
) {
    let refuse = |message: String| Refusal::new(&mxv.repo, file, None, Reason::Name, message);
    let names: Result<(NeedName, ProvideName, ProviderName), crate::RecordError> = (|| {
        Ok((
            service.name.as_str().parse()?,
            service.name.as_str().parse()?,
            MXV.parse()?,
        ))
    })();
    let (need_name, provide_name, provider_name) = match names {
        Ok(names) => names,
        Err(error) => {
            gathered
                .refusals
                .push(refuse(format!("a service name is refused: {error}")));
            return;
        }
    };
    let mut variables: Vec<VariableName> = Vec::new();
    if let Ok(endpoint) = service.kind.endpoint_variable().parse::<VariableName>() {
        variables.push(endpoint);
    }
    for variable in &service.env {
        if !variables.contains(variable) {
            variables.push(variable.clone());
        }
    }
    let Ok(delivers_as) = Distinct::new(variables) else {
        return;
    };
    let provide = Provide::new(
        Provider::new(ProviderKind::Workspace, provider_name.clone()),
        NeedKind::DataAccess,
        provide_name.clone(),
        Reference::Service(ServiceReference::new(service.kind, service.name.clone())),
        Scope::Workspace,
    );
    let need = Need::new(
        app.clone(),
        need_name,
        NeedKind::DataAccess,
        consumers,
        DeclaredIn::new(mxv.repo.clone(), file.clone()),
        DeliversAs::Variables(delivers_as),
    )
    .with_unbound(false, WhenUnbound::Fail)
    .map(|need| {
        need.with_requests(Request::Bind(ProvidePath::new(
            ProviderKind::Workspace,
            provider_name,
            provide_name,
        )))
    });
    match (provide, need) {
        (Ok(provide), Ok(need)) => {
            gathered.add_provide(provide, &mxv.repo, file);
            gathered.add_need(need, file, None);
        }
        (Err(error), _) | (_, Err(error)) => gathered.refusals.push(Refusal::new(
            &mxv.repo,
            file,
            None,
            Reason::Name,
            format!("the need or provide of a service is refused: {error}"),
        )),
    }
}

/// Refuses each app of `declarations` whose service provide differs from the
/// provide the store holds at its path while a stored need that survives the
/// run still requests the stored provide, and drops the records of that app.
///
/// A refused app leaves `read_apps`, so its stored needs survive and protect
/// their provides from the apps that remain. The comparison repeats until it refuses no
/// further app.
pub(crate) fn refuse_against_store(
    declarations: &mut Declarations,
    store: &(impl Store + ?Sized),
) -> Result<(), StoreError> {
    while refuse_once(declarations, store)? {}
    Ok(())
}

/// Compares the provides of the apps still in the run with the store once and
/// returns whether it refused an app.
fn refuse_once(
    declarations: &mut Declarations,
    store: &(impl Store + ?Sized),
) -> Result<bool, StoreError> {
    let surviving = surviving_needs(declarations, store)?;
    let stored_provides = store.provides()?;
    let mut refused: Vec<(AppName, RepoId, RepoFilePath, AppName, ProvidePath)> = Vec::new();
    for provide in &declarations.provides {
        if !matches!(provide.reference(), Reference::Service(_)) {
            continue;
        }
        let path = provide.path();
        let differs = stored_provides
            .iter()
            .any(|stored| stored.path() == path && stored != provide);
        if !differs {
            continue;
        }
        let Some(other) = surviving
            .iter()
            .find(|need| matches!(need.requests(), Some(Request::Bind(wanted)) if *wanted == path))
        else {
            continue;
        };
        for need in &declarations.needs {
            if matches!(need.requests(), Some(Request::Bind(wanted)) if *wanted == path) {
                refused.push((
                    need.app().clone(),
                    need.declared_in().repo().clone(),
                    need.declared_in().file().clone(),
                    other.app().clone(),
                    path.clone(),
                ));
            }
        }
    }
    let changed = !refused.is_empty();
    let mut released: BTreeSet<ProvidePath> = BTreeSet::new();
    for (app, repo, file, other, path) in refused {
        released.extend(
            declarations
                .needs
                .iter()
                .filter(|need| *need.app() == app)
                .filter_map(|need| match need.requests() {
                    Some(Request::Bind(wanted)) => Some(wanted.clone()),
                    _ => None,
                }),
        );
        declarations.read_apps.retain(|read| *read != app);
        declarations.needs.retain(|need| *need.app() != app);
        declarations.units.retain(|unit| *unit.app() != app);
        declarations.provides.retain(|provide| {
            provide.path() != path
                && !(provide.provider().kind() == ProviderKind::App
                    && provide.provider().name().as_str() == app.as_str())
        });
        declarations.refusals.push(Refusal::new(
            &repo,
            &file,
            None,
            Reason::Clash,
            format!(
                "the service provide {path} differs from the stored one, which the app {other} still needs"
            ),
        ));
    }
    // A service provide that only a refused app's needs requested is not
    // written either, so the store keeps the provide its other apps bound.
    let wanted: BTreeSet<ProvidePath> = declarations
        .needs
        .iter()
        .filter_map(|need| match need.requests() {
            Some(Request::Bind(wanted)) => Some(wanted.clone()),
            _ => None,
        })
        .collect();
    declarations.provides.retain(|provide| {
        !(matches!(provide.reference(), Reference::Service(_))
            && released.contains(&provide.path())
            && !wanted.contains(&provide.path()))
    });
    Ok(changed)
}
