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
use crumbkit_test::{Allow, Check, Examples, Failing, PathBudgets, Recoveries, Rule, Suite};
use jumpgate_cli::codes::{ALL, COMMAND_NOT_BUILT};
use jumpgate_cli::{Cli, TOOL};

/// The reason that the examples of the unbuilt commands do not run.
const UNBUILT_EXAMPLE: &str = "The command is not built yet and fails with jumpgate::command::not_built, which tests/commands.rs pins; the change that builds it runs its examples.";

/// The reason that the unbuilt commands' failure offers no next step.
const UNBUILT_NEXT_STEP: &str = "No command does the unbuilt command's work in this version, so the failure states that it has no next step; the change that builds the command removes the failure.";

/// jumpgate's opt-outs from crumbkit's conventions, which every check reads.
const ALLOW: &[Allow] = &[
    Allow::example("needs", &[], UNBUILT_EXAMPLE),
    Allow::example("resolve", &[], UNBUILT_EXAMPLE),
    Allow::example("check", &[], UNBUILT_EXAMPLE),
    Allow::new(Rule::NextStep, "needs", UNBUILT_NEXT_STEP),
    Allow::new(Rule::NextStep, "resolve", UNBUILT_NEXT_STEP),
    Allow::new(Rule::NextStep, "check", UNBUILT_NEXT_STEP),
];

/// jumpgate's built binary and the directory of its golden files.
const EXAMPLES: Examples<'_> =
    Examples::new(env!("CARGO_BIN_EXE_jumpgate"), "tests/golden/examples").allow(ALLOW);

/// The command lines that provoke each of jumpgate's codes.
const STARTS: &[Failing<'_>] = &[
    Failing::new(&COMMAND_NOT_BUILT, &["needs"]),
    Failing::new(&COMMAND_NOT_BUILT, &["resolve"]),
    Failing::new(&COMMAND_NOT_BUILT, &["check"]),
];

/// The settings of jumpgate's recovery check.
const RECOVERIES: Recoveries<'_> = Recoveries::new(EXAMPLES, &[ALL]).starts(STARTS);

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
