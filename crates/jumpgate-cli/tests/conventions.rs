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
    ALL, CHECK_CLOUD_FALLBACK, CHECK_NEED_CONSUMERS, CHECK_NEED_KIND, CHECK_NEED_OMITTED,
    CHECK_NEED_UNBOUND, CHECK_NEED_UNKNOWN, CHECK_OFFER_MISMATCH, CHECK_OFFER_REFERENCE,
    CHECK_OFFER_UNKNOWN, CHECK_SECRET_FALLBACK, CHECK_SECRET_REACH, CHECK_SECRET_VALUE,
    CHECK_START_CYCLE, CHECK_TARGET_KIND, CHECK_TARGET_UNKNOWN, CHECK_UNBOUND_DIFFERS,
    CHECK_UNIT_MISSING, CHECK_UNIT_REPEATED, CHECK_UNKNOWN_PROVIDER, DECLARATION_CLASH,
    DECLARATION_CONSUMERS, DECLARATION_DIRECTORY, DECLARATION_MISSING, DECLARATION_NAME,
    DECLARATION_PROVIDER, DECLARATION_STARMAP, DECLARATION_SYNTAX, DECLARATION_TOO_LARGE,
    DECLARATION_UNREADABLE, DECLARATION_WANT, NEED_AMBIGUOUS, NEED_FALLBACK_ON_CLOUD, NEED_KEPT,
    NEED_OFFER_MISSING, NEED_UNBOUND, RECORD_DUPLICATE, RECORD_EMPTY, RECORD_FALLBACK,
    RECORD_FILE_PATH, RECORD_KEYWORD, RECORD_MISSING_FIELD, RECORD_NAME, RECORD_OPTIONAL_CONFLICT,
    RECORD_RECORD_PATH, RECORD_REFERENCE_MISMATCH, RECORD_REPOSITORY, RECORD_SECRET_REFERENCE,
    RECORD_SHAPE, RECORD_SYNTAX, RECORD_TOO_LARGE, RECORD_UNKNOWN_FIELD, RECORD_VARIABLE,
    RECORD_WRONG_TYPE, REPO_DUPLICATE, REPO_GIT, REPO_PATH, REPO_REMOTE, STORE_BOARD,
    STORE_BOARD_FILE, STORE_CORRUPT, STORE_IO, STORE_MISPLACED, TARGET_MISSING,
};
use jumpgate_cli::{Cli, TOOL};

/// The reason that `needs` has no example the check can run.
const NEEDS_EXAMPLE: &str = "The example reads checkouts that the check's directory does not hold, so a run fails with jumpgate::repo::path; tests/commands.rs runs the command against fixture checkouts made with git init.";

/// The reason that a run of the example reads a board.
const BOARD_EXAMPLE: &str = "The example reads or writes a board that the check's directory does not hold, and the check must not touch the personal board; tests/commands.rs runs the command against a real temporary board made with board init.";

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
    Allow::example(
        "needs",
        &["--repo", "../shelf", "--repo", "../auth"],
        BOARD_EXAMPLE,
    ),
    Allow::example(
        "resolve",
        &["--target", "local", "--board", "../team-board"],
        BOARD_EXAMPLE,
    ),
    Allow::example("check", &["plan.json"], BOARD_EXAMPLE),
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

/// `check` of a plan that fails the secret-value rule, which `plan.json` fixes.
const CHECK_SECRET_VALUE_START: Failing<'_> = Failing::new(
    CHECK_SECRET_VALUE,
    &["check", "plans/secret-value.json", "--plan-only"],
);

/// `check` of a plan that fails the need-unbound rule, which `plan.json` fixes.
const CHECK_NEED_UNBOUND_START: Failing<'_> = Failing::new(
    CHECK_NEED_UNBOUND,
    &["check", "plans/need-unbound.json", "--plan-only"],
);

/// `check` of a plan that fails the offer-mismatch rule, which `plan.json` fixes.
const CHECK_OFFER_MISMATCH_START: Failing<'_> = Failing::new(
    CHECK_OFFER_MISMATCH,
    &["check", "plans/offer-mismatch.json", "--plan-only"],
);

/// `check` of a plan that fails the unknown-provider rule, which `plan.json` fixes.
const CHECK_UNKNOWN_PROVIDER_START: Failing<'_> = Failing::new(
    CHECK_UNKNOWN_PROVIDER,
    &["check", "plans/unknown-provider.json", "--plan-only"],
);

/// `check` of a plan that fails the secret-reach rule, which `plan.json` fixes.
const CHECK_SECRET_REACH_START: Failing<'_> = Failing::new(
    CHECK_SECRET_REACH,
    &["check", "plans/secret-reach.json", "--plan-only"],
);

/// `check` of a plan that fails the cloud-fallback rule, which `plan.json` fixes.
const CHECK_CLOUD_FALLBACK_START: Failing<'_> = Failing::new(
    CHECK_CLOUD_FALLBACK,
    &["check", "plans/cloud-fallback.json", "--plan-only"],
);

/// `check` of a plan that fails the unit-missing rule, which `plan.json` fixes.
const CHECK_UNIT_MISSING_START: Failing<'_> = Failing::new(
    CHECK_UNIT_MISSING,
    &["check", "plans/unit-missing.json", "--plan-only"],
);

/// `check` of a plan that fails the unit-repeated rule, which `plan.json` fixes.
const CHECK_UNIT_REPEATED_START: Failing<'_> = Failing::new(
    CHECK_UNIT_REPEATED,
    &["check", "plans/unit-repeated.json", "--plan-only"],
);

/// `check` of a plan that fails the start-cycle rule, which `plan.json` fixes.
const CHECK_START_CYCLE_START: Failing<'_> = Failing::new(
    CHECK_START_CYCLE,
    &["check", "plans/start-cycle.json", "--plan-only"],
);

/// `check` of a file that does not exist, which `plan.json` fixes.
const CHECK_NO_FILE: Failing<'_> =
    Failing::new(STORE_IO, &["check", "plans/nowhere.json", "--plan-only"]);

/// `check` of a file that is not JSON, which `plan.json` fixes.
const CHECK_SYNTAX: Failing<'_> = Failing::new(
    RECORD_SYNTAX,
    &["check", "plans/syntax.json", "--plan-only"],
);

/// `check` of a record of another type, which `plan.json` fixes.
const CHECK_SHAPE: Failing<'_> = Failing::new(
    RECORD_WRONG_TYPE,
    &["check", "plans/wrong-type.json", "--plan-only"],
);

/// `check` of a plan with a field that the type lacks, which `plan.json` fixes.
const CHECK_UNKNOWN_FIELD: Failing<'_> = Failing::new(
    RECORD_UNKNOWN_FIELD,
    &["check", "plans/unknown-field.json", "--plan-only"],
);

/// The command lines that provoke each of jumpgate's codes.
const STARTS: &[Failing<'_>] = &[
    NEEDS_NO_PATH,
    NEEDS_TWICE,
    NEEDS_NO_DECLARATION,
    RESOLVE_NO_TARGET,
    RESOLVE_UNBOUND,
    RESOLVE_MISSING_OFFER,
    RESOLVE_CLOUD_FALLBACK,
    CHECK_SECRET_VALUE_START,
    CHECK_NEED_UNBOUND_START,
    CHECK_OFFER_MISMATCH_START,
    CHECK_UNKNOWN_PROVIDER_START,
    CHECK_SECRET_REACH_START,
    CHECK_CLOUD_FALLBACK_START,
    CHECK_UNIT_MISSING_START,
    CHECK_UNIT_REPEATED_START,
    CHECK_START_CYCLE_START,
    CHECK_NO_FILE,
    CHECK_SYNTAX,
    CHECK_SHAPE,
    CHECK_UNKNOWN_FIELD,
];

/// The lines that differ from a refused line by one word and succeed, each
/// with the refused line it proves the one word of.
const PAIRS: &[Twin<'_>] = &[
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
    Twin::new(
        CHECK_SECRET_VALUE_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(
        CHECK_NEED_UNBOUND_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(
        CHECK_OFFER_MISMATCH_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(
        CHECK_UNKNOWN_PROVIDER_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(
        CHECK_SECRET_REACH_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(
        CHECK_CLOUD_FALLBACK_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(
        CHECK_UNIT_MISSING_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(
        CHECK_UNIT_REPEATED_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(
        CHECK_START_CYCLE_START,
        &["check", "plan.json", "--plan-only"],
    ),
    Twin::new(CHECK_NO_FILE, &["check", "plan.json", "--plan-only"]),
    Twin::new(CHECK_SYNTAX, &["check", "plan.json", "--plan-only"]),
    Twin::new(CHECK_SHAPE, &["check", "plan.json", "--plan-only"]),
    Twin::new(CHECK_UNKNOWN_FIELD, &["check", "plan.json", "--plan-only"]),
];

/// The reason that a code of `needs` has no start here.
const NEEDS_FIXTURE: &str = "Only a checkout made with git init, or a damaged declaration or store file, provokes it, which a plain command line cannot make, and tests/commands.rs and the library's tests provoke it against fixtures.";

/// The reason that a record code has no start here.
const RECORD_FIXTURE: &str = "The library's reader refuses it for one field of a plan, and the library's tests provoke it with a twin that the reader accepts; one command line of check shows the same refusal, which this suite shows with the codes syntax, wrong_type and unknown_field.";

/// The codes that no start of this suite provokes.
const UNPROVOKED: &[Unprovoked<'_>] = &[
    Unprovoked::new(STORE_BOARD, BOARD_START),
    Unprovoked::new(CHECK_TARGET_UNKNOWN, COMPARISON),
    Unprovoked::new(CHECK_TARGET_KIND, COMPARISON),
    Unprovoked::new(CHECK_NEED_UNKNOWN, COMPARISON),
    Unprovoked::new(CHECK_NEED_OMITTED, COMPARISON),
    Unprovoked::new(CHECK_UNBOUND_DIFFERS, COMPARISON),
    Unprovoked::new(CHECK_SECRET_FALLBACK, COMPARISON),
    Unprovoked::new(CHECK_NEED_KIND, COMPARISON),
    Unprovoked::new(CHECK_NEED_CONSUMERS, COMPARISON),
    Unprovoked::new(CHECK_OFFER_UNKNOWN, COMPARISON),
    Unprovoked::new(CHECK_OFFER_REFERENCE, COMPARISON),
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
    Unprovoked::new(STORE_CORRUPT, NEEDS_FIXTURE),
    Unprovoked::new(STORE_MISPLACED, NEEDS_FIXTURE),
    Unprovoked::new(STORE_BOARD_FILE, NEEDS_FIXTURE),
    Unprovoked::new(NEED_AMBIGUOUS, AMBIGUOUS),
    Unprovoked::new(NEED_KEPT, KEPT),
    Unprovoked::new(RECORD_DUPLICATE, RECORD_FIXTURE),
    Unprovoked::new(RECORD_EMPTY, RECORD_FIXTURE),
    Unprovoked::new(RECORD_FALLBACK, RECORD_FIXTURE),
    Unprovoked::new(RECORD_FILE_PATH, RECORD_FIXTURE),
    Unprovoked::new(RECORD_KEYWORD, RECORD_FIXTURE),
    Unprovoked::new(RECORD_MISSING_FIELD, RECORD_FIXTURE),
    Unprovoked::new(RECORD_NAME, RECORD_FIXTURE),
    Unprovoked::new(RECORD_OPTIONAL_CONFLICT, RECORD_FIXTURE),
    Unprovoked::new(RECORD_RECORD_PATH, RECORD_FIXTURE),
    Unprovoked::new(RECORD_REFERENCE_MISMATCH, RECORD_FIXTURE),
    Unprovoked::new(RECORD_REPOSITORY, RECORD_FIXTURE),
    Unprovoked::new(RECORD_SECRET_REFERENCE, RECORD_FIXTURE),
    Unprovoked::new(RECORD_SHAPE, RECORD_FIXTURE),
    Unprovoked::new(RECORD_TOO_LARGE, RECORD_FIXTURE),
    Unprovoked::new(RECORD_VARIABLE, RECORD_FIXTURE),
];

/// The reason that a code of the board store has no start here.
const BOARD_START: &str = "It needs a board, or a missing board program, and a run of this suite must not touch the personal board; tests/commands.rs provokes it with a directory that holds no board and the library's tests provoke the store's errors.";

/// The reason that a code of the plan-to-store comparison has no start here.
const COMPARISON: &str = "It needs a plan and a store that disagree, and each start of this suite reads a fixed store; tests/commands.rs provokes it against a copy of the fixture store agree with one change, and the run on the agreeing pair is its twin.";

/// The reason that an ambiguous match has no start.
const AMBIGUOUS: &str = "Two offers meet one need only when two bindings for one need and target sit in the target's authority board, which a directory store, with one file for each need and target, cannot hold; the library's matcher tests provoke it with a store of another kind.";

/// The reason that a kept record has no start.
const KEPT: &str = "It needs a store that holds a binding of a record that no declaration gives, which a start of this suite cannot build; tests/commands.rs provokes it with a copy of the fixture store and a binding, and the run that has no binding for the stale record is its twin.";

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
