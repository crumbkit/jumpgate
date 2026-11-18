//! The `jumpgate` command's command-line grammar and tool record.

use clap::{Parser, Subcommand};
use crumbkit::commands::{COMMANDS_LIST, CommandsList};
use crumbkit::completion::{
    COMPLETION_CHECK, COMPLETION_INSTALL, COMPLETION_PRINT, CompletionCheck, CompletionInstall,
    CompletionPrint,
};
use crumbkit::help::{Effects, Example, Goal, Group, Help, Leaf};
use crumbkit::schemas::{SCHEMAS_LIST, SCHEMAS_SHOW, SchemasList, SchemasShow};
use crumbkit::{NoInput, Tool, ViewFlags};
use crumbkit_skills::{Skill, skills_command};

use crate::check::{CHECK, CheckArgs};
use crate::needs::{NEEDS, NeedsArgs};
use crate::resolve::{RESOLVE, ResolveArgs};

/// Match what each app needs to what a provider offers, and check the plan.
#[derive(Debug, Parser)]
#[command(name = "jumpgate", version)]
pub struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub(crate) verb: Verb,

    /// The flags that choose how the result prints.
    #[command(flatten)]
    pub(crate) view: ViewFlags,

    /// Ask no question. No command asks one yet, and `crumbkit::parse`
    /// reads the flag.
    #[command(flatten)]
    pub(crate) no_input: NoInput,
}

/// The `jumpgate` command's goal headings, root verbs, leaves and their
/// examples.
pub const TOOL: Tool = Tool::new().help(HELP);

/// The goal headings, root verbs and leaves of the `jumpgate` command.
const HELP: Help = Help::new()
    .groups(&[Group::new(
        &[],
        &[
            Goal::new(
                "Turn declarations into a checked plan",
                &["needs", "resolve", "check"],
            ),
            Goal::new("Explore commands", &["commands", "schemas"]),
            Goal::new("Install jumpgate's skill", &["skills"]),
            Goal::new("Set up shell completion", &["completion"]),
        ],
    )])
    .root_verbs(
        &["needs", "resolve", "check"],
        "Each step of the plan is one verb that a person types as one step of their work, and jumpgate has no resource word to put them under.",
    )
    .leaves(&[
        Leaf::new(&["needs"])
            .examples(&[Example::new(
                &["--repo", "../shelf", "--repo", "../auth", "--store", "./records"],
                "Read the declarations of two checkouts and write their needs, offers and units to ./records",
            ),
            Example::new(
                &["--repo", "../shelf", "--repo", "../auth"],
                "Write them to the personal board",
            )])
            // The command writes one file for each record it derives, and a
            // second run with the same declarations writes the same files.
            .effects(Effects::additive().idempotent())
            .document(NEEDS),
        Leaf::new(&["resolve"])
            .examples(&[
                Example::new(
                    &["--target", "local", "--store", "./records", "--out", "plan.json"],
                    "Match the needs in ./records to offers for the target local and write the plan to plan.json",
                ),
                Example::new(
                    &["--target", "local", "--board", "../team-board"],
                    "Print the plan for the target local from the board in ../team-board",
                ),
            ])
            // The command writes the plan file only when `--out` names one, and
            // it writes the same file again for the same records.
            .effects(Effects::additive().idempotent())
            .document(RESOLVE),
        Leaf::new(&["check"])
            .examples(&[
                Example::new(
                    &["plan.json"],
                    "Run the six checks on plan.json, compare it with the personal board and list the failed checks first",
                ),
                Example::new(
                    &["plan.json", "--plan-only"],
                    "Run the six checks on plan.json alone and read no board",
                ),
            ])
            // The command reads one plan file and the store, and writes nothing.
            .effects(Effects::read_only())
            .document(CHECK),
        Leaf::new(&["commands", "list"])
            .examples(&[Example::new(&[], "List every command")])
            .effects(Effects::read_only())
            .document(COMMANDS_LIST),
        Leaf::new(&["schemas", "list"])
            .examples(&[Example::new(&[], "List every document that jumpgate prints")])
            .effects(Effects::read_only())
            .document(SCHEMAS_LIST),
        Leaf::new(&["schemas", "show"])
            .examples(&[Example::new(
                &["jumpgate/schemas-list/v1"],
                "Print the JSON Schema of the document that schemas list prints",
            )])
            .effects(Effects::read_only())
            .document(SCHEMAS_SHOW),
        JumpgateSkills::install_leaf(&["skills", "install"]),
        JumpgateSkills::check_leaf(&["skills", "check"]),
        JumpgateSkills::print_leaf(&["skills", "print"]),
        Leaf::new(&["completion", "install"])
            .examples(&[
                Example::new(&["--shell", "fish"], "Install jumpgate's completion for fish"),
                Example::new(
                    &["--shell", "zsh", "--force"],
                    "Install it for zsh, replacing a file or a link that install did not write",
                ),
            ])
            .effects(Effects::additive().idempotent())
            .document(COMPLETION_INSTALL),
        Leaf::new(&["completion", "check"])
            .examples(&[Example::new(
                &["--shell", "zsh"],
                "Report zsh's completion file when it is missing or differs",
            )])
            .effects(Effects::read_only())
            .document(COMPLETION_CHECK),
        Leaf::new(&["completion", "print"])
            .examples(&[Example::new(
                &["--shell", "fish"],
                "Print the registration script for fish",
            )])
            .effects(Effects::read_only())
            .document(COMPLETION_PRINT),
    ]);

skills_command!(
    /// The commands for jumpgate's skills.
    pub(crate) JumpgateSkills,
    "jumpgate"
);

/// jumpgate's agent skill, which `jumpgate skills install` writes into the
/// agent skills directories.
pub(crate) const SKILL: Skill = Skill::new(
    "jumpgate",
    env!("CARGO_PKG_VERSION"),
    include_str!("../skill/SKILL.md"),
);

/// The commands of `jumpgate`.
#[derive(Debug, Subcommand)]
pub(crate) enum Verb {
    /// Read the declarations of a set of repositories and write the need
    /// records, the offers each app provides and the units each app runs.
    ///
    /// It reads `mxv.toml` and the `starmap.toml` files that its processes
    /// read in each checkout, and runs no process but `git remote get-url
    /// origin` and `board`. The records go to the personal board unless
    /// `--store` names a directory or `--board` names another board. A
    /// declaration it refuses is reported and exits with 1, and the records
    /// of everything else are still written. It deletes the records that an
    /// app it read without a refusal no longer declares.
    Needs(NeedsArgs),
    /// Match each need to an offer for one target, write the plan and list
    /// the needs that nothing binds.
    ///
    /// It reads the target, needs, offers, units and bindings in a store, the
    /// personal board unless `--store` or `--board` names another, and runs
    /// no process but `board`. A need that nothing meets, and an ambiguous match,
    /// is listed before the bound needs and makes the command exit with 1
    /// after it writes the plan. The plan holds references and no secret
    /// value.
    Resolve(ResolveArgs),
    /// Run every check on a plan, whichever matcher wrote it.
    ///
    /// It reads the plan file and the store the plan came from, the personal
    /// board unless `--store` or `--board` names another, and writes
    /// nothing. It reads no secret store and resolves no reference. The
    /// plan's target, each need's kind and consumers, and each binding's need
    /// and offer must exist in the store and agree with the plan, and
    /// `--plan-only` skips that comparison. A plan that does not parse is
    /// refused with its parse code and no check runs. Otherwise every check
    /// runs, the failed checks are listed before the passed ones, and
    /// the command exits with 1 when a check failed. The checks are that the
    /// plan holds no secret value, every need is bound or has a stated
    /// when_unbound, every binding names an offer with a known provider, a
    /// secret reaches only the units that declare it, a cloud plan has no
    /// need met by a fallback, and every unit that a unit starts after
    /// exists with no cycle in the start order.
    Check(CheckArgs),
    /// Explore jumpgate's commands.
    Commands {
        /// What to do with the commands.
        #[command(subcommand)]
        command: CommandsCommand,
    },
    /// Explore the JSON documents that jumpgate prints.
    Schemas {
        /// What to do with the documents.
        #[command(subcommand)]
        command: SchemasCommand,
    },
    /// Give coding agents jumpgate's skill.
    Skills {
        /// What to do with the skill.
        #[command(subcommand)]
        command: JumpgateSkills,
    },
    /// Set up jumpgate's completion in a shell.
    Completion {
        /// What to do with the completion.
        #[command(subcommand)]
        command: CompletionCommand,
    },
}

/// The commands for jumpgate's own commands.
#[derive(Debug, Subcommand)]
pub(crate) enum CommandsCommand {
    /// List a branch of jumpgate's commands with their arguments, examples and columns.
    List(CommandsList),
}

/// The commands for the JSON documents that jumpgate prints.
#[derive(Debug, Subcommand)]
pub(crate) enum SchemasCommand {
    /// List the JSON documents that jumpgate prints, with a description of each.
    List(SchemasList),
    /// Print the JSON Schema of each document.
    Show(SchemasShow),
}

/// The commands for jumpgate's shell completion.
#[derive(Debug, Subcommand)]
pub(crate) enum CompletionCommand {
    /// Install jumpgate's completion where the shell loads it.
    Install(CompletionInstall),
    /// Check that jumpgate's installed completion file matches this jumpgate.
    Check(CompletionCheck),
    /// Print jumpgate's completion registration script for a shell.
    Print(CompletionPrint),
}
