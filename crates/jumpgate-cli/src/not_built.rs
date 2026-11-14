//! The failure of a command that this version of jumpgate does not build
//! yet, and the documents those commands will print.

use std::fmt;

use crumbkit::{Document, Failure, Prose};

use crate::codes::COMMAND_NOT_BUILT;

/// The result that an unbuilt command declares. The command never returns
/// one, because it always fails. The change that builds each command
/// replaces it with the command's own result type, and the document's schema
/// changes with it.
pub(crate) type Pending = Prose<String>;

/// The document that `jumpgate resolve` prints once it is built.
pub(crate) const RESOLVE: Document<Pending> = Document::new(
    "jumpgate",
    "resolve",
    1,
    "The plan for one target and the needs left unbound.",
);

/// The document that `jumpgate check` prints once it is built.
pub(crate) const CHECK: Document<Pending> = Document::new(
    "jumpgate",
    "check",
    1,
    "The result of each check on a plan.",
);

/// A command that this version of jumpgate does not build yet.
#[derive(Debug, Failure)]
#[failure(
    code = COMMAND_NOT_BUILT,
    no_next_step = "this version of jumpgate holds the record types only, and a later version builds jumpgate {command}",
    field(command = *command)
)]
pub(crate) struct NotBuilt {
    /// The command's name, such as `needs`.
    command: &'static str,
}

impl NotBuilt {
    /// Returns the failed result of the command `command`.
    pub(crate) const fn result(command: &'static str) -> Result<Pending, Self> {
        Err(Self { command })
    }
}

impl fmt::Display for NotBuilt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "jumpgate {} is not built yet", self.command)
    }
}

impl std::error::Error for NotBuilt {}
