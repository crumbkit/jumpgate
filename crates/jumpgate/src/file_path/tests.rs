use super::*;
use crate::error::Reason;

fn refused_in_repo(text: &str) -> Reason {
    text.parse::<RepoFilePath>()
        .expect_err("the path is refused")
        .reason()
}

fn refused_delivery(text: &str) -> Reason {
    text.parse::<DeliveryPath>()
        .expect_err("the path is refused")
        .reason()
}

#[test]
fn a_relative_path_within_a_repository_is_accepted() {
    let path: RepoFilePath = "services/api/starmap.toml".parse().unwrap();
    assert_eq!(path.as_str(), "services/api/starmap.toml");
    assert!("mxv.toml".parse::<RepoFilePath>().is_ok());
}

#[test]
fn a_repository_path_with_spaces_and_dots_inside_a_segment_is_accepted() {
    assert!("my config/.starmap.toml".parse::<RepoFilePath>().is_ok());
}

#[test]
fn an_absolute_repository_path_is_refused_with_the_file_path_code() {
    let error = "/etc/starmap.toml".parse::<RepoFilePath>().unwrap_err();
    assert_eq!(error.reason(), Reason::FilePath);
    assert_eq!(error.code().to_string(), "jumpgate::record::file_path");
}

#[test]
fn a_repository_path_with_a_dot_dot_segment_is_refused() {
    assert_eq!(refused_in_repo("../starmap.toml"), Reason::FilePath);
    assert_eq!(
        refused_in_repo("services/../../starmap.toml"),
        Reason::FilePath
    );
}

#[test]
fn a_repository_path_with_a_dot_segment_is_refused() {
    assert_eq!(refused_in_repo("./starmap.toml"), Reason::FilePath);
}

#[test]
fn a_repository_path_with_an_empty_segment_is_refused() {
    assert_eq!(refused_in_repo("services//starmap.toml"), Reason::FilePath);
    assert_eq!(refused_in_repo("services/"), Reason::FilePath);
    assert_eq!(refused_in_repo(""), Reason::FilePath);
}

#[test]
fn a_repository_path_with_a_backslash_or_a_control_character_is_refused() {
    assert_eq!(refused_in_repo("services\\starmap.toml"), Reason::FilePath);
    assert_eq!(refused_in_repo("services/star\nmap.toml"), Reason::FilePath);
    assert_eq!(refused_in_repo("services/star\0map.toml"), Reason::FilePath);
}

#[test]
fn a_repository_path_of_1024_bytes_is_accepted_and_one_of_1025_is_refused() {
    assert!("a".repeat(1024).parse::<RepoFilePath>().is_ok());
    assert_eq!(refused_in_repo(&"a".repeat(1025)), Reason::FilePath);
}

#[test]
fn an_absolute_delivery_path_is_accepted() {
    let path: DeliveryPath = "/run/secrets/api-token".parse().unwrap();
    assert_eq!(path.as_str(), "/run/secrets/api-token");
}

#[test]
fn a_relative_delivery_path_is_accepted() {
    assert!("config/token".parse::<DeliveryPath>().is_ok());
}

#[test]
fn a_delivery_path_that_is_only_a_slash_is_refused() {
    assert_eq!(refused_delivery("/"), Reason::FilePath);
}

#[test]
fn a_delivery_path_with_a_dot_dot_segment_is_refused() {
    assert_eq!(refused_delivery("/run/../etc/passwd"), Reason::FilePath);
}

#[test]
fn a_delivery_path_that_starts_with_two_slashes_is_refused() {
    assert_eq!(refused_delivery("//run/token"), Reason::FilePath);
}

#[test]
fn a_refused_path_is_never_quoted() {
    let message = "../s3cr3t".parse::<RepoFilePath>().unwrap_err().to_string();
    assert!(!message.contains("s3cr3t"), "{message}");
}
