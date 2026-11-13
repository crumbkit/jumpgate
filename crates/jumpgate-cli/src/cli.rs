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
use crumbkit_skills::{
    SKILLS_CHECK, SKILLS_INSTALL, SKILLS_PRINT, Skill, SkillsCheck, SkillsInstall,
};

use crate::not_built::{CHECK, NEEDS, RESOLVE};

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
                &[],
                "Read the declarations and write the need records (not built yet)",
            )])
            // The effects change when `needs` is built, because it writes need records.
            .effects(Effects::read_only())
            .document(NEEDS),
        Leaf::new(&["resolve"])
            .examples(&[Example::new(
                &[],
                "Match needs to offers and write the plan (not built yet)",
            )])
            // The effects change when `resolve` is built, because it writes the plan.
            .effects(Effects::read_only())
            .document(RESOLVE),
        Leaf::new(&["check"])
            .examples(&[Example::new(&[], "Run every check on a plan (not built yet)")])
            // The effects are set again when `check` is built, from what its checks read.
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
        Leaf::new(&["skills", "install"])
            .examples(&[
                Example::new(
                    &[],
                    "Install jumpgate's skill into each agent skills directory that exists",
                ),
                Example::new(
                    &["--force"],
                    "Install it, replacing a file or a link that install did not write",
                ),
            ])
            .effects(Effects::additive().idempotent())
            .document(SKILLS_INSTALL),
        Leaf::new(&["skills", "check"])
            .examples(&[Example::new(
                &[],
                "Report each agent skills directory whose copy is missing or differs",
            )])
            .effects(Effects::read_only())
            .document(SKILLS_CHECK),
        Leaf::new(&["skills", "print"])
            .examples(&[Example::new(&[], "Print jumpgate's skill")])
            .effects(Effects::read_only())
            .document(SKILLS_PRINT),
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
    /// records and the offers each app provides.
    ///
    /// This version of jumpgate does not build needs yet, and the command
    /// fails with jumpgate::command::not_built.
    Needs,
    /// Match each need to an offer for one target, write the plan and list
    /// the needs that nothing binds.
    ///
    /// This version of jumpgate does not build resolve yet, and the command
    /// fails with jumpgate::command::not_built.
    Resolve,
    /// Run every check on a plan, whichever matcher wrote it.
    ///
    /// This version of jumpgate does not build check yet, and the command
    /// fails with jumpgate::command::not_built.
    Check,
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
        command: SkillsCommand,
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

/// The commands for jumpgate's agent skill.
#[derive(Debug, Subcommand)]
pub(crate) enum SkillsCommand {
    /// Install jumpgate's skill into each agent skills directory that exists.
    Install(SkillsInstall),
    /// Check that each installed copy of jumpgate's skill matches this jumpgate.
    Check(SkillsCheck),
    /// Print jumpgate's skill.
    Print,
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
