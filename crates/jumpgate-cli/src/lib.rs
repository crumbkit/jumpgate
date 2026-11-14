//! The `jumpgate` command, which reads app declarations, matches needs to
//! offers and checks the plan.
//!
//! The library holds the whole command, and the `jumpgate` binary calls
//! `run`. It exports the parser, the tool record and the failure codes, so
//! the integration tests can pass them to crumbkit-test's checks beside the
//! built binary. It is not an API for other crates, so each public item is
//! hidden from the documentation and outside the semver promise.

mod checkout;
mod cli;
#[doc(hidden)]
pub mod codes;
mod git;
mod needs;
mod not_built;
mod remote;
mod repo_arg;

use std::ffi::OsString;
use std::process::ExitCode;

use clap::CommandFactory as _;

#[doc(hidden)]
pub use crate::cli::{Cli, TOOL};
use crate::cli::{CommandsCommand, CompletionCommand, SKILL, SchemasCommand, Verb};
use crate::not_built::NotBuilt;

/// Parses `args`, runs the command and returns the process's exit code. The
/// first argument is the program's name.
///
/// `crumbkit::parse_from` prints help or a refused command line and returns
/// the exit code. `crumbkit::finish` prints the result or the failure in
/// the format that the output flags choose, and returns the exit code.
#[doc(hidden)]
#[must_use]
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let (cli, mut output) = match crumbkit::parse_from::<Cli>(&TOOL, args) {
        Ok(parsed) => parsed,
        Err(stop) => return stop.exit_code(),
    };
    match cli.verb {
        Verb::Needs(args) => crumbkit::finish(output, needs::NEEDS, needs::run(&args)),
        Verb::Resolve => crumbkit::finish(output, not_built::RESOLVE, NotBuilt::result("resolve")),
        Verb::Check => crumbkit::finish(output, not_built::CHECK, NotBuilt::result("check")),
        // `commands list` and `schemas` read the command tree through
        // `&mut output`, so each runs before `finish` takes `output`.
        Verb::Commands {
            command: CommandsCommand::List(args),
        } => {
            let result = crumbkit::commands::list(&mut output, &TOOL, &args);
            crumbkit::finish(output, crumbkit::commands::COMMANDS_LIST, result)
        }
        Verb::Schemas {
            command: SchemasCommand::List(args),
        } => {
            let result = crumbkit::schemas::list(&mut output, &TOOL, &args);
            crumbkit::finish(output, crumbkit::schemas::SCHEMAS_LIST, result)
        }
        Verb::Schemas {
            command: SchemasCommand::Show(args),
        } => {
            let result = crumbkit::schemas::show(&mut output, &TOOL, &args);
            crumbkit::finish(output, crumbkit::schemas::SCHEMAS_SHOW, result)
        }
        Verb::Skills { command } => crumbkit_skills::run(output, &[SKILL], &command),
        Verb::Completion { command } => match command {
            CompletionCommand::Install(args) => {
                let result = crumbkit::completion::install(Cli::command, &args);
                crumbkit::finish(output, crumbkit::completion::COMPLETION_INSTALL, result)
            }
            CompletionCommand::Check(args) => {
                let result = crumbkit::completion::check(Cli::command, &args);
                crumbkit::finish(output, crumbkit::completion::COMPLETION_CHECK, result)
            }
            CompletionCommand::Print(args) => {
                let result = crumbkit::completion::print(Cli::command, &args);
                crumbkit::finish(output, crumbkit::completion::COMPLETION_PRINT, result)
            }
        },
    }
}
