//! End-to-end tests of the `jumpgate` binary: its help, its version, the
//! `needs`, `resolve` and `check` commands against fixture checkouts and
//! fixture plans.

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

/// Writes a `board` script that logs its arguments to `calls.log` in `dir`
/// and fails with a board code, and returns the directory that holds it. A
/// test that puts the directory first on `PATH` runs no real board, and the
/// personal board is never touched.
#[expect(
    clippy::disallowed_methods,
    reason = "the test writes the script that stands in for board in its own temporary directory"
)]
fn stub_board(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = bin.join("board");
    let log = dir.join("calls.log");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$@\" >> {}\necho 'board::path::not_found' >&2\nexit 1\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

/// Runs the binary as `jumpgate` does, with `bin` first on `PATH`.
fn jumpgate_with_board(dir: &Path, bin: &Path, args: &[&str]) -> Output {
    #[expect(
        clippy::disallowed_methods,
        reason = "the test runs the built binary, as a person would"
    )]
    let output = Command::new(env!("CARGO_BIN_EXE_jumpgate"))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("HOME", dir)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .expect("the binary runs");
    output
}

/// Returns the argument line of each call the stub board logged in `dir`.
#[expect(
    clippy::disallowed_methods,
    reason = "the test reads the log that its stub board wrote"
)]
fn board_calls(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("calls.log"))
        .map(|log| log.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// The text of a secret need of the app `shelf` that no declaration gives.
fn stale_need(name: &str) -> String {
    serde_json::json!({
        "$type": "jumpgate/need/v1",
        "app": "shelf",
        "name": name,
        "kind": "secret",
        "consumers": ["web"],
        "declared_in": {"repo": "github.com/acme/shelf", "file": "starmap.toml"},
        "delivers_as": {"variables": ["OLD_TOKEN"]},
        "optional": false,
        "when": [],
        "when_unbound": "fail"
    })
    .to_string()
}

/// Runs `needs` over the stack and the store `store`.
fn needs_over(dir: &Path, store: &Path) -> Output {
    let repos = display(&["shelf", "auth", "ledger"].map(|app| dir.join(app)));
    jumpgate(dir, &needs_args(&repos, &store.display().to_string()))
}

#[test]
fn needs_keeps_a_stale_need_that_a_binding_names_and_fails_naming_the_binding() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    put(
        &store,
        "jumpgate/needs/shelf/old-token.json",
        &stale_need("old-token"),
    );
    put(
        &store,
        "jumpgate/bindings/local/shelf/old-token.json",
        &serde_json::json!({
            "$type": "jumpgate/binding/v1",
            "need": "/jumpgate/needs/shelf/old-token",
            "offer": "/jumpgate/offers/secret-store/one-password/old-token",
            "target": "local"
        })
        .to_string(),
    );
    let output = needs_over(dir, &store);
    let shown = printed(&output);
    assert_eq!(output.status.code(), Some(1), "{shown}");
    assert!(store.join("jumpgate/needs/shelf/old-token.json").is_file());
    assert!(
        shown.contains(
            "Kept, because a binding still names them:\n  /jumpgate/needs/shelf/old-token"
        ),
        "{shown}"
    );
    assert!(shown.contains("jumpgate::need::kept"), "{shown}");
    assert!(
        shown.contains("/jumpgate/bindings/local/shelf/old-token"),
        "{shown}"
    );
}

#[test]
fn needs_deletes_a_stale_need_that_no_binding_names_and_exits_with_0() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    put(
        &store,
        "jumpgate/needs/shelf/old-key.json",
        &stale_need("old-key"),
    );
    let output = needs_over(dir, &store);
    let shown = printed(&output);
    assert_eq!(output.status.code(), Some(0), "{shown}");
    assert!(!store.join("jumpgate/needs/shelf/old-key.json").exists());
    assert!(
        shown.contains(
            "Removed, because no declaration gives them now:\n  /jumpgate/needs/shelf/old-key"
        ),
        "{shown}"
    );
    assert!(!shown.contains("Kept"), "{shown}");
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
        "jumpgate/offers/app/auth/site.json",
        "jumpgate/offers/secret-store/one-password/shelf-dev-api-token-credential-76e290dd.json",
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
fn needs_with_a_store_directory_succeeds_and_runs_no_board() {
    let temp = scratch();
    let dir = temp.path();
    let shelf = checkout(dir, "shelf", Some("https://github.com/acme/shelf.git"));
    let repo = shelf.display().to_string();
    let bin = stub_board(dir);
    let store = dir.join("records").display().to_string();
    let with = jumpgate_with_board(dir, &bin, &["needs", "--repo", &repo, "--store", &store]);
    assert_eq!(with.status.code(), Some(0), "{}", printed(&with));
    assert_eq!(board_calls(dir), Vec::<String>::new());
}

#[test]
fn needs_without_a_store_option_uses_the_personal_board() {
    let temp = scratch();
    let dir = temp.path();
    let shelf = checkout(dir, "shelf", Some("https://github.com/acme/shelf.git"));
    let repo = shelf.display().to_string();
    let bin = stub_board(dir);
    let output = jumpgate_with_board(dir, &bin, &["needs", "--repo", &repo]);
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    assert!(
        printed(&output).contains("jumpgate::store::board"),
        "{}",
        printed(&output)
    );
    let calls = board_calls(dir);
    assert!(!calls.is_empty());
    assert!(
        calls
            .iter()
            .all(|call| call.starts_with("--personal --no-input ")),
        "{calls:?}"
    );
}

#[test]
fn a_board_option_runs_board_in_that_directory() {
    let temp = scratch();
    let dir = temp.path();
    let shelf = checkout(dir, "shelf", Some("https://github.com/acme/shelf.git"));
    let repo = shelf.display().to_string();
    let bin = stub_board(dir);
    let output = jumpgate_with_board(
        dir,
        &bin,
        &["needs", "--repo", &repo, "--board", "team-notes"],
    );
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    let calls = board_calls(dir);
    assert!(
        calls
            .iter()
            .all(|call| call.starts_with("-C team-notes --no-input ")),
        "{calls:?}"
    );
}

#[test]
fn a_store_directory_and_a_board_directory_together_are_refused_with_exit_code_2() {
    let temp = scratch();
    let dir = temp.path();
    let output = jumpgate(
        dir,
        &[
            "resolve", "--target", "local", "--store", "a", "--board", "b",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
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

#[test]
fn needs_refuses_the_second_of_two_repositories_that_make_one_app_and_exits_with_1() {
    let temp = scratch();
    let dir = temp.path();
    let (one, two) = (dir.join("one"), dir.join("two"));
    let first = checkout(&one, "auth", Some("https://github.com/acme/auth.git"));
    let second = checkout(&two, "auth", Some("https://github.com/evil/auth.git"));
    let store = dir.join("records").display().to_string();
    let both = display(&[first.clone(), second]);
    let output = jumpgate(dir, &needs_args(&both, &store));
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    let message = printed(&output);
    assert!(
        message.contains("jumpgate::declaration::clash"),
        "{message}"
    );
    assert!(message.contains("github.com/evil/auth"), "{message}");
    let only = display(&[first]);
    let output = jumpgate(dir, &needs_args(&only, &store));
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
}

/// Writes the file `relative` under `store`, with its directories.
#[expect(
    clippy::disallowed_methods,
    reason = "the test writes records into its own store directory, as a person or another tool would"
)]
fn put(store: &Path, relative: &str, content: &str) {
    let file = store.join(relative);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, content).unwrap();
}

/// Writes the target record `name` of kind `kind` into `store`.
fn put_target(store: &Path, name: &str, kind: &str) {
    let record = serde_json::json!({
        "$type": "jumpgate/target/v1",
        "name": name,
        "kind": kind,
        "authority": "local",
    });
    put(
        store,
        &format!("jumpgate/targets/{name}.json"),
        &record.to_string(),
    );
}

/// Reads the stack into a store in `dir` with `needs`, adds a `local` and
/// a `cloud` target, and returns the store's directory.
fn stocked(dir: &Path) -> PathBuf {
    let repos = display(&stack(dir));
    let store = dir.join("records");
    let output = jumpgate(dir, &needs_args(&repos, &store.display().to_string()));
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    put_target(&store, "local", "local");
    put_target(&store, "cloud", "cloud");
    store
}

/// Runs `resolve` for `target` over `store` with `extra` arguments.
fn resolve(dir: &Path, store: &Path, target: &str, extra: &[&str]) -> Output {
    let store = store.display().to_string();
    let mut args = vec!["resolve", "--target", target, "--store", &store];
    args.extend_from_slice(extra);
    jumpgate(dir, &args)
}

#[expect(
    clippy::disallowed_methods,
    reason = "the test reads the plan file that the command wrote"
)]
fn read_text(file: &Path) -> String {
    String::from_utf8(std::fs::read(file).unwrap()).unwrap()
}

fn read_plan(file: &Path) -> serde_json::Value {
    serde_json::from_str(&read_text(file)).unwrap()
}

#[test]
fn resolve_writes_the_plan_of_the_stack_to_the_out_file_and_exits_with_0() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let out = dir.join("plans/local.json");
    let output = resolve(dir, &store, "local", &["--out", &out.display().to_string()]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let plan = read_plan(&out);
    assert_eq!(plan["$type"], "jumpgate/plan/v1alpha1");
    assert_eq!(plan["target"]["name"], "local");
    assert_eq!(plan["adapter"], serde_json::json!({}));
    let units: Vec<&str> = plan["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|unit| unit["name"].as_str().unwrap())
        .collect();
    assert_eq!(units, ["server", "store", "worker", "db", "web"]);
    let web = plan["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["name"] == "web")
        .unwrap();
    let auth = web["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|binding| binding["need"] == "/jumpgate/needs/shelf/auth-api")
        .unwrap();
    assert_eq!(auth["offer"], "/jumpgate/offers/app/auth/api");
    assert_eq!(auth["via"], "want");
    let text = read_text(&out);
    assert!(
        text.contains("op://shelf-dev/api-token/credential"),
        "{text}"
    );
    assert!(text.ends_with("}\n"), "{text}");
    let unbound = text.find("\"unbound\"").unwrap();
    assert!(unbound < text.find("\"units\"").unwrap(), "{text}");
    let message = text_of(&output);
    assert!(message.contains("Wrote the plan to"), "{message}");
    assert!(
        message.contains("/jumpgate/needs/ledger/billing-api"),
        "{message}"
    );
}

/// Everything the command printed to standard output.
fn text_of(output: &Output) -> String {
    text(&output.stdout)
}

#[test]
fn resolve_without_out_prints_the_plan_alone_and_matches_the_file_it_would_write() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let printed_plan = resolve(dir, &store, "local", &[]);
    assert_eq!(
        printed_plan.status.code(),
        Some(0),
        "{}",
        printed(&printed_plan)
    );
    let out = dir.join("plan.json");
    let written = resolve(dir, &store, "local", &["--out", &out.display().to_string()]);
    assert_eq!(written.status.code(), Some(0));
    let from_stdout: serde_json::Value = serde_json::from_slice(&printed_plan.stdout).unwrap();
    assert_eq!(from_stdout, read_plan(&out));
}

#[test]
fn resolve_with_json_prints_the_plan_and_the_notes_in_the_data() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let output = resolve(dir, &store, "local", &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["data"]["target"], "local");
    assert_eq!(document["data"]["plan"]["$type"], "jumpgate/plan/v1alpha1");
    assert!(document["data"]["notes"].is_array(), "{document}");
    assert_eq!(document["failures"], serde_json::json!([]));
}

#[test]
fn resolve_run_twice_writes_the_same_file_and_leaves_no_temporary_file() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let out = dir.join("plans/local.json");
    let arg = out.display().to_string();
    assert_eq!(
        resolve(dir, &store, "local", &["--out", &arg])
            .status
            .code(),
        Some(0)
    );
    let first = files(&dir.join("plans"));
    assert_eq!(
        resolve(dir, &store, "local", &["--out", &arg])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(files(&dir.join("plans")), first);
    assert_eq!(first.len(), 1);
}

#[test]
fn resolve_for_a_cloud_target_gives_the_same_bindings_and_names_the_kind() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let local = resolve(dir, &store, "local", &["--json"]);
    let cloud = resolve(dir, &store, "cloud", &["--json"]);
    assert_eq!(cloud.status.code(), Some(0), "{}", printed(&cloud));
    let local: serde_json::Value = serde_json::from_slice(&local.stdout).unwrap();
    let cloud: serde_json::Value = serde_json::from_slice(&cloud.stdout).unwrap();
    assert_eq!(cloud["data"]["kind"], "cloud");
    assert_eq!(
        cloud["data"]["plan"]["units"],
        local["data"]["plan"]["units"]
    );
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the test changes the declaration it copied into its own temporary directory"
)]
fn resolve_lists_a_required_unbound_need_first_writes_the_plan_and_exits_with_1() {
    let temp = scratch();
    let dir = temp.path();
    let repos = stack(dir);
    let ledger = dir.join("ledger/mxv.toml");
    let text = std::fs::read_to_string(&ledger).unwrap();
    let required = text.replace("optional = true\n", "");
    assert_ne!(text, required);
    std::fs::write(&ledger, required).unwrap();
    let store = dir.join("records");
    let needs = jumpgate(
        dir,
        &needs_args(&display(&repos), &store.display().to_string()),
    );
    assert_eq!(needs.status.code(), Some(0), "{}", printed(&needs));
    put_target(&store, "local", "local");
    let out = dir.join("plan.json");
    let output = resolve(dir, &store, "local", &["--out", &out.display().to_string()]);
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    let message = printed(&output);
    assert!(message.contains("jumpgate::need::unbound"), "{message}");
    let blocking = message.find("Unbound, blocking").unwrap();
    assert!(blocking < message.find("Bound ").unwrap(), "{message}");
    let plan = read_plan(&out);
    assert_eq!(
        plan["unbound"][0]["need"],
        "/jumpgate/needs/ledger/billing-api"
    );
    assert_eq!(plan["unbound"][0]["when_unbound"], "fail");
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the test changes the declaration it copied into its own temporary directory"
)]
fn resolve_binds_a_want_whichever_run_of_needs_read_the_provider() {
    let temp = scratch();
    let dir = temp.path();
    let repos = display(&stack(dir));
    let shelf = dir.join("shelf/mxv.toml");
    let text = std::fs::read_to_string(&shelf).unwrap();
    let wants_a_unit = text.replace("provides = \"api\"", "provides = \"server\"");
    assert_ne!(text, wants_a_unit);
    std::fs::write(&shelf, wants_a_unit).unwrap();
    let store = dir.join("records");
    let arg = store.display().to_string();
    for repo in [&repos[0], &repos[1]] {
        let output = jumpgate(dir, &needs_args(std::slice::from_ref(repo), &arg));
        assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    }
    put_target(&store, "local", "local");
    let output = resolve(dir, &store, "local", &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        document["data"]["plan"]
            .to_string()
            .contains("/jumpgate/offers/app/auth/server"),
        "{document}"
    );
}

#[test]
fn resolve_fails_when_a_binding_names_an_offer_the_store_lacks() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let binding = serde_json::json!({
        "$type": "jumpgate/binding/v1",
        "need": "/jumpgate/needs/shelf/auth-api",
        "offer": "/jumpgate/offers/app/auth/gone",
        "target": "local",
    });
    put(
        &store,
        "jumpgate/bindings/local/shelf/auth-api.json",
        &binding.to_string(),
    );
    let output = resolve(dir, &store, "local", &[]);
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    assert!(
        printed(&output).contains("jumpgate::need::offer_missing"),
        "{}",
        printed(&output)
    );
}

#[test]
fn resolve_ignores_a_binding_from_a_board_that_is_not_the_authority() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let binding = serde_json::json!({
        "$type": "jumpgate/binding/v1",
        "need": "/jumpgate/needs/shelf/auth-api",
        "offer": "/jumpgate/offers/app/auth/site",
        "target": "local",
    });
    put(
        &store,
        "boards/team/jumpgate/bindings/local/shelf/auth-api.json",
        &binding.to_string(),
    );
    let output = resolve(dir, &store, "local", &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let notes = document["data"]["notes"].to_string();
    assert!(notes.contains("board team"), "{notes}");
    assert!(
        document["data"]["plan"]
            .to_string()
            .contains("/jumpgate/offers/app/auth/api"),
        "{document}"
    );
}

#[test]
fn resolve_refuses_a_target_the_store_lacks_and_a_missing_store_and_a_missing_target_name() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let missing = resolve(dir, &store, "staging", &[]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        printed(&missing).contains("jumpgate::target::missing"),
        "{}",
        printed(&missing)
    );
    let nameless = jumpgate(dir, &["resolve", "--store", "records"]);
    assert_eq!(nameless.status.code(), Some(2));
    assert!(resolve(dir, &store, "local", &[]).status.success());
}

#[test]
fn resolve_prints_a_fallback_value_for_a_local_target_and_never_for_a_cloud_target() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let marker = "s3cr3t-marker-value";
    let need = serde_json::json!({
        "$type": "jumpgate/need/v1",
        "app": "shelf",
        "name": "dev-key",
        "kind": "secret",
        "consumers": ["web"],
        "declared_in": {"repo": "github.com/acme/shelf", "file": "starmap.toml"},
        "delivers_as": {"variables": ["DEV_KEY"]},
        "optional": true,
        "when": [],
        "when_unbound": {"fallback": marker}
    });
    put(
        &store,
        "jumpgate/needs/shelf/dev-key.json",
        &need.to_string(),
    );
    let local = resolve(dir, &store, "local", &[]);
    assert_eq!(local.status.code(), Some(0), "{}", printed(&local));
    let shown = printed(&local);
    assert!(shown.contains(marker), "{shown}");
    assert!(
        shown.contains("op://shelf-dev/api-token/credential"),
        "{shown}"
    );
    let out = dir.join("cloud-plan.json");
    let cloud = resolve(dir, &store, "cloud", &["--out", &out.display().to_string()]);
    assert_eq!(cloud.status.code(), Some(1), "{}", printed(&cloud));
    let shown = printed(&cloud);
    assert!(
        shown.contains("jumpgate::need::fallback_on_cloud"),
        "{shown}"
    );
    assert!(!shown.contains(marker), "{shown}");
    assert!(!read_text(&out).contains(marker));
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the test changes the declaration it copied into its own temporary directory"
)]
fn needs_removes_the_records_of_a_want_that_the_app_no_longer_declares() {
    let temp = scratch();
    let dir = temp.path();
    let repos = display(&stack(dir));
    let store = dir.join("records");
    let arg = store.display().to_string();
    assert_eq!(
        jumpgate(dir, &needs_args(&repos, &arg)).status.code(),
        Some(0)
    );
    let need = store.join("jumpgate/needs/shelf/auth-api.json");
    assert!(need.is_file());
    let shelf = dir.join("shelf/mxv.toml");
    let text = std::fs::read_to_string(&shelf).unwrap();
    let without = text.split("[wants.auth-api]").next().unwrap();
    std::fs::write(&shelf, without).unwrap();
    let output = jumpgate(dir, &needs_args(&repos, &arg));
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    assert!(!need.exists());
    assert!(
        printed(&output).contains("/jumpgate/needs/shelf/auth-api"),
        "{}",
        printed(&output)
    );
    assert!(
        store
            .join("jumpgate/needs/ledger/shelf-http.json")
            .is_file()
    );
    assert!(store.join("jumpgate/offers/app/auth/api.json").is_file());
}

#[test]
fn needs_leaves_the_records_of_an_app_it_did_not_read() {
    let temp = scratch();
    let dir = temp.path();
    let repos = display(&stack(dir));
    let store = dir.join("records");
    let arg = store.display().to_string();
    assert_eq!(
        jumpgate(dir, &needs_args(&repos, &arg)).status.code(),
        Some(0)
    );
    let before = files(&store);
    let only = &repos[1..2];
    assert_eq!(
        jumpgate(dir, &needs_args(only, &arg)).status.code(),
        Some(0)
    );
    assert_eq!(files(&store), before);
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the test changes the declaration it copied into its own temporary directory"
)]
fn needs_keeps_every_record_of_an_app_whose_declaration_is_refused() {
    let temp = scratch();
    let dir = temp.path();
    let repos = display(&stack(dir));
    let store = dir.join("records");
    let arg = store.display().to_string();
    assert_eq!(
        jumpgate(dir, &needs_args(&repos, &arg)).status.code(),
        Some(0)
    );
    let before = files(&store);
    let shelf = dir.join("shelf/mxv.toml");
    let text = std::fs::read_to_string(&shelf).unwrap();
    std::fs::write(
        &shelf,
        text.replace("ports = [\"http\", \"db\"]", "ports = [\"HTTP\"]"),
    )
    .unwrap();
    let output = jumpgate(dir, &needs_args(&repos, &arg));
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    // The refused app still writes the records the reader made of it, so the
    // store may gain a file, and it loses none.
    let after = files(&store);
    for (path, content) in &before {
        assert_eq!(after.get(path), Some(content), "{}", path.display());
    }
}

/// Runs the binary with `args` in `dir` with its standard output a pipe
/// whose reading end is already closed, and returns its status and what it
/// wrote to standard error.
#[expect(
    clippy::disallowed_methods,
    reason = "the test runs the built binary, as a person would"
)]
fn with_closed_stdout(dir: &Path, args: &[&str]) -> (std::process::ExitStatus, String) {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_jumpgate"))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("HOME", dir)
        .env("XDG_STATE_HOME", dir.join("state"))
        .stdout(writer)
        .output()
        .expect("the binary runs");
    (output.status, text(&output.stderr))
}

#[test]
fn resolve_and_needs_with_a_closed_standard_output_end_by_exit_code_without_a_panic() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    let store_arg = store.display().to_string();
    let repos = display(&[dir.join("shelf"), dir.join("auth"), dir.join("ledger")]);
    let needs = needs_args(&repos, &store_arg);
    let resolve_args = ["resolve", "--target", "local", "--store", &store_arg];
    for args in [needs.as_slice(), resolve_args.as_slice()] {
        let (status, stderr) = with_closed_stdout(dir, args);
        assert!(status.code().is_some(), "{args:?} was killed: {status}");
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
    }
}

/// Starts the binary with `args`, which reads a named pipe that nothing
/// writes, so it waits, sends it Ctrl-C and returns its status.
#[cfg(unix)]
#[expect(
    clippy::disallowed_methods,
    reason = "the test runs the built binary and signals it, as a person would"
)]
fn interrupted(dir: &Path, args: &[&str]) -> std::process::ExitStatus {
    let mut child = Command::new(env!("CARGO_BIN_EXE_jumpgate"))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("HOME", dir)
        .env("XDG_STATE_HOME", dir.join("state"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("the binary starts");
    std::thread::sleep(std::time::Duration::from_millis(300));
    let signal = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(signal.success());
    child.wait().expect("the binary ends")
}

/// Makes a named pipe at `path`.
#[cfg(unix)]
#[expect(
    clippy::disallowed_methods,
    reason = "the test makes a named pipe so that a read of it waits"
)]
fn fifo(path: &Path) {
    let status = Command::new("mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo runs");
    assert!(status.success());
}

#[cfg(unix)]
#[test]
fn ctrl_c_ends_resolve_and_leaves_no_plan_file_and_no_temporary_file() {
    use std::os::unix::process::ExitStatusExt as _;

    let temp = scratch();
    let dir = temp.path();
    let store = dir.join("records");
    std::fs::create_dir_all(&store).unwrap();
    fifo(&store.join("store.json"));
    let out = dir.join("plans/plan.json");
    let status = interrupted(
        dir,
        &[
            "resolve",
            "--target",
            "local",
            "--store",
            &store.display().to_string(),
            "--out",
            &out.display().to_string(),
        ],
    );
    assert_eq!(status.signal(), Some(2), "{status}");
    assert!(!dir.join("plans").exists());
}

#[cfg(unix)]
#[test]
fn ctrl_c_ends_needs_and_leaves_the_store_empty() {
    use std::os::unix::process::ExitStatusExt as _;

    let temp = scratch();
    let dir = temp.path();
    let shelf = checkout(dir, "shelf", None);
    let store = dir.join("records");
    std::fs::create_dir_all(&store).unwrap();
    fifo(&store.join("store.json"));
    let status = interrupted(
        dir,
        &[
            "needs",
            "--repo",
            &format!("github.com/acme/shelf={}", shelf.display()),
            "--store",
            &store.display().to_string(),
        ],
    );
    assert_eq!(status.signal(), Some(2), "{status}");
    #[expect(
        clippy::disallowed_methods,
        reason = "the test lists the entries of its own store directory"
    )]
    let names: Vec<String> = std::fs::read_dir(&store)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["store.json"]);
}

#[test]
fn needs_keeps_a_binding_record_and_a_target_record() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    put(
        &store,
        "jumpgate/bindings/local/shelf/auth-api.json",
        &serde_json::json!({
            "$type": "jumpgate/binding/v1",
            "need": "/jumpgate/needs/shelf/auth-api",
            "offer": "/jumpgate/offers/app/auth/api",
            "target": "local",
        })
        .to_string(),
    );
    let before = files(&store);
    let repos = display(&stack(&dir.join("again")));
    let output = jumpgate(dir, &needs_args(&repos, &store.display().to_string()));
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    assert_eq!(files(&store), before);
}

/// The directory of the fixture plans.
fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture")
}

#[test]
fn check_of_the_fixture_plan_passes_all_six_checks_and_exits_with_0() {
    let output = jumpgate(&fixture_dir(), &["check", "plan.json", "--plan-only"]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let shown = text_of(&output);
    assert!(shown.contains("0 of 6 checks failed"), "{shown}");
    assert!(!shown.contains("Failed:"), "{shown}");
    assert!(shown.contains("Passed:"), "{shown}");
}

#[test]
fn check_of_a_failing_plan_lists_the_failed_check_first_and_exits_with_1() {
    let output = jumpgate(
        &fixture_dir(),
        &["check", "plans/start-cycle.json", "--plan-only"],
    );
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    let shown = printed(&output);
    let failed = shown.find("Failed:").expect(&shown);
    assert!(failed < shown.find("Passed:").expect(&shown), "{shown}");
    assert!(shown.contains("jumpgate::check::start_cycle"), "{shown}");
    assert!(shown.contains("shelf/web"), "{shown}");
}

#[test]
fn check_with_json_prints_each_check_with_its_status_and_failures() {
    let output = jumpgate(
        &fixture_dir(),
        &["check", "plans/unit-missing.json", "--json", "--plan-only"],
    );
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    let shown = printed(&output);
    let document: serde_json::Value = shown
        .lines()
        .find_map(|line| serde_json::from_str(line).ok())
        .expect("a JSON document is printed");
    let text = document.to_string();
    assert!(text.contains("jumpgate::check::unit_missing"), "{text}");
    assert!(text.contains("\"status\":\"failed\""), "{text}");
    assert!(text.contains("\"status\":\"passed\""), "{text}");
}

#[test]
fn check_of_a_file_that_is_no_plan_or_is_missing_refuses_with_its_code_and_exits_with_1() {
    for (file, code) in [
        ("plans/syntax.json", "jumpgate::record::syntax"),
        ("plans/wrong-type.json", "jumpgate::record::wrong_type"),
        (
            "plans/unknown-field.json",
            "jumpgate::record::unknown_field",
        ),
        ("plans/nowhere.json", "jumpgate::store::io"),
    ] {
        let output = jumpgate(&fixture_dir(), &["check", file, "--plan-only"]);
        assert_eq!(output.status.code(), Some(1), "{file}");
        let shown = printed(&output);
        assert!(shown.contains(code), "{file}: {shown}");
        assert!(!shown.contains("Passed:"), "{file}: {shown}");
    }
}

#[test]
fn check_without_a_plan_is_refused_with_exit_code_2() {
    let temp = scratch();
    let output = jumpgate(temp.path(), &["check"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn check_writes_nothing() {
    let temp = scratch();
    let dir = temp.path();
    let plan = read_text(&fixture_dir().join("plan.json"));
    put(dir, "plan.json", &plan);
    let before = files(dir);
    let output = jumpgate(dir, &["check", "plan.json", "--plan-only"]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    assert_eq!(changed(&before, &files(dir)), Vec::<PathBuf>::new());
}

#[test]
fn check_passes_the_plan_that_resolve_wrote_for_a_local_and_a_cloud_target() {
    let temp = scratch();
    let dir = temp.path();
    let store = stocked(dir);
    for target in ["local", "cloud"] {
        let out = dir.join(format!("{target}-plan.json"));
        let resolved = resolve(dir, &store, target, &["--out", &out.display().to_string()]);
        assert_eq!(resolved.status.code(), Some(0), "{}", printed(&resolved));
        let output = check_against(dir, &store, &out, &[]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{target}: {}",
            printed(&output)
        );
        assert!(text_of(&output).contains("0 of 6 checks failed"));
    }
}

#[test]
fn check_shows_no_fallback_value_whether_the_plan_passes_or_fails() {
    let temp = scratch();
    let dir = temp.path();
    let marker = "s3cr3t-marker-value";
    let mut plan: serde_json::Value =
        serde_json::from_str(&read_text(&fixture_dir().join("plan.json"))).unwrap();
    plan["units"][1]["bindings"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "need": "/jumpgate/needs/shelf/dev-key",
                "kind": "value",
                "consumers": ["web"],
            "via": "fallback",
            "delivers_as": {"variables": ["DEV_KEY"]},
            "supplies": {"fallback": marker}
        }));
    put(dir, "local.json", &plan.to_string());
    plan["target"]["kind"] = serde_json::json!("cloud");
    put(dir, "cloud.json", &plan.to_string());
    let local = jumpgate(dir, &["check", "local.json", "--plan-only"]);
    assert_eq!(local.status.code(), Some(0), "{}", printed(&local));
    assert!(!printed(&local).contains(marker));
    let cloud = jumpgate(dir, &["check", "cloud.json", "--json", "--plan-only"]);
    assert_eq!(cloud.status.code(), Some(1), "{}", printed(&cloud));
    let shown = printed(&cloud);
    assert!(shown.contains("jumpgate::check::cloud_fallback"), "{shown}");
    assert!(!shown.contains(marker), "{shown}");
}

#[test]
fn check_refuses_a_fallback_value_on_a_secret_need_even_in_a_fallback_binding() {
    let output = jumpgate(
        &fixture_dir(),
        &["check", "plans/secret-fallback.json", "--plan-only"],
    );
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    let shown = printed(&output);
    assert!(shown.contains("jumpgate::check::secret_value"), "{shown}");
    assert!(!shown.contains("not-a-real-secret"), "{shown}");
}

/// Runs `check` on `plan` against the store directory `store`.
fn check_against(dir: &Path, store: &Path, plan: &Path, extra: &[&str]) -> Output {
    let store = store.display().to_string();
    let plan = plan.display().to_string();
    let mut args = vec!["check", &plan, "--store", &store];
    args.extend_from_slice(extra);
    jumpgate(dir, &args)
}

/// Resolves `local` over a stocked store and returns the store and the plan.
fn resolved(dir: &Path) -> (PathBuf, PathBuf) {
    let store = stocked(dir);
    let plan = dir.join("local-plan.json");
    let output = resolve(
        dir,
        &store,
        "local",
        &["--out", &plan.display().to_string()],
    );
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    (store, plan)
}

#[test]
fn check_names_the_record_that_the_store_lacks_for_a_target_a_need_and_an_offer() {
    for (file, code) in [
        (
            "jumpgate/targets/local.json",
            "jumpgate::check::target_unknown",
        ),
        (
            "jumpgate/needs/shelf/api-token.json",
            "jumpgate::check::need_unknown",
        ),
    ] {
        let temp = scratch();
        let dir = temp.path();
        let (store, plan) = resolved(dir);
        remove_file(&store.join(file));
        let output = check_against(dir, &store, &plan, &[]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{file}: {}",
            printed(&output)
        );
        assert!(
            printed(&output).contains(code),
            "{file}: {}",
            printed(&output)
        );
    }
}

#[test]
fn check_names_an_offer_that_the_store_lacks() {
    let temp = scratch();
    let dir = temp.path();
    let (store, plan) = resolved(dir);
    let offers = store.join("jumpgate/offers");
    remove_dir(&offers);
    let output = check_against(dir, &store, &plan, &[]);
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    assert!(
        printed(&output).contains("jumpgate::check::offer_unknown"),
        "{}",
        printed(&output)
    );
}

/// The files of the fixture store `agree`, which holds the records that the
/// plan `store-plan.json` states.
const AGREE: [&str; 4] = [
    "jumpgate/targets/local.json",
    "jumpgate/needs/shelf/api-token.json",
    "jumpgate/needs/shelf/session-key.json",
    "jumpgate/offers/secret-store/one-password/api-token.json",
];

/// Copies the fixture store `agree` and the plan `store-plan.json` into
/// `dir` and returns the store's directory and the plan's file.
fn agreeing(dir: &Path) -> (PathBuf, PathBuf) {
    let store = dir.join("agree");
    for file in AGREE {
        let content = read_text(&fixture_dir().join("agree").join(file));
        put(&store, file, &content);
    }
    let plan = dir.join("store-plan.json");
    put(
        dir,
        "store-plan.json",
        &read_text(&fixture_dir().join("store-plan.json")),
    );
    (store, plan)
}

/// Rewrites the JSON file `file` of `store` with `change`.
fn change_json(store: &Path, file: &str, change: impl FnOnce(&mut serde_json::Value)) {
    let path = store.join(file);
    let mut value: serde_json::Value = serde_json::from_str(&read_text(&path)).unwrap();
    change(&mut value);
    put(store, file, &value.to_string());
}

#[test]
fn check_passes_a_plan_against_the_store_that_agrees_with_it() {
    let temp = scratch();
    let dir = temp.path();
    let (store, plan) = agreeing(dir);
    let output = check_against(dir, &store, &plan, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
}

/// Each code of the comparison of a plan with its store, and the change to
/// the agreeing plan or the agreeing store that provokes it. The twin of
/// each is the run on the agreeing pair, which passes.
#[test]
fn check_provokes_each_code_of_the_comparison_with_a_change_that_the_twin_lacks() {
    type Change = fn(&Path, &mut serde_json::Value);
    /// Pairs a code with its change, which coerces the closure to a function.
    fn case(code: &'static str, change: Change) -> (&'static str, Change) {
        (code, change)
    }
    let cases = [
        case("jumpgate::check::target_unknown", |_, plan| {
            plan["target"]["name"] = serde_json::json!("staging");
        }),
        case("jumpgate::check::target_kind", |_, plan| {
            plan["target"]["kind"] = serde_json::json!("cloud");
        }),
        case("jumpgate::check::need_unknown", |store, _| {
            remove_file(&store.join("jumpgate/needs/shelf/session-key.json"));
        }),
        case("jumpgate::check::need_omitted", |_, plan| {
            plan["unbound"] = serde_json::json!([]);
        }),
        case("jumpgate::check::unbound_differs", |_, plan| {
            plan["unbound"][0]["when_unbound"] = serde_json::json!("fail");
        }),
        case("jumpgate::check::secret_fallback", |store, _| {
            change_json(store, "jumpgate/needs/shelf/session-key.json", |need| {
                need["kind"] = serde_json::json!("secret");
                need["when_unbound"] = serde_json::json!({"fallback": "placeholder"});
            });
        }),
        case("jumpgate::check::need_kind", |store, _| {
            change_json(store, "jumpgate/needs/shelf/api-token.json", |need| {
                need["kind"] = serde_json::json!("value");
            });
        }),
        case("jumpgate::check::need_consumers", |store, _| {
            change_json(store, "jumpgate/needs/shelf/api-token.json", |need| {
                need["consumers"] = serde_json::json!(["web", "worker"]);
            });
        }),
        case("jumpgate::check::offer_unknown", |store, _| {
            remove_file(&store.join("jumpgate/offers/secret-store/one-password/api-token.json"));
        }),
        case("jumpgate::check::offer_reference", |store, _| {
            change_json(
                store,
                "jumpgate/offers/secret-store/one-password/api-token.json",
                |offer| {
                    offer["reference"] =
                        serde_json::json!({"secret": "op://shelf-dev/other/credential"});
                },
            );
        }),
    ];
    for (code, change) in cases {
        let temp = scratch();
        let dir = temp.path();
        let (store, plan) = agreeing(dir);
        let twin = check_against(dir, &store, &plan, &[]);
        assert_eq!(twin.status.code(), Some(0), "{code}: {}", printed(&twin));
        let mut stated: serde_json::Value = serde_json::from_str(&read_text(&plan)).unwrap();
        change(&store, &mut stated);
        put(dir, "store-plan.json", &stated.to_string());
        let output = check_against(dir, &store, &plan, &[]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{code}: {}",
            printed(&output)
        );
        assert!(
            printed(&output).contains(code),
            "{code}: {}",
            printed(&output)
        );
    }
}

#[test]
fn check_with_plan_only_reads_no_store_and_names_none_in_its_report() {
    let temp = scratch();
    let dir = temp.path();
    let (_, plan) = resolved(dir);
    let bin = stub_board(dir);
    let plan = plan.display().to_string();
    let output = jumpgate_with_board(dir, &bin, &["check", &plan, "--plan-only", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    assert_eq!(board_calls(dir), Vec::<String>::new());
    assert!(
        !printed(&output).contains("\"store\""),
        "{}",
        printed(&output)
    );
}

#[test]
fn check_without_a_store_option_compares_the_plan_with_the_personal_board() {
    let temp = scratch();
    let dir = temp.path();
    let (_, plan) = resolved(dir);
    let bin = stub_board(dir);
    let plan = plan.display().to_string();
    let output = jumpgate_with_board(dir, &bin, &["check", &plan]);
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    assert!(
        printed(&output).contains("jumpgate::store::board"),
        "{}",
        printed(&output)
    );
    let calls = board_calls(dir);
    assert!(!calls.is_empty());
    assert!(
        calls
            .iter()
            .all(|call| call.starts_with("--personal --no-input ")),
        "{calls:?}"
    );
}

#[test]
fn check_refuses_plan_only_with_a_store_option_with_exit_code_2() {
    let temp = scratch();
    let output = jumpgate(
        temp.path(),
        &["check", "plan.json", "--plan-only", "--board", "b"],
    );
    assert_eq!(output.status.code(), Some(2));
}

fn remove_file(file: &Path) {
    std::fs::remove_file(file).unwrap();
}

fn remove_dir(dir: &Path) {
    std::fs::remove_dir_all(dir).unwrap();
}

/// Makes the git repository `name` in `dir` a board and returns its path.
/// Returns `None` with a message when no `board` program is installed, so
/// the test is skipped.
#[expect(
    clippy::disallowed_methods,
    reason = "the test starts board to make the temporary board it runs against"
)]
fn temporary_board(dir: &Path, name: &str) -> Option<PathBuf> {
    let installed = Command::new("board")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !installed {
        skipped();
        return None;
    }
    let board = dir.join(name);
    std::fs::create_dir_all(&board).unwrap();
    git(&board, &["init", "--quiet"]);
    git(&board, &["config", "user.name", "jumpgate test"]);
    git(&board, &["config", "user.email", "test@example.com"]);
    let output = Command::new("board")
        .args(["init", "--no-input"])
        .current_dir(&board)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", printed(&output));
    Some(board)
}

/// Writes the target record `name` into the board at `board`.
fn put_board_target(board: &Path, name: &str, authority: &str) {
    put_board_record(
        board,
        &format!("/jumpgate/targets/{name}"),
        &serde_json::json!({
            "$type": "jumpgate/target/v1",
            "name": name,
            "kind": "local",
            "authority": authority,
        }),
    );
}

/// Writes `record` at `path` into the board at `board` with `board put`.
#[expect(
    clippy::disallowed_methods,
    reason = "the test starts board to write a record into the temporary board"
)]
fn put_board_record(board: &Path, path: &str, record: &serde_json::Value) {
    use std::io::Write as _;
    let mut child = Command::new("board")
        .arg("-C")
        .arg(board)
        .args(["--no-input", "put"])
        .arg(path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(record.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", text(&output.stderr));
}

/// Runs the built binary with the `PATH` of the test, so it finds the
/// `board` that the test found, and with `HOME` in `dir`.
fn jumpgate_on_path(dir: &Path, args: &[&str]) -> Output {
    #[expect(
        clippy::disallowed_methods,
        reason = "the test runs the built binary, as a person would"
    )]
    let output = Command::new(env!("CARGO_BIN_EXE_jumpgate"))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("HOME", dir)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("the binary runs");
    output
}

/// Counts the commits of the board at `board`.
#[expect(
    clippy::disallowed_methods,
    reason = "the test starts git to count the commits of its temporary board"
)]
fn commits(board: &Path) -> usize {
    let output = Command::new("git")
        .arg("-C")
        .arg(board)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    text(&output.stdout).trim().parse().unwrap()
}

#[test]
fn needs_resolve_and_check_run_end_to_end_against_a_real_board() {
    let temp = scratch();
    let dir = temp.path();
    let Some(board) = temporary_board(dir, "team") else {
        return;
    };
    let board_arg = board.display().to_string();
    let repos = display(&stack(dir));
    let mut needs = vec!["needs"];
    for repo in &repos {
        needs.push("--repo");
        needs.push(repo);
    }
    needs.extend(["--board", &board_arg]);
    let written = jumpgate_on_path(dir, &needs);
    assert_eq!(written.status.code(), Some(0), "{}", printed(&written));
    assert!(
        printed(&written).contains("(board team)"),
        "{}",
        printed(&written)
    );
    // The board holds the records at the spec paths and the five types.
    assert!(board.join("jumpgate/needs/shelf/api-token.json").is_file());
    assert!(
        board
            .join(".board/types/jumpgate/need/v1.schema.json")
            .is_file()
    );
    put_board_target(&board, "local", "team");

    let plan = dir.join("plan.json");
    let plan_arg = plan.display().to_string();
    let resolved = jumpgate_on_path(
        dir,
        &[
            "resolve", "--target", "local", "--board", &board_arg, "--out", &plan_arg,
        ],
    );
    assert_eq!(resolved.status.code(), Some(0), "{}", printed(&resolved));
    let checked = jumpgate_on_path(dir, &["check", &plan_arg, "--board", &board_arg]);
    assert_eq!(checked.status.code(), Some(0), "{}", printed(&checked));
    assert!(text_of(&checked).contains("0 of 6 checks failed"));

    // A second run of needs, with nothing changed, makes no commit.
    let before = commits(&board);
    let again = jumpgate_on_path(dir, &needs);
    assert_eq!(again.status.code(), Some(0), "{}", printed(&again));
    assert_eq!(commits(&board), before);

    // A need of a read app that no declaration gives is deleted by the next
    // run, in one commit.
    let stale = board.join("jumpgate/needs/shelf/old-token.json");
    put_board_record(
        &board,
        "/jumpgate/needs/shelf/old-token",
        &serde_json::json!({
            "$type": "jumpgate/need/v1",
            "app": "shelf",
            "name": "old-token",
            "kind": "secret",
            "consumers": ["web"],
            "declared_in": {"repo": "github.com/acme/shelf", "file": "starmap.toml"},
            "delivers_as": {"variables": ["OLD_TOKEN"]},
            "optional": false,
            "when": [],
            "when_unbound": "fail"
        }),
    );
    assert!(stale.is_file());
    let before = commits(&board);
    let cleaned = jumpgate_on_path(dir, &needs);
    assert_eq!(cleaned.status.code(), Some(0), "{}", printed(&cleaned));
    assert!(
        printed(&cleaned).contains("/jumpgate/needs/shelf/old-token"),
        "{}",
        printed(&cleaned)
    );
    assert!(!stale.exists());
    assert_eq!(commits(&board), before + 1);
}

/// Reports that a test is skipped because `board` is not installed.
#[expect(
    clippy::disallowed_macros,
    reason = "the skip message is the test's own report"
)]
fn skipped() {
    eprintln!("skipped: no board program on PATH, so there is no board to run against");
}

/// Copies every file under `from` into `to`, keeping the directories.
#[expect(
    clippy::disallowed_methods,
    reason = "the test copies the fixture files into its own temporary directory"
)]
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The service that the golden run adds to the `beacon` fixture, which no
/// process lists.
const BEACON_UNUSED_SERVICE: &str = "\n[service.jobs]\nkind = \"sqs\"\n";

/// Copies the `beacon` fixture into a new git checkout under `root`, with
/// `extra` added to its `mxv.toml`, and returns the checkout's directory.
#[expect(
    clippy::disallowed_methods,
    reason = "the test adds to the mxv.toml of its own copy of the fixture"
)]
fn beacon(root: &Path, extra: &str) -> PathBuf {
    let dir = root.join("beacon");
    copy_tree(&Path::new(STACK).join("beacon"), &dir);
    let mxv = dir.join("mxv.toml");
    let mut text = std::fs::read_to_string(&mxv).unwrap();
    text.push_str(extra);
    std::fs::write(&mxv, text).unwrap();
    git(&dir, &["init", "--quiet"]);
    git(
        &dir,
        &["remote", "add", "origin", "git@github.com:acme/beacon.git"],
    );
    dir
}

#[test]
fn needs_on_a_git_copy_of_the_beacon_fixture_prints_the_records_and_the_unused_service() {
    let temp = scratch();
    let dir = temp.path();
    let checkout = beacon(dir, BEACON_UNUSED_SERVICE);
    let repos = [checkout.display().to_string()];
    let output = jumpgate(dir, &needs_args(&repos, "records"));
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/beacon-needs.txt");
    let expected = read_text(&golden);
    assert_eq!(text(&output.stdout), expected);
    let written = files(&dir.join("records"));
    let names: Vec<String> = written
        .keys()
        .map(|path| path.display().to_string())
        .filter(|path| path.contains("needs/") || path.contains("workspace/"))
        .collect();
    assert_eq!(
        names,
        [
            "jumpgate/needs/beacon/api-signing-key.json",
            "jumpgate/needs/beacon/beacon-token.json",
            "jumpgate/needs/beacon/files.json",
            "jumpgate/needs/beacon/store.json",
            "jumpgate/offers/workspace/mxv/files.json",
            "jumpgate/offers/workspace/mxv/store.json",
        ]
    );
}

#[test]
fn needs_with_json_lists_the_unused_service_and_leaves_none_when_every_service_is_listed() {
    let temp = scratch();
    let dir = temp.path();
    let checkout = beacon(dir, BEACON_UNUSED_SERVICE);
    let repos = [checkout.display().to_string()];
    let mut args = needs_args(&repos, "records");
    args.push("--json");
    let output = jumpgate(dir, &args);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["data"]["unused"],
        serde_json::json!(["github.com/acme/beacon:jobs"])
    );
    let other = scratch();
    let plain = beacon(other.path(), "");
    let repos = [plain.display().to_string()];
    let mut args = needs_args(&repos, "records");
    args.push("--json");
    let output = jumpgate(other.path(), &args);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["data"]["unused"], serde_json::json!([]));
}

/// Reads the beacon fixture into a store with `needs` and adds a `local` and
/// a `cloud` target.
fn beacon_store(dir: &Path) -> PathBuf {
    let checkout = beacon(dir, "");
    let store = dir.join("records");
    let repos = [checkout.display().to_string()];
    let location = store.display().to_string();
    let output = jumpgate(dir, &needs_args(&repos, &location));
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    put_target(&store, "local", "local");
    put_target(&store, "cloud", "cloud");
    store
}

#[test]
fn resolve_for_a_local_target_binds_the_four_needs_of_the_beacon_fixture_and_check_passes_the_plan()
{
    let temp = scratch();
    let dir = temp.path();
    let store = beacon_store(dir);
    let out = dir.join("local-plan.json");
    let out_path = out.display().to_string();
    let output = resolve(dir, &store, "local", &["--out", &out_path]);
    assert_eq!(output.status.code(), Some(0), "{}", printed(&output));
    let plan = read_plan(&out);
    assert_eq!(plan["unbound"], serde_json::json!([]));
    let api = plan["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["name"] == "api")
        .unwrap();
    let service = api["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|binding| binding["need"] == "/jumpgate/needs/beacon/store")
        .unwrap();
    assert_eq!(service["offer"], "/jumpgate/offers/workspace/mxv/store");
    assert_eq!(
        service["supplies"],
        serde_json::json!({"reference": {"service": {"kind": "dynamodb", "name": "store"}}})
    );
    assert_eq!(
        service["delivers_as"],
        serde_json::json!({"variables": ["AWS_ENDPOINT_URL_DYNAMODB", "STORE_TABLE"]})
    );
    let checked = check_against(dir, &store, &out, &[]);
    assert_eq!(checked.status.code(), Some(0), "{}", printed(&checked));
}

#[test]
fn resolve_for_a_cloud_target_lists_the_services_as_local_only_and_check_reads_that_plan() {
    let temp = scratch();
    let dir = temp.path();
    let store = beacon_store(dir);
    let out = dir.join("cloud-plan.json");
    let out_path = out.display().to_string();
    let output = resolve(dir, &store, "cloud", &["--out", &out_path]);
    assert_eq!(output.status.code(), Some(1), "{}", printed(&output));
    assert!(
        printed(&output).contains("jumpgate::need::local_only"),
        "{}",
        printed(&output)
    );
    let plan = read_plan(&out);
    let unbound: Vec<(&str, &str)> = plan["unbound"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["need"].as_str().unwrap(),
                entry["reason"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        unbound,
        [
            ("/jumpgate/needs/beacon/files", "local_only"),
            ("/jumpgate/needs/beacon/store", "local_only"),
        ]
    );
    let checked = check_against(dir, &store, &out, &[]);
    let shown = printed(&checked);
    assert!(shown.contains("jumpgate::check::need_unbound"), "{shown}");
    assert!(!shown.contains("jumpgate::record::"), "{shown}");
}
