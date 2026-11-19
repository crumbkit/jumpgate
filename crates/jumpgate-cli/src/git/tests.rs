#![expect(
    clippy::disallowed_methods,
    reason = "the tests write the scripts that stand in for git"
)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::*;

/// Writes an executable shell script with `body` and returns its path.
fn script(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("git");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

const LONG: Duration = Duration::from_secs(10);

#[test]
fn the_url_git_prints_is_returned_trimmed() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "echo https://github.com/acme/shelf.git");
    assert_eq!(
        run(git.as_os_str(), dir.path(), LONG).as_deref(),
        Ok("https://github.com/acme/shelf.git")
    );
}

#[test]
fn a_git_that_outlasts_its_deadline_is_timed_out() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "sleep 30");
    assert_eq!(
        run(git.as_os_str(), dir.path(), Duration::from_millis(200)),
        Err(GitError::TimedOut)
    );
}

#[test]
fn a_git_that_prints_more_than_a_url_is_an_output_error() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "head -c 100000 /dev/zero | tr '\\0' 'a'");
    assert_eq!(
        run(git.as_os_str(), dir.path(), LONG),
        Err(GitError::Output)
    );
}

#[test]
fn a_git_that_prints_text_that_is_not_utf8_is_an_output_error() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "printf '\\377\\376\\n'");
    assert_eq!(
        run(git.as_os_str(), dir.path(), LONG),
        Err(GitError::Output)
    );
}

#[test]
fn a_git_that_fails_is_a_failure_and_a_missing_git_does_not_start() {
    let dir = tempfile::tempdir().unwrap();
    let git = script(dir.path(), "exit 128");
    assert_eq!(
        run(git.as_os_str(), dir.path(), LONG),
        Err(GitError::Failed)
    );
    let missing = dir.path().join("none");
    assert_eq!(
        run(missing.as_os_str(), dir.path(), LONG),
        Err(GitError::Start)
    );
}

fn init_repo(dir: &Path) {
    for args in [
        vec!["init", "--quiet"],
        vec!["config", "user.name", "t"],
        vec!["config", "user.email", "t@example.com"],
    ] {
        let status = std::process::Command::new("git")
            .args(&args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .status()
            .unwrap();
        assert!(status.success());
    }
}

fn track(dir: &Path, files: &[&str]) {
    for file in files {
        let path = dir.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "x").unwrap();
    }
    let status = std::process::Command::new("git")
        .args(["add", "--all"])
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn the_tracked_starmap_files_are_listed_at_every_depth_and_untracked_ones_are_not() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    track(
        dir.path(),
        &[
            "starmap.toml",
            "cli/starmap.toml",
            "a/b/starmap.toml",
            "a/readme.md",
        ],
    );
    fs::create_dir_all(dir.path().join("loose")).unwrap();
    fs::write(dir.path().join("loose/starmap.toml"), "x").unwrap();
    let mut found = tracked_starmaps(dir.path()).unwrap();
    found.sort();
    assert_eq!(
        found,
        ["a/b/starmap.toml", "cli/starmap.toml", "starmap.toml"]
    );
}

#[test]
fn a_checkout_below_the_top_of_a_larger_repository_lists_paths_relative_to_itself() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    track(
        dir.path(),
        &[
            "starmap.toml",
            "app/starmap.toml",
            "app/cli/starmap.toml",
            "other/starmap.toml",
        ],
    );
    let mut found = tracked_starmaps(&dir.path().join("app")).unwrap();
    found.sort();
    assert_eq!(found, ["cli/starmap.toml", "starmap.toml"]);
}

#[test]
fn a_git_that_fails_times_out_does_not_start_or_prints_too_much_is_an_error_when_listing() {
    let dir = tempfile::tempdir().unwrap();
    let failing = script(dir.path(), "echo fatal >&2; exit 128");
    assert_eq!(
        list_starmaps(failing.as_os_str(), dir.path(), 1024, LONG),
        Err(GitError::Failed)
    );
    let slow = script(dir.path(), "sleep 30");
    assert_eq!(
        list_starmaps(
            slow.as_os_str(),
            dir.path(),
            1024,
            Duration::from_millis(200)
        ),
        Err(GitError::TimedOut)
    );
    let missing = dir.path().join("none");
    assert_eq!(
        list_starmaps(missing.as_os_str(), dir.path(), 1024, LONG),
        Err(GitError::Start)
    );
    let long = script(dir.path(), "head -c 5000 /dev/zero | tr '\\0' 'a'");
    assert_eq!(
        list_starmaps(long.as_os_str(), dir.path(), 1024, LONG),
        Err(GitError::Output)
    );
}

#[test]
fn the_git_listing_of_a_copy_of_the_beacon_fixture_names_every_starmap_file_on_disk() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/stack/beacon");
    let dir = tempfile::tempdir().unwrap();
    let mut on_disk = Vec::new();
    copy_starmaps(&source, &source, dir.path(), &mut on_disk);
    init_repo(dir.path());
    let status = std::process::Command::new("git")
        .args(["add", "--all"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    let mut listed = tracked_starmaps(dir.path()).unwrap();
    listed.sort();
    on_disk.sort();
    assert!(!on_disk.is_empty());
    assert_eq!(listed, on_disk);
}

fn copy_starmaps(root: &Path, from: &Path, to: &Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            copy_starmaps(root, &path, to, found);
        } else if path.file_name().is_some_and(|name| name == "starmap.toml") {
            let relative = path.strip_prefix(root).unwrap();
            let target = to.join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(&path, &target).unwrap();
            found.push(relative.display().to_string());
        }
    }
}
