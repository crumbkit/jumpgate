//! The failure codes of the `jumpgate` command. Each code has the form
//! `jumpgate::<area>::<name>`, where the area is a noun from jumpgate's
//! domain (CLI-3). The library's refusals reach the command with the same
//! area and name they have in the library.

use crumbkit::Code;

/// The command exists in the command tree, and this version of jumpgate
/// does not do its work yet.
pub const COMMAND_NOT_BUILT: Code = Code::new("jumpgate", "command", "not_built");

/// `jumpgate needs` was run without `--store`, and the personal board that
/// it would use instead is not built yet.
pub const COMMAND_STORE_REQUIRED: Code = Code::new("jumpgate", "command", "store_required");

/// A `--repo` path is not a directory.
pub const REPO_PATH: Code = Code::new("jumpgate", "repo", "path");

/// git did not give the URL of the checkout's `origin`.
pub const REPO_GIT: Code = Code::new("jumpgate", "repo", "git");

/// The checkout's `origin` is not a remote of the form `host/owner/name`.
pub const REPO_REMOTE: Code = Code::new("jumpgate", "repo", "remote");

/// Two `--repo` values name one repository.
pub const REPO_DUPLICATE: Code = Code::new("jumpgate", "repo", "duplicate");

/// The repository has no `mxv.toml`.
pub const DECLARATION_MISSING: Code = Code::new("jumpgate", "declaration", "missing");
/// A declaration file exists and cannot be read.
pub const DECLARATION_UNREADABLE: Code = Code::new("jumpgate", "declaration", "unreadable");
/// A declaration file is longer than the reader accepts.
pub const DECLARATION_TOO_LARGE: Code = Code::new("jumpgate", "declaration", "too_large");
/// A declaration file is not valid TOML.
pub const DECLARATION_SYNTAX: Code = Code::new("jumpgate", "declaration", "syntax");
/// A want is not a table, has an unknown or missing key, or a bad value.
pub const DECLARATION_WANT: Code = Code::new("jumpgate", "declaration", "want");
/// A name breaks the rule of the record that holds it.
pub const DECLARATION_NAME: Code = Code::new("jumpgate", "declaration", "name");
/// A `starmap.toml` does not parse as a starmap file.
pub const DECLARATION_STARMAP: Code = Code::new("jumpgate", "declaration", "starmap");
/// Two declarations of one app would write the same record.
pub const DECLARATION_CLASH: Code = Code::new("jumpgate", "declaration", "clash");
/// A want names a repository that was read and does not provide the name.
pub const DECLARATION_PROVIDER: Code = Code::new("jumpgate", "declaration", "provider");
/// A want or a secret has no unit to receive it.
pub const DECLARATION_CONSUMERS: Code = Code::new("jumpgate", "declaration", "consumers");
/// A process's `cwd` is not a path within the checkout.
pub const DECLARATION_DIRECTORY: Code = Code::new("jumpgate", "declaration", "directory");

/// The file system refused to read or write a store file.
pub const STORE_IO: Code = Code::new("jumpgate", "store", "io");
/// A store file is not a valid record.
pub const STORE_CORRUPT: Code = Code::new("jumpgate", "store", "corrupt");
/// A record's file is not at the path of the record it holds.
pub const STORE_MISPLACED: Code = Code::new("jumpgate", "store", "misplaced");
/// The store's board file or a board directory is not a board id.
pub const STORE_BOARD_FILE: Code = Code::new("jumpgate", "store", "board_file");

/// The codes that the library's refusals and store errors map onto.
const LIBRARY: &[&Code] = &[
    &DECLARATION_MISSING,
    &DECLARATION_UNREADABLE,
    &DECLARATION_TOO_LARGE,
    &DECLARATION_SYNTAX,
    &DECLARATION_WANT,
    &DECLARATION_NAME,
    &DECLARATION_STARMAP,
    &DECLARATION_CLASH,
    &DECLARATION_PROVIDER,
    &DECLARATION_CONSUMERS,
    &DECLARATION_DIRECTORY,
    &STORE_IO,
    &STORE_CORRUPT,
    &STORE_MISPLACED,
    &STORE_BOARD_FILE,
];

/// Returns the command's code for the library code `code`. Every code of
/// the library has one, which a test checks; the declaration syntax code
/// stands in for a code that a later library version adds.
pub(crate) fn of_library(code: jumpgate::Code) -> &'static Code {
    LIBRARY
        .iter()
        .find(|known| known.area() == code.area() && known.reason() == code.name())
        .copied()
        .unwrap_or(&DECLARATION_SYNTAX)
}

/// Every code above.
pub const ALL: &[&Code] = &[
    &COMMAND_NOT_BUILT,
    &COMMAND_STORE_REQUIRED,
    &REPO_PATH,
    &REPO_GIT,
    &REPO_REMOTE,
    &REPO_DUPLICATE,
    &DECLARATION_MISSING,
    &DECLARATION_UNREADABLE,
    &DECLARATION_TOO_LARGE,
    &DECLARATION_SYNTAX,
    &DECLARATION_WANT,
    &DECLARATION_NAME,
    &DECLARATION_STARMAP,
    &DECLARATION_CLASH,
    &DECLARATION_PROVIDER,
    &DECLARATION_CONSUMERS,
    &DECLARATION_DIRECTORY,
    &STORE_IO,
    &STORE_CORRUPT,
    &STORE_MISPLACED,
    &STORE_BOARD_FILE,
];

#[cfg(test)]
mod tests {
    use jumpgate::declaration::Reason as Declaration;
    use jumpgate::store::Reason as Store;

    use super::*;

    #[test]
    fn every_library_code_maps_to_the_code_of_the_same_name() {
        let library = Declaration::ALL
            .iter()
            .map(|reason| reason.code())
            .chain(Store::ALL.iter().map(|reason| reason.code()));
        for code in library {
            let mapped = of_library(code);
            assert_eq!((mapped.area(), mapped.reason()), (code.area(), code.name()));
        }
    }
}
