use super::*;

fn id(url: &str) -> Option<String> {
    repo_id(url).map(|id| id.as_str().to_owned())
}

#[test]
fn each_form_of_a_remote_names_the_same_repository() {
    for url in [
        "https://github.com/acme/shelf.git",
        "https://github.com/acme/shelf",
        "https://github.com/acme/shelf/",
        "HTTPS://GitHub.com/Acme/Shelf.git",
        "https://user:token@github.com/acme/shelf.git",
        "ssh://git@github.com/acme/shelf.git",
        "ssh://git@github.com:22/acme/shelf.git",
        "git@github.com:acme/shelf.git",
        "git@github.com:acme/shelf",
        "git://github.com/acme/shelf.git",
    ] {
        assert_eq!(id(url).as_deref(), Some("github.com/acme/shelf"), "{url}");
    }
}

#[test]
fn a_remote_that_is_not_host_owner_name_names_no_repository() {
    for url in [
        "",
        "/tmp/shelf",
        "file:///tmp/shelf.git",
        "../shelf",
        "https://github.com/shelf.git",
        "https://github.com/a/b/c.git",
        "git@github.com:shelf",
        "https://localhost/acme/shelf.git",
    ] {
        assert_eq!(id(url), None, "{url}");
    }
}

#[test]
fn a_token_in_a_remote_is_not_part_of_the_id() {
    let id = id("https://x-access-token:s3cr3t@github.com/acme/shelf.git").unwrap();
    assert!(!id.contains("s3cr3t"));
}
