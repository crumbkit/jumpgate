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
    Allow, Examples, Failing, PathBudgets, Recoveries, Rule, Suite, Twin, Twins, Unprovoked,
};
use jumpgate_cli::codes::{
    ALL, COMMAND_NOT_BUILT, COMMAND_STORE_REQUIRED, DECLARATION_CLASH, DECLARATION_CONSUMERS,
    DECLARATION_DIRECTORY, DECLARATION_MISSING, DECLARATION_NAME, DECLARATION_PROVIDER,
    DECLARATION_STARMAP, DECLARATION_SYNTAX, DECLARATION_TOO_LARGE, DECLARATION_UNREADABLE,
    DECLARATION_WANT, NEED_AMBIGUOUS, NEED_FALLBACK_ON_CLOUD, NEED_OFFER_MISSING, NEED_UNBOUND,
    REPO_DUPLICATE, REPO_GIT, REPO_PATH, REPO_REMOTE, STORE_BOARD_FILE, STORE_CORRUPT, STORE_IO,
    STORE_MISPLACED, TARGET_MISSING,
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
    Allow::example("check", &[], UNBUILT_EXAMPLE),
    Allow::new(Rule::NextStep, "check", UNBUILT_NEXT_STEP),
    Allow::untwinned(
        Failing::new(COMMAND_NOT_BUILT, &["check"]),
        "No command line of an unbuilt command is accepted, so it has no twin; the change that builds check removes the failure.",
    ),
    Allow::option(Rule::RepeatsAndCommas, "needs", "--repo", REPO_COMMA),
];

/// The directory that each run of an example, a twin or a recovery starts
/// in, a fresh copy of it each time. It holds the checkout `shelf` and the
/// stores `records`, `unbound`, `missing-offer` and `fallback`.
const FIXTURE: &str = "tests/fixture";

/// jumpgate's built binary and the directory of its golden files.
const EXAMPLES: Examples<'_> =
    Examples::new(env!("CARGO_BIN_EXE_jumpgate"), "tests/golden/examples")
        .allow(ALLOW)
        .fresh_dir(FIXTURE);

/// `needs` without a store, which the same line with `--store=records` fixes.
const NEEDS_NO_STORE: Failing<'_> = Failing::new(
    COMMAND_STORE_REQUIRED,
    &["needs", "--repo", "github.com/acme/shelf=shelf"],
);

/// `needs` of a path that is no directory, which `shelf` fixes.
const NEEDS_NO_PATH: Failing<'_> = Failing::new(
    REPO_PATH,
    &[
        "needs",
        "--repo",
        "github.com/acme/shelf=nowhere",
        "--store",
        "records",
    ],
);

/// `needs` of one repository named twice, which a second id fixes.
const NEEDS_TWICE: Failing<'_> = Failing::new(
    REPO_DUPLICATE,
    &[
        "needs",
        "--repo",
        "github.com/acme/shelf=shelf",
        "--repo",
        "github.com/acme/shelf=shelf",
        "--store",
        "records",
    ],
);

/// `needs` of a directory that holds no `mxv.toml`, which `shelf` fixes.
const NEEDS_NO_DECLARATION: Failing<'_> = Failing::new(
    DECLARATION_MISSING,
    &[
        "needs",
        "--repo",
        "github.com/acme/shelf=records",
        "--store",
        "records",
    ],
);

/// `resolve` without a store, which the same line with `--store=records`
/// fixes.
const RESOLVE_NO_STORE: Failing<'_> =
    Failing::new(COMMAND_STORE_REQUIRED, &["resolve", "--target", "local"]);

/// `resolve` for a target the store lacks, which `local` fixes.
const RESOLVE_NO_TARGET: Failing<'_> = Failing::new(
    TARGET_MISSING,
    &["resolve", "--target", "staging", "--store", "records"],
);

/// `resolve` over a store with a required need that nothing meets, which
/// the store `records` fixes.
const RESOLVE_UNBOUND: Failing<'_> = Failing::new(
    NEED_UNBOUND,
    &["resolve", "--target", "local", "--store", "unbound"],
);

/// `resolve` over a store with a binding to an offer it lacks, which the
/// store `records` fixes.
const RESOLVE_MISSING_OFFER: Failing<'_> = Failing::new(
    NEED_OFFER_MISSING,
    &["resolve", "--target", "local", "--store", "missing-offer"],
);

/// `resolve` for a cloud target where only a fallback meets a need, which
/// the local target fixes.
const RESOLVE_CLOUD_FALLBACK: Failing<'_> = Failing::new(
    NEED_FALLBACK_ON_CLOUD,
    &["resolve", "--target", "cloud", "--store", "fallback"],
);

/// The command lines that provoke each of jumpgate's codes.
const STARTS: &[Failing<'_>] = &[
    NEEDS_NO_STORE,
    NEEDS_NO_PATH,
    NEEDS_TWICE,
    NEEDS_NO_DECLARATION,
    RESOLVE_NO_STORE,
    RESOLVE_NO_TARGET,
    RESOLVE_UNBOUND,
    RESOLVE_MISSING_OFFER,
    RESOLVE_CLOUD_FALLBACK,
    Failing::new(COMMAND_NOT_BUILT, &["check"]),
];

/// The lines that differ from a refused line by one word and succeed, each
/// with the refused line it proves the one word of.
const PAIRS: &[Twin<'_>] = &[
    Twin::new(
        NEEDS_NO_STORE,
        &[
            "needs",
            "--repo",
            "github.com/acme/shelf=shelf",
            "--store=records",
        ],
    ),
    Twin::new(
        NEEDS_NO_PATH,
        &[
            "needs",
            "--repo",
            "github.com/acme/shelf=shelf",
            "--store",
            "records",
        ],
    ),
    Twin::new(
        NEEDS_TWICE,
        &[
            "needs",
            "--repo",
            "github.com/acme/shelf=shelf",
            "--repo",
            "github.com/acme/other=shelf",
            "--store",
            "records",
        ],
    ),
    Twin::new(
        NEEDS_NO_DECLARATION,
        &[
            "needs",
            "--repo",
            "github.com/acme/shelf=shelf",
            "--store",
            "records",
        ],
    ),
    Twin::new(
        RESOLVE_NO_STORE,
        &["resolve", "--target", "local", "--store=records"],
    ),
    Twin::new(
        RESOLVE_NO_TARGET,
        &["resolve", "--target", "local", "--store", "records"],
    ),
    Twin::new(
        RESOLVE_UNBOUND,
        &["resolve", "--target", "local", "--store", "records"],
    ),
    Twin::new(
        RESOLVE_MISSING_OFFER,
        &["resolve", "--target", "local", "--store", "records"],
    ),
    Twin::new(
        RESOLVE_CLOUD_FALLBACK,
        &["resolve", "--target", "local", "--store", "fallback"],
    ),
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
    Unprovoked::new(NEED_AMBIGUOUS, AMBIGUOUS),
];

/// The reason that an ambiguous match has no start.
const AMBIGUOUS: &str = "Two offers meet one need only when two bindings for one need and target sit in the target's authority board, which a directory store, with one file for each need and target, cannot hold; the library's matcher tests provoke it with a store of another kind.";

/// The settings of jumpgate's recovery check.
const RECOVERIES: Recoveries<'_> = Recoveries::new(EXAMPLES, ALL)
    .starts(STARTS)
    .unprovoked(UNPROVOKED);

/// The settings of jumpgate's twin check.
const TWINS: Twins<'_> = Twins::new(EXAMPLES, ALL)
    .pairs(PAIRS)
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
    .twins(TWINS);

crumbkit_test::conventions!(SUITE);
