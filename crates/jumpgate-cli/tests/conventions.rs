//! The convention tests of the `jumpgate` command, which hold its command
//! tree, tool record and failure codes to crumbkit's conventions with
//! crumbkit-test (CLI-6).
//!
//! They live in an integration test because the checks that run examples
//! and recoveries run the built binary, whose path cargo sets only for
//! integration tests. `SNAPSHOTS=overwrite cargo nextest run -p jumpgate-cli
//! --test conventions` writes and rewrites the golden files.

#![cfg(test)]

use clap::CommandFactory as _;
use crumbkit_test::{
    Allow, Check, Examples, Failing, PathBudgets, Recoveries, Rule, Suite, Unprovoked,
};
use jumpgate_cli::codes::{
    ALL, COMMAND_NOT_BUILT, COMMAND_STORE_REQUIRED, DECLARATION_CLASH, DECLARATION_CONSUMERS,
    DECLARATION_DIRECTORY, DECLARATION_MISSING, DECLARATION_NAME, DECLARATION_PROVIDER,
    DECLARATION_STARMAP, DECLARATION_SYNTAX, DECLARATION_TOO_LARGE, DECLARATION_UNREADABLE,
    DECLARATION_WANT, REPO_DUPLICATE, REPO_GIT, REPO_PATH, REPO_REMOTE, STORE_BOARD_FILE,
    STORE_CORRUPT, STORE_IO, STORE_MISPLACED,
};
use jumpgate_cli::{Cli, TOOL};

/// The reason that the examples of the unbuilt commands do not run.
const UNBUILT_EXAMPLE: &str = "The command is not built yet and fails with jumpgate::command::not_built, which tests/commands.rs pins; the change that builds it runs its examples.";

/// The reason that `needs` has no example the check can run.
const NEEDS_EXAMPLE: &str = "The example reads checkouts that the check's directory does not hold, so a run fails with jumpgate::repo::path; tests/commands.rs runs the command against fixture checkouts made with git init.";

/// The reason that the unbuilt commands' failure offers no next step.
const UNBUILT_NEXT_STEP: &str = "No command does the unbuilt command's work in this version, so the failure states that it has no next step; the change that builds the command removes the failure.";

/// The reason that `--repo` takes no comma list.
const REPO_COMMA: &str = "A path may hold a comma, so one use of --repo names one checkout.";

/// jumpgate's opt-outs from crumbkit's conventions, which every check reads.
const ALLOW: &[Allow] = &[
    Allow::example(
        "needs",
        &[
            "--repo",
            "../shelf",
            "--repo",
            "../auth",
            "--store",
            "./records",
        ],
        NEEDS_EXAMPLE,
    ),
    Allow::example("resolve", &[], UNBUILT_EXAMPLE),
    Allow::example("check", &[], UNBUILT_EXAMPLE),
    Allow::new(Rule::NextStep, "resolve", UNBUILT_NEXT_STEP),
    Allow::new(Rule::NextStep, "check", UNBUILT_NEXT_STEP),
    Allow::option(Rule::RepeatsAndCommas, "needs", "--repo", REPO_COMMA),
];

/// jumpgate's built binary and the directory of its golden files.
const EXAMPLES: Examples<'_> =
    Examples::new(env!("CARGO_BIN_EXE_jumpgate"), "tests/golden/examples").allow(ALLOW);

/// The command lines that provoke each of jumpgate's codes.
const STARTS: &[Failing<'_>] = &[
    Failing::new(COMMAND_STORE_REQUIRED, &["needs", "--repo", "."]),
    Failing::new(
        REPO_PATH,
        &["needs", "--repo", "nowhere", "--store", "records"],
    ),
    Failing::new(
        REPO_DUPLICATE,
        &[
            "needs",
            "--repo",
            "github.com/a/b=.",
            "--repo",
            "github.com/a/b=.",
            "--store",
            "records",
        ],
    ),
    Failing::new(
        DECLARATION_MISSING,
        &["needs", "--repo", "github.com/a/b=.", "--store", "records"],
    ),
    Failing::new(COMMAND_NOT_BUILT, &["resolve"]),
    Failing::new(COMMAND_NOT_BUILT, &["check"]),
];

/// The reason that a code of `needs` has no start here.
const NEEDS_FIXTURE: &str = "Only a checkout made with git init, or a damaged declaration or store file, provokes it, which a plain command line cannot make, and tests/commands.rs and the library's tests provoke it against fixtures.";

/// The codes that no start of this suite provokes.
const UNPROVOKED: &[Unprovoked<'_>] = &[
    Unprovoked::new(REPO_GIT, NEEDS_FIXTURE),
    Unprovoked::new(REPO_REMOTE, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_UNREADABLE, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_TOO_LARGE, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_SYNTAX, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_WANT, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_NAME, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_STARMAP, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_CLASH, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_PROVIDER, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_CONSUMERS, NEEDS_FIXTURE),
    Unprovoked::new(DECLARATION_DIRECTORY, NEEDS_FIXTURE),
    Unprovoked::new(STORE_IO, NEEDS_FIXTURE),
    Unprovoked::new(STORE_CORRUPT, NEEDS_FIXTURE),
    Unprovoked::new(STORE_MISPLACED, NEEDS_FIXTURE),
    Unprovoked::new(STORE_BOARD_FILE, NEEDS_FIXTURE),
];

/// The settings of jumpgate's recovery check.
const RECOVERIES: Recoveries<'_> = Recoveries::new(EXAMPLES, &[ALL])
    .starts(STARTS)
    .unprovoked(UNPROVOKED);

/// The budgets of the paths through the help, which are crumbkit's
/// defaults.
const PATH_BUDGETS: PathBudgets<'_> = PathBudgets::new("tests/golden/examples");

/// Everything the checks read, which `conventions!` expands to one test for
/// each check.
const SUITE: Suite<'_> = Suite::new(|| (Cli::command(), TOOL))
    .allow(ALLOW)
    .verbs(&["install", "check", "print"])
    .examples(EXAMPLES)
    .recoveries(RECOVERIES)
    .path_budgets(PATH_BUDGETS)
    .skip(
        Check::Twins,
        "jumpgate::command::not_built is the only code, and no command line of an unbuilt command is accepted, so it has no twin; the change that builds each command writes the twins of its codes.",
    );

crumbkit_test::conventions!(SUITE);
