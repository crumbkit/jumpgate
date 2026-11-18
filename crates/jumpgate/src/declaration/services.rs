//! The needs and offers that the `[service.<name>]` tables of `mxv.toml`
//! give.
//!
//! mxv runs each kind of service in one backend that every app of a
//! workspace shares, so a service name is a name within the workspace. A
//! service that a process lists in `needs` gives one need of its app and one
//! offer of the workspace provider `mxv`. A service that no process lists
//! gives neither.

use std::collections::{BTreeMap, BTreeSet};

use crate::declaration::mxv::{MXV_FILE, Mxv, ServiceDeclaration};
use crate::declaration::{Gathered, Reason, Refusal, UnusedService};
use crate::distinct::Distinct;
use crate::file_path::RepoFilePath;
use crate::keyword::{NeedKind, ProviderKind, Scope, ServiceKind};
use crate::name::{NeedName, OfferName, ProviderName, ServiceName, UnitName};
use crate::path::OfferPath;
use crate::record::{
    DeclaredIn, DeliversAs, MXV, Need, Offer, Provider, Reference, Request, ServiceReference,
    WhenUnbound,
};
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

/// Writes the needs and offers of the services of `mxv` into `gathered`,
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
        let consumers: Vec<UnitName> = mxv
            .units
            .iter()
            .filter(|parsed| {
                parsed
                    .needs
                    .iter()
                    .any(|used| used.name == service.name.as_str())
            })
            .map(|parsed| parsed.name())
            .collect();
        match Distinct::<UnitName, 1>::new(consumers) {
            Ok(consumers) => used(mxv, app, &file, service, consumers, gathered),
            Err(_) => gathered.unused.push(UnusedService {
                repo: mxv.repo.clone(),
                service: service.name.clone(),
            }),
        }
    }
}

/// Writes the need of `service`, which `consumers` list, and its offer.
fn used(
    mxv: &Mxv,
    app: &crate::name::AppName,
    file: &RepoFilePath,
    service: &ServiceDeclaration,
    consumers: Distinct<UnitName, 1>,
    gathered: &mut Gathered,
) {
    let refuse = |message: String| Refusal::new(&mxv.repo, file, None, Reason::Name, message);
    let names: Result<(NeedName, OfferName, ProviderName), crate::RecordError> = (|| {
        Ok((
            service.name.as_str().parse()?,
            service.name.as_str().parse()?,
            MXV.parse()?,
        ))
    })();
    let (need_name, offer_name, provider_name) = match names {
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
    let offer = Offer::new(
        Provider::new(ProviderKind::Workspace, provider_name.clone()),
        NeedKind::DataAccess,
        offer_name.clone(),
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
        need.with_requests(Request::Bind(OfferPath::new(
            ProviderKind::Workspace,
            provider_name,
            offer_name,
        )))
    });
    match (offer, need) {
        (Ok(offer), Ok(need)) => {
            gathered.add_offer(offer, &mxv.repo, file);
            gathered.add_need(need, file, None);
        }
        (Err(error), _) | (_, Err(error)) => gathered.refusals.push(Refusal::new(
            &mxv.repo,
            file,
            None,
            Reason::Name,
            format!("the need or offer of a service is refused: {error}"),
        )),
    }
}
