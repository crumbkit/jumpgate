//! End-to-end tests of the `jumpgate` binary: its help, its version, the
//! `needs` command against fixture checkouts, and the two commands that
//! later versions fill in, each of which fails with
//! `jumpgate::command::not_built` until then.

#![cfg(test)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// Runs the built `jumpgate` binary with `args` in `dir`, with an
/// environment that holds only `HOME`, so no setting of the caller's shell
/// changes the output.
fn jumpgate(dir: &Path, args: &[&str]) -> Output {
    #[expect(
        clippy::disallowed_methods,
        reason = "the test runs the built binary, as a person would"
    )]
    let output = Command::new(env!("CARGO_BIN_EXE_jumpgate"))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("HOME", dir)
        .env("XDG_STATE_HOME", dir.join("state"))
        .output()
        .expect("the binary runs");
    output
}

/// Returns a directory of its own for one test, which is removed when the
/// returned value is dropped at the end of the test.
fn scratch() -> TempDir {
    tempfile::tempdir().expect("the temporary directory is writable")
}

/// Returns every file under `root`, by its path relative to `root`, with
/// its content.
#[expect(
    clippy::disallowed_methods,
    reason = "the test reads the files under its own temporary directory to see what a command changed"
)]
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_path_buf();
                found.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    found
}

/// Returns the files under `root` that were added, changed or removed
/// between `before` and `after`.
fn changed(
    before: &BTreeMap<PathBuf, Vec<u8>>,
    after: &BTreeMap<PathBuf, Vec<u8>>,
) -> Vec<PathBuf> {
    before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn version_prints_the_package_version_and_exits_with_0() {
    let temp = scratch();
    let dir = temp.path();
    let output = jumpgate(dir, &["--version"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        format!("jumpgate {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_lists_the_needs_resolve_and_check_commands_and_exits_with_0() {
    let temp = scratch();
    let dir = temp.path();
    let output = jumpgate(dir, &["--help"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let help = text(&output.stdout);
    for command in ["needs", "resolve", "check"] {
        assert!(help.contains(command), "{help}");
    }
}

#[test]
fn each_unbuilt_command_fails_with_the_not_built_code_and_exits_with_1() {
    let temp = scratch();
    let dir = temp.path();
    for command in ["resolve", "check"] {
        let output = jumpgate(dir, &[command]);
        assert_eq!(output.status.code(), Some(1), "{command}");
        let printed = format!("{}{}", text(&output.stdout), text(&output.stderr));
        assert!(
            printed.contains("jumpgate::command::not_built"),
            "{command}: {printed}"
        );
        assert!(printed.contains(command), "{command}: {printed}");
    }
}

#[test]
fn an_unbuilt_command_with_json_prints_a_failure_document_with_the_code() {
    let temp = scratch();
    let dir = temp.path();
    let output = jumpgate(dir, &["resolve", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let printed = format!("{}{}", text(&output.stdout), text(&output.stderr));
    let document: serde_json::Value = printed
        .lines()
        .find_map(|line| serde_json::from_str(line).ok())
        .expect("a JSON document is printed");
    assert!(
        document
            .to_string()
            .contains("jumpgate::command::not_built"),
        "{document}"
    );
}

#[test]
fn an_unknown_command_is_refused_with_exit_code_2() {
    let temp = scratch();
    let dir = temp.path();
    let output = jumpgate(dir, &["deploy"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn skills_print_prints_the_embedded_skill_and_exits_with_0() {
    let temp = scratch();
    let dir = temp.path();
    let output = jumpgate(dir, &["skills", "print"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(
        text(&output.stdout).contains("name: jumpgate"),
        "{}",
        text(&output.stdout)
    );
}

#[test]
fn skills_install_writes_only_the_skill_directory_and_check_passes_until_the_copy_changes() {
    let temp = scratch();
    let dir = temp.path();
    std::fs::create_dir_all(dir.join(".claude/skills")).unwrap();
    let before = files(dir);
    let install = jumpgate(dir, &["skills", "install"]);
    assert_eq!(install.status.code(), Some(0), "{}", text(&install.stderr));
    let written = changed(&before, &files(dir));
    assert!(!written.is_empty(), "{}", text(&install.stdout));
    for path in &written {
        assert!(
            path.starts_with(".claude/skills/jumpgate"),
            "install changed a file outside the skill directory: {}",
            path.display()
        );
    }
    let check = jumpgate(dir, &["skills", "check"]);
    assert_eq!(check.status.code(), Some(0), "{}", text(&check.stderr));
    let installed = dir.join(".claude/skills/jumpgate/SKILL.md");
    #[expect(
        clippy::disallowed_methods,
        reason = "the test changes the installed copy to see the check fail"
    )]
    std::fs::write(&installed, "changed\n").unwrap();
    let changed = jumpgate(dir, &["skills", "check"]);
    assert_ne!(changed.status.code(), Some(0));
}

#[test]
fn completion_install_writes_only_the_completion_file_and_check_passes_until_it_changes() {
    let temp = scratch();
    let dir = temp.path();
    let before = files(dir);
    let install = jumpgate(dir, &["completion", "install", "--shell", "fish"]);
    assert_eq!(install.status.code(), Some(0), "{}", text(&install.stderr));
    let written = changed(&before, &files(dir));
    assert_eq!(
        written,
        [PathBuf::from(".config/fish/completions/jumpgate.fish")],
        "{}",
        text(&install.stdout)
    );
    let check = jumpgate(dir, &["completion", "check", "--shell", "fish"]);
    assert_eq!(check.status.code(), Some(0), "{}", text(&check.stderr));
    let installed = dir.join(".config/fish/completions/jumpgate.fish");
    #[expect(
        clippy::disallowed_methods,
        reason = "the test changes the installed file to see the check fail"
    )]
    std::fs::write(&installed, "changed\n").unwrap();
    let changed = jumpgate(dir, &["completion", "check", "--shell", "fish"]);
    assert_ne!(changed.status.code(), Some(0));
}

/// The directory of the fixture stack of three apps.
const STACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/stack");

/// Runs `git` with `args` in `dir`.
#[expect(
    clippy::disallowed_methods,
    reason = "the test builds a checkout with git, as a person would"
)]
fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        text(&output.stderr)
    );
}

/// Copies the fixture app `app` into a new git checkout under `root`, whose
/// remote `origin` is `remote`, and returns the checkout's directory.
#[expect(
    clippy::disallowed_methods,
    reason = "the test copies the fixture files into its own temporary directory"
)]
fn checkout(root: &Path, app: &str, remote: Option<&str>) -> PathBuf {
    let dir = root.join(app);
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(Path::new(STACK).join(app)).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), dir.join(entry.file_name())).unwrap();
    }
    git(&dir, &["init", "--quiet"]);
    if let Some(remote) = remote {
        git(&dir, &["remote", "add", "origin", remote]);
    }
    dir
}

/// The three fixture checkouts, each with the remote a person would have.
fn stack(root: &Path) -> Vec<PathBuf> {
    ["shelf", "auth", "ledger"]
        .iter()
        .map(|app| checkout(root, app, Some(&format!("git@github.com:acme/{app}.git"))))
        .collect()
}

/// The arguments `needs --repo <dir>... --store <store>`.
fn needs_args<'a>(repos: &'a [String], store: &'a str) -> Vec<&'a str> {
    let mut args = vec!["needs"];
    for repo in repos {
        args.push("--repo");
        args.push(repo);
    }
    args.push("--store");
    args.push(store);
    args
}

fn display(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

/// Everything the command printed, standard output then standard error.
fn printed(output: &Output) -> String {
    format!("{}{}", text(&output.stdout), text(&output.stderr))
}

#[test]
fn needs_writes_the_needs_offers_and_units_of_the_stack_and_exits_with_0() {
    let temp = scratch();
    let dir = temp.path();
    let repos = display(&stack(dir));
    let store = dir.join("records");
    let output = jumpgate(dir, &needs_args(&repos, &store.display().to_string()));
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let written = files(&store);
    let names: Vec<String> = written
        .keys()
        .map(|path| path.display().to_string())
        .collect();
    for expected in [
        "store.json",
        "jumpgate/needs/shelf/api-token.json",
        "jumpgate/needs/shelf/auth-api.json",
        "jumpgate/needs/ledger/shelf-http.json",
        "jumpgate/offers/app/auth/api.json",
        "jumpgate/offers/app/auth/api-route.json",
        "jumpgate/offers/secret-store/one-password/shelf-dev-api-token-credential.json",
        "jumpgate/units/shelf/web.json",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "{expected} in {names:?}"
        );
    }
    assert!(
        text(&output.stdout).contains("/jumpgate/needs/shelf/api-token"),
        "{}",
        printed(&output)
    );
}

#[test]
fn needs_with_json_prints_the_paths_it_wrote_and_no_failure() {
    let temp = scratch();
    let dir = temp.path();
    let repos = display(&stack(dir));
    let store = dir.join("records").display().to_string();
    let mut args = needs_args(&repos, &store);
    args.push("--json");
    let output = jumpgate(dir, &args);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let data = &document["data"];
    assert_eq!(data["board"], "local");
    assert!(data["needs"].as_array().unwrap().len() >= 4, "{data}");
    assert_eq!(document["failures"], serde_json::json!([]));
}

#[test]
fn needs_run_twice_writes_the_same_files() {
    let temp = scratch();
    let dir = temp.path();
    let repos = display(&stack(dir));
    let store = dir.join("records");
    let args = needs_args(&repos, &store.display().to_string())
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    assert_eq!(jumpgate(dir, &args).status.code(), Some(0));
    let first = files(&store);
    assert_eq!(jumpgate(dir, &args).status.code(), Some(0));
    assert_eq!(files(&store), first);
}

#[test]
fn needs_with_an_id_and_a_path_reads_a_checkout_that_has_no_remote() {
    let temp = scratch();
    let dir = temp.path();
    let bare = checkout(dir, "shelf", None);
    let store = dir.join("records").display().to_string();
    let plain = [bare.display().to_string()];
    let refused = jumpgate(dir, &needs_args(&plain, &store));
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        printed(&refused).contains("jumpgate::repo::git"),
        "{}",
        printed(&refused)
    );
    let named = [format!("github.com/acme/shelf={}", bare.display())];
    let accepted = jumpgate(dir, &needs_args(&named, &store));
    assert_eq!(accepted.status.code(), Some(0), "{}", printed(&accepted));
}

#[test]
fn needs_refuses_an_origin_that_is_not_a_repository_url() {
    let temp = scratch();
    let dir = temp.path();
    let local = checkout(dir, "shelf", Some("/srv/git/shelf.git"));
    let store = dir.join("records").display().to_string();
    let output = jumpgate(dir, &needs_args(&[local.display().to_string()], &store));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        printed(&output).contains("jumpgate::repo::remote"),
        "{}",
        printed(&output)
    );
    let named = [format!("github.com/acme/shelf={}", local.display())];
    assert_eq!(
        jumpgate(dir, &needs_args(&named, &store)).status.code(),
        Some(0)
    );
}

#[test]
fn needs_refuses_a_repo_that_is_not_a_directory_and_one_named_twice() {
    let temp = scratch();
    let dir = temp.path();
    let store = dir.join("records").display().to_string();
    let missing = jumpgate(
        dir,
        &needs_args(&[dir.join("nowhere").display().to_string()], &store),
    );
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        printed(&missing).contains("jumpgate::repo::path"),
        "{}",
        printed(&missing)
    );
    let shelf = checkout(dir, "shelf", Some("https://github.com/acme/shelf.git"));
    let twice = [
        shelf.display().to_string(),
        format!("github.com/acme/shelf={}", shelf.display()),
    ];
    let output = jumpgate(dir, &needs_args(&twice, &store));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        printed(&output).contains("jumpgate::repo::duplicate"),
        "{}",
        printed(&output)
    );
    let once = [shelf.display().to_string()];
    assert_eq!(
        jumpgate(dir, &needs_args(&once, &store)).status.code(),
        Some(0)
    );
}

#[test]
fn needs_without_a_store_fails_and_with_one_succeeds() {
    let temp = scratch();
    let dir = temp.path();
    let shelf = checkout(dir, "shelf", Some("https://github.com/acme/shelf.git"));
    let repo = shelf.display().to_string();
    let without = jumpgate(dir, &["needs", "--repo", &repo]);
    assert_eq!(without.status.code(), Some(1));
    assert!(
        printed(&without).contains("jumpgate::command::store_required"),
        "{}",
        printed(&without)
    );
    assert!(files(dir).keys().all(|path| !path.starts_with("jumpgate")));
    let store = dir.join("records").display().to_string();
    let with = jumpgate(dir, &["needs", "--repo", &repo, "--store", &store]);
    assert_eq!(with.status.code(), Some(0), "{}", printed(&with));
}

#[test]
fn needs_without_a_repo_is_refused_with_exit_code_2() {
    let temp = scratch();
    let dir = temp.path();
    let output = jumpgate(dir, &["needs", "--store", "records"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the test changes the declaration it copied into its own temporary directory"
)]
fn needs_reports_a_refused_declaration_writes_the_rest_and_exits_with_1() {
    let temp = scratch();
    let dir = temp.path();
    let repos = stack(dir);
    let shelf_mxv = dir.join("shelf/mxv.toml");
    let broken = std::fs::read_to_string(&shelf_mxv)
        .unwrap()
        .replace("ports = [\"http\", \"db\"]", "ports = [\"HTTP\"]");
    assert_ne!(broken, std::fs::read_to_string(&shelf_mxv).unwrap());
    std::fs::write(&shelf_mxv, broken).unwrap();
    let store = dir.join("records");
    let output = jumpgate(
        dir,
        &needs_args(&display(&repos), &store.display().to_string()),
    );
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    let message = printed(&output);
    assert!(message.contains("jumpgate::declaration::name"), "{message}");
    assert!(
        message.contains("github.com/acme/shelf:mxv.toml:1"),
        "{message}"
    );
    assert!(
        store.join("jumpgate/offers/app/auth/api.json").is_file(),
        "the other repositories are still written"
    );
}

#[test]
fn needs_with_a_board_file_that_is_not_a_board_id_fails_and_a_valid_one_is_kept() {
    let temp = scratch();
    let dir = temp.path();
    let shelf = checkout(dir, "shelf", Some("https://github.com/acme/shelf.git"));
    let repo = [shelf.display().to_string()];
    let store = dir.join("records");
    std::fs::create_dir_all(&store).unwrap();
    #[expect(
        clippy::disallowed_methods,
        reason = "the test writes the store's board file"
    )]
    std::fs::write(store.join("store.json"), "{\"board\": \"Personal\"}").unwrap();
    let output = jumpgate(dir, &needs_args(&repo, &store.display().to_string()));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        printed(&output).contains("jumpgate::store::board_file"),
        "{}",
        printed(&output)
    );
    #[expect(
        clippy::disallowed_methods,
        reason = "the test writes the store's board file"
    )]
    std::fs::write(store.join("store.json"), "{\"board\": \"personal\"}").unwrap();
    let ok = jumpgate(dir, &needs_args(&repo, &store.display().to_string()));
    assert_eq!(ok.status.code(), Some(0), "{}", printed(&ok));
    assert!(printed(&ok).contains("board personal"), "{}", printed(&ok));
}
