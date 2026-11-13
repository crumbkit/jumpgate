use super::*;
use crate::error::Reason;

fn refused(text: &str) -> Reason {
    text.parse::<RepoId>()
        .expect_err("the id is refused")
        .reason()
}

#[test]
fn a_host_owner_and_name_id_is_accepted() {
    let repo: RepoId = "github.com/acme/shelf".parse().unwrap();
    assert_eq!(repo.as_str(), "github.com/acme/shelf");
}

#[test]
fn an_owner_and_name_with_dots_hyphens_and_underscores_are_accepted() {
    assert!(
        "git.example.org/acme-labs/shelf_api.rs"
            .parse::<RepoId>()
            .is_ok()
    );
}

#[test]
fn an_id_with_two_segments_is_refused_with_the_repository_code() {
    let error = "github.com/shelf".parse::<RepoId>().unwrap_err();
    assert_eq!(error.reason(), Reason::Repository);
    assert_eq!(error.code().to_string(), "jumpgate::record::repository");
}

#[test]
fn an_id_with_four_segments_is_refused() {
    assert_eq!(refused("gitlab.com/acme/group/shelf"), Reason::Repository);
}

#[test]
fn a_remote_url_is_refused() {
    assert_eq!(refused("https://github.com/acme/shelf"), Reason::Repository);
    assert_eq!(refused("git@github.com:acme/shelf.git"), Reason::Repository);
}

#[test]
fn a_host_without_a_dot_is_refused() {
    assert_eq!(refused("localhost/acme/shelf"), Reason::Repository);
}

#[test]
fn a_host_with_an_empty_label_is_refused() {
    assert_eq!(refused("github..com/acme/shelf"), Reason::Repository);
    assert_eq!(refused(".github.com/acme/shelf"), Reason::Repository);
}

#[test]
fn an_uppercase_letter_is_refused_so_one_repository_has_one_id() {
    assert_eq!(refused("github.com/Acme/shelf"), Reason::Repository);
    assert_eq!(refused("GitHub.com/acme/shelf"), Reason::Repository);
}

#[test]
fn an_owner_or_name_that_is_dot_dot_or_starts_with_a_dot_is_refused() {
    assert_eq!(refused("github.com/../shelf"), Reason::Repository);
    assert_eq!(refused("github.com/acme/.shelf"), Reason::Repository);
    assert_eq!(refused("github.com/acme/shelf..git"), Reason::Repository);
}

#[test]
fn an_empty_segment_is_refused() {
    assert_eq!(refused("github.com//shelf"), Reason::Repository);
    assert_eq!(refused(""), Reason::Repository);
}

#[test]
fn an_id_of_255_bytes_is_accepted_and_one_of_256_is_refused() {
    let name = "a".repeat(255 - "github.com/acme/".len());
    assert!(format!("github.com/acme/{name}").parse::<RepoId>().is_ok());
    assert_eq!(
        refused(&format!("github.com/acme/{name}a")),
        Reason::Repository
    );
}

#[test]
fn a_refused_id_is_never_quoted() {
    let message = "github.com/acme/s3cr3t value"
        .parse::<RepoId>()
        .unwrap_err()
        .to_string();
    assert!(!message.contains("s3cr3t"), "{message}");
}
