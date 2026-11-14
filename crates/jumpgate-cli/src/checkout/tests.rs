#![expect(
    clippy::disallowed_methods,
    reason = "the tests build the files of a checkout directly"
)]

use super::*;

fn path(text: &str) -> RepoFilePath {
    text.parse().unwrap()
}

#[test]
fn a_file_of_the_checkout_is_read_and_a_missing_one_is_none() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mxv.toml"), "ports = []\n").unwrap();
    let checkout = DirectoryCheckout::new(dir.path().to_path_buf());
    assert_eq!(
        checkout.read(&path("mxv.toml")).unwrap().as_deref(),
        Some("ports = []\n")
    );
    assert_eq!(checkout.read(&path("starmap.toml")).unwrap(), None);
}

#[test]
fn a_file_in_a_subdirectory_is_read() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("web")).unwrap();
    fs::write(dir.path().join("web/starmap.toml"), "x").unwrap();
    let checkout = DirectoryCheckout::new(dir.path().to_path_buf());
    assert_eq!(
        checkout.read(&path("web/starmap.toml")).unwrap().as_deref(),
        Some("x")
    );
}

#[test]
fn a_link_and_a_directory_are_refused_and_the_file_they_name_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "s3cr3t").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret"), dir.path().join("mxv.toml")).unwrap();
    fs::create_dir(dir.path().join("starmap.toml")).unwrap();
    let checkout = DirectoryCheckout::new(dir.path().to_path_buf());
    assert!(checkout.read(&path("mxv.toml")).is_err());
    assert!(checkout.read(&path("starmap.toml")).is_err());
    let direct = DirectoryCheckout::new(outside.path().to_path_buf());
    assert_eq!(
        direct.read(&path("secret")).unwrap().as_deref(),
        Some("s3cr3t")
    );
}

#[test]
fn a_file_that_is_not_utf8_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mxv.toml"), [0xff, 0xfe]).unwrap();
    let checkout = DirectoryCheckout::new(dir.path().to_path_buf());
    assert!(checkout.read(&path("mxv.toml")).is_err());
}

#[test]
fn a_file_longer_than_the_limit_is_cut_one_byte_past_it() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("mxv.toml"),
        vec![b'a'; MAX_DECLARATION_BYTES + 100],
    )
    .unwrap();
    let checkout = DirectoryCheckout::new(dir.path().to_path_buf());
    let text = checkout.read(&path("mxv.toml")).unwrap().unwrap();
    assert_eq!(text.len(), MAX_DECLARATION_BYTES + 1);
}
