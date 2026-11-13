use super::*;

#[test]
fn a_bare_path_has_no_id() {
    let arg: RepoArg = "../shelf".parse().unwrap();
    assert_eq!(arg.id(), None);
    assert_eq!(arg.path(), std::path::Path::new("../shelf"));
}

#[test]
fn an_id_and_a_path_are_split_at_the_first_equals_sign() {
    let arg: RepoArg = "github.com/acme/shelf=/work/shelf".parse().unwrap();
    assert_eq!(arg.id().map(RepoId::as_str), Some("github.com/acme/shelf"));
    assert_eq!(arg.path(), std::path::Path::new("/work/shelf"));
}

#[test]
fn a_path_that_holds_an_equals_sign_is_a_path() {
    let arg: RepoArg = "/work/a=b".parse().unwrap();
    assert_eq!(arg.id(), None);
    assert_eq!(arg.path(), std::path::Path::new("/work/a=b"));
}

#[test]
fn an_empty_value_or_an_empty_path_is_refused() {
    assert!("".parse::<RepoArg>().is_err());
    assert!("github.com/acme/shelf=".parse::<RepoArg>().is_err());
}
