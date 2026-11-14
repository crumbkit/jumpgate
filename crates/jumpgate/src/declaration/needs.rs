//! The needs that the wants of `mxv.toml` give.

use crate::declaration::mxv::{MXV_FILE, Mxv, ParsedUnit, WantDeclaration};
use crate::declaration::{Gathered, Provided, Reason, Refusal};
use crate::distinct::Distinct;
use crate::file_path::RepoFilePath;
use crate::keyword::NeedKind;
use crate::name::UnitName;
use crate::record::{DeclaredIn, DeliversAs, Need, Request, Want, WhenUnbound};

/// Writes the need of each want of `mxv` into `gathered`.
pub(crate) fn from_wants(mxv: &Mxv, provided: &Provided, gathered: &mut Gathered) {
    let Some(app) = &mxv.app else {
        return;
    };
    let file = RepoFilePath::known(MXV_FILE);
    let consumers: Vec<UnitName> = mxv.units.iter().map(ParsedUnit::name).collect();
    for want in &mxv.wants {
        let refuse = |reason, message: &str| {
            Refusal::new(&mxv.repo, &file, Some(want.line), reason, message)
        };
        if provided
            .get(&want.repo)
            .is_some_and(|offered| offered.shared.contains(&want.provides))
        {
            gathered.refusals.push(refuse(
                Reason::Provider,
                "the name is shared by two kinds of the app, so the want names the kind, such as api-route",
            ));
            continue;
        }
        let Some(kind) = kind_of(want, provided) else {
            gathered.refusals.push(refuse(
                Reason::Provider,
                "the repository of the want was read and does not provide the name",
            ));
            continue;
        };
        let Ok(consumers) = Distinct::<UnitName, 1>::new(consumers.clone()) else {
            gathered.refusals.push(refuse(
                Reason::Consumers,
                "the app has no process or container to receive the want",
            ));
            continue;
        };
        let Ok(delivers_as) = Distinct::new(vec![want.delivers_as.clone()]) else {
            continue;
        };
        let (optional, when_unbound) = if want.optional {
            (true, WhenUnbound::Omit)
        } else {
            (false, WhenUnbound::Fail)
        };
        let need = Need::new(
            app.clone(),
            want.name.clone(),
            kind,
            consumers,
            DeclaredIn::new(mxv.repo.clone(), file.clone()),
            DeliversAs::Variables(delivers_as),
        )
        .with_unbound(optional, when_unbound)
        .map(|need| {
            need.with_requests(Request::Want(Want::new(
                want.repo.clone(),
                want.provides.clone(),
            )))
        });
        match need {
            Ok(need) => gathered.add_need(need, &file, Some(want.line)),
            Err(error) => gathered.refusals.push(refuse(
                Reason::Want,
                &format!("the want is refused: {error}"),
            )),
        }
    }
}

/// Returns the kind of the need of `want`: the kind of the offer it asks
/// for when the repository was read, and address when it was not. It
/// returns `None` for a repository that was read and lacks the offer.
fn kind_of(want: &WantDeclaration, provided: &Provided) -> Option<NeedKind> {
    match provided.get(&want.repo) {
        None => Some(NeedKind::Address),
        Some(offered) => offered.kinds.get(&want.provides).copied(),
    }
}
