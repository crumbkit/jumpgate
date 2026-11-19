//! The failure codes of the `jumpgate` command. Each code has the form
//! `jumpgate::<area>::<name>`, where the area is a noun from jumpgate's
//! domain (CLI-3). The library's refusals reach the command with the same
//! area and name they have in the library.

use crumbkit::Code;

/// The store holds no target record of the name `--target` gives.
pub const TARGET_MISSING: Code<'static> = Code::from_spelling("jumpgate::target::missing");

/// A need that must be bound has no binding, need, bind or fallback that
/// meets it.
pub const NEED_UNBOUND: Code<'static> = Code::from_spelling("jumpgate::need::unbound");

/// A binding record names a provide that the store does not hold.
pub const NEED_PROVIDE_MISSING: Code<'static> =
    Code::from_spelling("jumpgate::need::provide_missing");

/// Two or more provides meet one need, and the matcher binds none of them.
pub const NEED_AMBIGUOUS: Code<'static> = Code::from_spelling("jumpgate::need::ambiguous");

/// Only a fallback value would meet a need, and the target is a cloud
/// target, which takes no fallback.
pub const NEED_FALLBACK_ON_CLOUD: Code<'static> =
    Code::from_spelling("jumpgate::need::fallback_on_cloud");

/// A record that no declaration gives any more is still named by a binding
/// of the store, so `needs` kept it.
pub const NEED_KEPT: Code<'static> = Code::from_spelling("jumpgate::need::kept");

/// A need asks for a service that mxv runs on a local machine, and the
/// target is a cloud target, where no such service runs.
pub const NEED_LOCAL_ONLY: Code<'static> = Code::from_spelling("jumpgate::need::local_only");

/// A need asks for a service of one kind, and the provide that a binding or
/// the need's request names is a service of another kind.
pub const NEED_SERVICE_KIND: Code<'static> = Code::from_spelling("jumpgate::need::service_kind");

/// A `--repo` path is not a directory.
pub const REPO_PATH: Code<'static> = Code::from_spelling("jumpgate::repo::path");

/// git did not give the URL of the checkout's `origin`.
pub const REPO_GIT: Code<'static> = Code::from_spelling("jumpgate::repo::git");

/// The checkout's `origin` is not a remote of the form `host/owner/name`.
pub const REPO_REMOTE: Code<'static> = Code::from_spelling("jumpgate::repo::remote");

/// Two `--repo` values name one repository.
pub const REPO_DUPLICATE: Code<'static> = Code::from_spelling("jumpgate::repo::duplicate");

/// The repository has no `mxv.toml`.
pub const DECLARATION_MISSING: Code<'static> =
    Code::from_spelling("jumpgate::declaration::missing");
/// A declaration file exists and cannot be read.
pub const DECLARATION_UNREADABLE: Code<'static> =
    Code::from_spelling("jumpgate::declaration::unreadable");
/// A declaration file is longer than the reader accepts.
pub const DECLARATION_TOO_LARGE: Code<'static> =
    Code::from_spelling("jumpgate::declaration::too_large");
/// A declaration file is not valid TOML.
pub const DECLARATION_SYNTAX: Code<'static> = Code::from_spelling("jumpgate::declaration::syntax");
/// A need is not a table, has an unknown or missing key, or a bad value.
pub const DECLARATION_NEED: Code<'static> = Code::from_spelling("jumpgate::declaration::need");
/// A provide is not a table, has an unknown or missing key, names a unit the app lacks, or has a bad value.
pub const DECLARATION_PROVIDE: Code<'static> =
    Code::from_spelling("jumpgate::declaration::provide");
/// The absolute path of a socket provide is longer than a Unix socket path can be.
pub const DECLARATION_SOCKET_PATH: Code<'static> =
    Code::from_spelling("jumpgate::declaration::socket_path");
/// The file declares `[wants]`, which is now spelled `[needs.<name>]`.
pub const DECLARATION_USE_NEEDS: Code<'static> =
    Code::from_spelling("jumpgate::declaration::use_needs");
/// The file declares `[offers]`, which is now spelled `[provides.<name>]`.
pub const DECLARATION_USE_PROVIDES: Code<'static> =
    Code::from_spelling("jumpgate::declaration::use_provides");
/// A name breaks the rule of the record that holds it.
pub const DECLARATION_NAME: Code<'static> = Code::from_spelling("jumpgate::declaration::name");
/// A `starmap.toml` does not parse as a starmap file.
pub const DECLARATION_STARMAP: Code<'static> =
    Code::from_spelling("jumpgate::declaration::starmap");
/// Two declarations of one app would write the same record.
pub const DECLARATION_CLASH: Code<'static> = Code::from_spelling("jumpgate::declaration::clash");
/// A need names a repository that was read and does not provide the name.
pub const DECLARATION_PROVIDER: Code<'static> =
    Code::from_spelling("jumpgate::declaration::provider");
/// A need or a secret has no unit, and a need no task either, to receive it.
pub const DECLARATION_CONSUMERS: Code<'static> =
    Code::from_spelling("jumpgate::declaration::consumers");
/// A process's `cwd` is not a path within the checkout.
pub const DECLARATION_DIRECTORY: Code<'static> =
    Code::from_spelling("jumpgate::declaration::directory");
/// A process lists a service in `needs` that its app does not declare.
pub const DECLARATION_SERVICE: Code<'static> =
    Code::from_spelling("jumpgate::declaration::service");

/// The file system refused to read or write a store file.
pub const STORE_IO: Code<'static> = Code::from_spelling("jumpgate::store::io");
/// A store file is not a valid record.
pub const STORE_CORRUPT: Code<'static> = Code::from_spelling("jumpgate::store::corrupt");
/// A record's file is not at the path of the record it holds.
pub const STORE_MISPLACED: Code<'static> = Code::from_spelling("jumpgate::store::misplaced");
/// The store's board file or a board directory is not a board id.
pub const STORE_BOARD_FILE: Code<'static> = Code::from_spelling("jumpgate::store::board_file");
/// The `board` program did not start, did not answer, printed something
/// unexpected or refused a read or a write.
pub const STORE_BOARD: Code<'static> = Code::from_spelling("jumpgate::store::board");

/// The plan's text is not JSON.
pub const RECORD_SYNTAX: Code<'static> = Code::from_spelling("jumpgate::record::syntax");

/// The plan's text is longer than the reader accepts.
pub const RECORD_TOO_LARGE: Code<'static> = Code::from_spelling("jumpgate::record::too_large");

/// A value of the plan has the wrong JSON type.
pub const RECORD_SHAPE: Code<'static> = Code::from_spelling("jumpgate::record::shape");

/// The plan's `$type` names another type.
pub const RECORD_WRONG_TYPE: Code<'static> = Code::from_spelling("jumpgate::record::wrong_type");

/// An object of the plan holds a field its type does not have.
pub const RECORD_UNKNOWN_FIELD: Code<'static> =
    Code::from_spelling("jumpgate::record::unknown_field");

/// An object of the plan lacks a field its type needs.
pub const RECORD_MISSING_FIELD: Code<'static> =
    Code::from_spelling("jumpgate::record::missing_field");

/// A name in the plan breaks its rule.
pub const RECORD_NAME: Code<'static> = Code::from_spelling("jumpgate::record::name");

/// An environment variable's name in the plan breaks its rule.
pub const RECORD_VARIABLE: Code<'static> = Code::from_spelling("jumpgate::record::variable");

/// A repository id in the plan is not `host/owner/name`.
pub const RECORD_REPOSITORY: Code<'static> = Code::from_spelling("jumpgate::record::repository");

/// A file path in the plan breaks its rule.
pub const RECORD_FILE_PATH: Code<'static> = Code::from_spelling("jumpgate::record::file_path");

/// A record path in the plan does not have the shape of its record type.
pub const RECORD_RECORD_PATH: Code<'static> = Code::from_spelling("jumpgate::record::record_path");

/// A secret-store reference in the plan is not `op://` followed by a vault, an item and a field.
pub const RECORD_SECRET_REFERENCE: Code<'static> =
    Code::from_spelling("jumpgate::record::secret_reference");

/// A word in the plan is not one its field takes.
pub const RECORD_KEYWORD: Code<'static> = Code::from_spelling("jumpgate::record::keyword");

/// A list in the plan repeats an entry.
pub const RECORD_DUPLICATE: Code<'static> = Code::from_spelling("jumpgate::record::duplicate");

/// A list in the plan that needs an entry is empty.
pub const RECORD_EMPTY: Code<'static> = Code::from_spelling("jumpgate::record::empty");

/// A reference does not fit its provide.
pub const RECORD_REFERENCE_MISMATCH: Code<'static> =
    Code::from_spelling("jumpgate::record::reference_mismatch");

/// A fallback value in the plan is too long or holds a NUL character.
pub const RECORD_FALLBACK: Code<'static> = Code::from_spelling("jumpgate::record::fallback");

/// A need's optional flag contradicts its `when_unbound`.
pub const RECORD_OPTIONAL_CONFLICT: Code<'static> =
    Code::from_spelling("jumpgate::record::optional_conflict");

/// Check 1: a binding carries a literal value although no fallback made it.
pub const CHECK_SECRET_VALUE: Code<'static> = Code::from_spelling("jumpgate::check::secret_value");

/// Check 2: a need has no binding, and what it says to do when unbound does not let the plan run.
pub const CHECK_NEED_UNBOUND: Code<'static> = Code::from_spelling("jumpgate::check::need_unbound");

/// Check 3: a binding's provide, its way of being matched and what it supplies disagree.
pub const CHECK_PROVIDE_MISMATCH: Code<'static> =
    Code::from_spelling("jumpgate::check::provide_mismatch");

/// Check 3: a binding names a provide whose provider the plan does not know.
pub const CHECK_UNKNOWN_PROVIDER: Code<'static> =
    Code::from_spelling("jumpgate::check::unknown_provider");

/// Check 4: a secret reaches a unit of an app that does not declare it.
pub const CHECK_SECRET_REACH: Code<'static> = Code::from_spelling("jumpgate::check::secret_reach");

/// Check 5: a cloud plan has a need that a fallback meets.
pub const CHECK_CLOUD_FALLBACK: Code<'static> =
    Code::from_spelling("jumpgate::check::cloud_fallback");

/// Check 6: a unit starts after a unit that is not in the plan.
pub const CHECK_UNIT_MISSING: Code<'static> = Code::from_spelling("jumpgate::check::unit_missing");

/// Check 6: the plan lists one unit more than once.
pub const CHECK_UNIT_REPEATED: Code<'static> =
    Code::from_spelling("jumpgate::check::unit_repeated");

/// Check 6: a unit is in a cycle of the start order.
pub const CHECK_START_CYCLE: Code<'static> = Code::from_spelling("jumpgate::check::start_cycle");

/// Check 5, against a store: the store holds no record of the plan's target.
pub const CHECK_TARGET_UNKNOWN: Code<'static> =
    Code::from_spelling("jumpgate::check::target_unknown");

/// Check 5, against a store: the plan's target kind is not the kind of the target record.
pub const CHECK_TARGET_KIND: Code<'static> = Code::from_spelling("jumpgate::check::target_kind");

/// Check 2, against a store: the plan binds or lists a need that the store does not hold.
pub const CHECK_NEED_UNKNOWN: Code<'static> = Code::from_spelling("jumpgate::check::need_unknown");

/// Check 2, against a store: a need that applies to the target is neither bound nor listed as unbound.
pub const CHECK_NEED_OMITTED: Code<'static> = Code::from_spelling("jumpgate::check::need_omitted");

/// Check 2, against a store: the plan's answer for an unbound need is not the one the need record gives.
pub const CHECK_UNBOUND_DIFFERS: Code<'static> =
    Code::from_spelling("jumpgate::check::unbound_differs");

/// Check 2, against a store: an unbound secret need says it falls back.
pub const CHECK_SECRET_FALLBACK: Code<'static> =
    Code::from_spelling("jumpgate::check::secret_fallback");

/// Check 1, against a store: the plan's kind for a need is not the need record's kind.
pub const CHECK_NEED_KIND: Code<'static> = Code::from_spelling("jumpgate::check::need_kind");

/// Check 4, against a store: the plan's consumers of a need are not the need record's.
pub const CHECK_NEED_CONSUMERS: Code<'static> =
    Code::from_spelling("jumpgate::check::need_consumers");

/// Check 3, against a store: a binding names a provide that the store does not hold.
pub const CHECK_PROVIDE_UNKNOWN: Code<'static> =
    Code::from_spelling("jumpgate::check::provide_unknown");

/// Check 3, against a store: a binding supplies a reference other than its provide record's.
pub const CHECK_PROVIDE_REFERENCE: Code<'static> =
    Code::from_spelling("jumpgate::check::provide_reference");

/// The codes that the library's refusals and store errors map onto.
const LIBRARY: &[Code<'static>] = &[
    DECLARATION_MISSING,
    DECLARATION_UNREADABLE,
    DECLARATION_TOO_LARGE,
    DECLARATION_SYNTAX,
    DECLARATION_NEED,
    DECLARATION_PROVIDE,
    DECLARATION_SOCKET_PATH,
    DECLARATION_USE_NEEDS,
    DECLARATION_USE_PROVIDES,
    DECLARATION_NAME,
    DECLARATION_STARMAP,
    DECLARATION_CLASH,
    DECLARATION_PROVIDER,
    DECLARATION_CONSUMERS,
    DECLARATION_DIRECTORY,
    DECLARATION_SERVICE,
    STORE_IO,
    STORE_CORRUPT,
    STORE_MISPLACED,
    STORE_BOARD_FILE,
    STORE_BOARD,
    RECORD_SYNTAX,
    RECORD_TOO_LARGE,
    RECORD_SHAPE,
    RECORD_WRONG_TYPE,
    RECORD_UNKNOWN_FIELD,
    RECORD_MISSING_FIELD,
    RECORD_NAME,
    RECORD_VARIABLE,
    RECORD_REPOSITORY,
    RECORD_FILE_PATH,
    RECORD_RECORD_PATH,
    RECORD_SECRET_REFERENCE,
    RECORD_KEYWORD,
    RECORD_DUPLICATE,
    RECORD_EMPTY,
    RECORD_REFERENCE_MISMATCH,
    RECORD_FALLBACK,
    RECORD_OPTIONAL_CONFLICT,
    CHECK_SECRET_VALUE,
    CHECK_NEED_UNBOUND,
    CHECK_PROVIDE_MISMATCH,
    CHECK_UNKNOWN_PROVIDER,
    CHECK_SECRET_REACH,
    CHECK_CLOUD_FALLBACK,
    CHECK_UNIT_MISSING,
    CHECK_UNIT_REPEATED,
    CHECK_START_CYCLE,
    CHECK_TARGET_UNKNOWN,
    CHECK_TARGET_KIND,
    CHECK_NEED_UNKNOWN,
    CHECK_NEED_OMITTED,
    CHECK_UNBOUND_DIFFERS,
    CHECK_SECRET_FALLBACK,
    CHECK_NEED_KIND,
    CHECK_NEED_CONSUMERS,
    CHECK_PROVIDE_UNKNOWN,
    CHECK_PROVIDE_REFERENCE,
];

/// Returns the command's code for the library code `code`. Every code of
/// the library has one, which a test checks; the declaration syntax code
/// stands in for a code that a later library version adds.
pub(crate) fn of_library(code: jumpgate::Code) -> Code<'static> {
    LIBRARY
        .iter()
        .find(|known| known.area() == code.area() && known.reason() == code.name())
        .copied()
        .unwrap_or(DECLARATION_SYNTAX)
}

/// Every code above.
pub const ALL: &[Code<'static>] = &[
    TARGET_MISSING,
    NEED_UNBOUND,
    NEED_PROVIDE_MISSING,
    NEED_AMBIGUOUS,
    NEED_FALLBACK_ON_CLOUD,
    NEED_KEPT,
    NEED_LOCAL_ONLY,
    NEED_SERVICE_KIND,
    REPO_PATH,
    REPO_GIT,
    REPO_REMOTE,
    REPO_DUPLICATE,
    DECLARATION_MISSING,
    DECLARATION_UNREADABLE,
    DECLARATION_TOO_LARGE,
    DECLARATION_SYNTAX,
    DECLARATION_NEED,
    DECLARATION_PROVIDE,
    DECLARATION_SOCKET_PATH,
    DECLARATION_USE_NEEDS,
    DECLARATION_USE_PROVIDES,
    DECLARATION_NAME,
    DECLARATION_STARMAP,
    DECLARATION_CLASH,
    DECLARATION_PROVIDER,
    DECLARATION_CONSUMERS,
    DECLARATION_DIRECTORY,
    DECLARATION_SERVICE,
    STORE_IO,
    STORE_CORRUPT,
    STORE_MISPLACED,
    STORE_BOARD_FILE,
    STORE_BOARD,
    RECORD_SYNTAX,
    RECORD_TOO_LARGE,
    RECORD_SHAPE,
    RECORD_WRONG_TYPE,
    RECORD_UNKNOWN_FIELD,
    RECORD_MISSING_FIELD,
    RECORD_NAME,
    RECORD_VARIABLE,
    RECORD_REPOSITORY,
    RECORD_FILE_PATH,
    RECORD_RECORD_PATH,
    RECORD_SECRET_REFERENCE,
    RECORD_KEYWORD,
    RECORD_DUPLICATE,
    RECORD_EMPTY,
    RECORD_REFERENCE_MISMATCH,
    RECORD_FALLBACK,
    RECORD_OPTIONAL_CONFLICT,
    CHECK_SECRET_VALUE,
    CHECK_NEED_UNBOUND,
    CHECK_PROVIDE_MISMATCH,
    CHECK_UNKNOWN_PROVIDER,
    CHECK_SECRET_REACH,
    CHECK_CLOUD_FALLBACK,
    CHECK_UNIT_MISSING,
    CHECK_UNIT_REPEATED,
    CHECK_START_CYCLE,
    CHECK_TARGET_UNKNOWN,
    CHECK_TARGET_KIND,
    CHECK_NEED_UNKNOWN,
    CHECK_NEED_OMITTED,
    CHECK_UNBOUND_DIFFERS,
    CHECK_SECRET_FALLBACK,
    CHECK_NEED_KIND,
    CHECK_NEED_CONSUMERS,
    CHECK_PROVIDE_UNKNOWN,
    CHECK_PROVIDE_REFERENCE,
];

#[cfg(test)]
mod tests {
    use jumpgate::check::Reason as Check;
    use jumpgate::declaration::Reason as Declaration;
    use jumpgate::store::Reason as Store;

    use super::*;

    #[test]
    fn every_library_code_maps_to_the_code_of_the_same_name() {
        let library = Declaration::ALL
            .iter()
            .map(|reason| reason.code())
            .chain(Store::ALL.iter().map(|reason| reason.code()))
            .chain(jumpgate::Reason::ALL.iter().map(|reason| reason.code()))
            .chain(Check::ALL.iter().map(|reason| reason.code()));
        for code in library {
            let mapped = of_library(code);
            assert_eq!((mapped.area(), mapped.reason()), (code.area(), code.name()));
        }
    }
}
