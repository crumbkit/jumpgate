//! End-to-end tests of the `jumpgate` skeleton: its help, its version, and
//! the three commands that later versions fill in, each of which fails with
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
    for command in ["needs", "resolve", "check"] {
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
