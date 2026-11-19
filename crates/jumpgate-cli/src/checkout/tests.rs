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

#[test]
fn the_root_is_the_canonical_directory() {
    let dir = tempfile::tempdir().unwrap();
    let checkout = DirectoryCheckout::new(dir.path().to_path_buf());
    let expected = fs::canonicalize(dir.path()).unwrap();
    assert_eq!(checkout.root().unwrap(), expected.to_str().unwrap());
}

#[test]
fn a_missing_directory_has_no_root() {
    let dir = tempfile::tempdir().unwrap();
    let checkout = DirectoryCheckout::new(dir.path().join("absent"));
    let error = checkout.root().unwrap_err();
    assert!(error.message().contains("cannot be resolved"), "{error}");
}

#[test]
fn a_checkout_reached_through_a_symlink_has_the_real_directory_as_its_root() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let checkout = DirectoryCheckout::new(link);
    let expected = fs::canonicalize(&real).unwrap();
    assert_eq!(checkout.root().unwrap(), expected.to_str().unwrap());
}

#[test]
fn a_directory_whose_path_is_not_utf8_text_says_so() {
    use std::os::unix::ffi::OsStrExt as _;
    let dir = tempfile::tempdir().unwrap();
    let odd = dir.path().join(std::ffi::OsStr::from_bytes(b"odd-\xff"));
    if fs::create_dir(&odd).is_err() {
        // The file system refuses a name that is not UTF-8, so there is no
        // such checkout to build here.
        return;
    }
    let error = DirectoryCheckout::new(odd).root().unwrap_err();
    assert!(error.message().contains("not UTF-8 text"), "{error}");
    assert!(!error.message().contains("not a directory"), "{error}");
}
