use super::*;

fn report() -> Report {
    Report {
        store: "records".to_owned(),
        board: "local".to_owned(),
        needs: vec!["/jumpgate/needs/shelf/api-token".to_owned()],
        offers: vec![],
        units: vec!["/jumpgate/units/shelf/web".to_owned()],
        unread: vec!["github.com/acme/shelf:starmap.toml".to_owned()],
    }
}

#[test]
fn the_report_prints_a_count_line_then_each_group_that_has_records() {
    let text = report().to_string();
    assert_eq!(
        text,
        "Wrote 1 needs, 0 offers and 1 units to records (board local).\nNeeds:\n  /jumpgate/needs/shelf/api-token\nUnits:\n  /jumpgate/units/shelf/web\nNo process reads:\n  github.com/acme/shelf:starmap.toml\n"
    );
}

#[test]
fn every_failure_has_a_declared_code_and_a_next_step() {
    let failures = [
        NeedsFailure::StoreRequired,
        NeedsFailure::RepoPath {
            path: "x".to_owned(),
        },
        NeedsFailure::Git {
            path: "x".to_owned(),
            error: GitError::Failed,
        },
        NeedsFailure::Remote {
            path: "x".to_owned(),
        },
        NeedsFailure::Duplicate {
            id: "github.com/acme/x".to_owned(),
        },
    ];
    for failure in &failures {
        assert!(codes::ALL.contains(&failure.code()), "{failure}");
        assert!(matches!(failure.recovery(), Recovery::Hint(_)));
    }
}

#[test]
fn a_git_failure_names_the_checkout_and_what_happened() {
    for (error, words) in [
        (GitError::Start, "did not start"),
        (GitError::Failed, "remote named origin"),
        (GitError::TimedOut, "in time"),
        (GitError::Output, "other than a URL"),
    ] {
        let failure = NeedsFailure::Git {
            path: "../shelf".to_owned(),
            error,
        };
        let message = failure.to_string();
        assert!(
            message.contains("../shelf") && message.contains(words),
            "{message}"
        );
    }
}
