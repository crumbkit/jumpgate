//! The need records that the `[needs.<name>]` tables of `mxv.toml` give.

use crate::declaration::mxv::{MXV_FILE, Mxv, NeedDeclaration, ParsedUnit};
use crate::declaration::{Gathered, ProvidesByRepo, Reason, Refusal};
use crate::distinct::Distinct;
use crate::file_path::RepoFilePath;
use crate::keyword::NeedKind;
use crate::name::UnitName;
use crate::record::{DeclaredIn, DeliversAs, Need, Request, Source, WhenUnbound};

/// Writes the need record of each `[needs.<name>]` table of `mxv` into
/// `gathered`.
pub(crate) fn from_needs(mxv: &Mxv, by_repo: &ProvidesByRepo, gathered: &mut Gathered) {
    let Some(app) = &mxv.app else {
        return;
    };
    let file = RepoFilePath::known(MXV_FILE);
    let consumers: Vec<UnitName> = mxv.units.iter().map(ParsedUnit::name).collect();
    for need in &mxv.needs {
        let refuse = |reason, message: &str| {
            Refusal::new(&mxv.repo, &file, Some(need.line), reason, message)
        };
        if by_repo
            .get(&need.repo)
            .is_some_and(|repo_provides| repo_provides.shared.contains(&need.from))
        {
            gathered.refusals.push(refuse(
                Reason::Provider,
                "the name is shared by two kinds of the app, so the need names the kind, such as api-route",
            ));
            continue;
        }
        let Some(kind) = kind_of(need, by_repo) else {
            gathered.refusals.push(refuse(
                Reason::Provider,
                "the repository of the need was read and does not provide the name",
            ));
            continue;
        };
        let Ok(request) = Source::new(need.repo.clone(), need.from.clone()) else {
            gathered.refusals.push(refuse(
                Reason::Provider,
                "the last segment of the repository of the need is not an app name",
            ));
            continue;
        };
        let Ok(consumers) = Distinct::<UnitName, 1>::new(consumers.clone()) else {
            gathered.refusals.push(refuse(
                Reason::Consumers,
                "the app has no process or container to receive the need",
            ));
            continue;
        };
        let Ok(delivers_as) = Distinct::new(vec![need.delivers_as.clone()]) else {
            continue;
        };
        let (optional, when_unbound) = if need.optional {
            (true, WhenUnbound::Omit)
        } else {
            (false, WhenUnbound::Fail)
        };
        let record = Need::new(
            app.clone(),
            need.name.clone(),
            kind,
            consumers,
            DeclaredIn::new(mxv.repo.clone(), file.clone()),
            DeliversAs::Variables(delivers_as),
        )
        .with_unbound(optional, when_unbound)
        .map(|need| need.with_requests(Request::Need(request)));
        match record {
            Ok(record) => gathered.add_need(record, &file, Some(need.line)),
            Err(error) => gathered.refusals.push(refuse(
                Reason::Need,
                &format!("the need is refused: {error}"),
            )),
        }
    }
}

/// Returns the kind of the need of `need`: the kind of the provide it asks
/// for when the repository was read, and address when it was not. It
/// returns `None` for a repository that was read and lacks the provide.
fn kind_of(need: &NeedDeclaration, by_repo: &ProvidesByRepo) -> Option<NeedKind> {
    match by_repo.get(&need.repo) {
        None => Some(NeedKind::Address),
        Some(repo_provides) => repo_provides.kinds.get(&need.from).copied(),
    }
}
