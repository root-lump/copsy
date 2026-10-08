mod support;

use std::fs;
use std::path::Path;
use std::process::Output;
use support::{Repository, git};

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn cd_marker(path: &Path) -> String {
    format!("__COPSY_CD__{}\n", path.display())
}

#[test]
fn new_reuse_and_switch_preserve_setup_and_launch_marker_order() {
    let repo = Repository::new();
    let target = repo.root.join("remote-name-feature-topic");
    let expected = format!(
        "__COPSY_SETUP__{0}\n__COPSY_CD__{0}\n__COPSY_LAUNCH__code\t{0}\n\
         __COPSY_LAUNCH__cursor\t{0}\n__COPSY_OPEN__echo custom\n\
         __COPSY_LAUNCH__claude\t{0}\n__COPSY_LAUNCH__codex\t{0}\n",
        target.display()
    );
    for command in ["new", "add", "switch"] {
        let output = repo.run(
            &repo.main,
            &[
                command,
                "feature/topic",
                "--setup",
                "--code",
                "--cursor",
                "--claude",
                "--codex",
                "--open",
                "echo custom",
            ],
        );
        assert_success(&output);
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn carries_untracked_changes_and_keeps_status_output_off_stdout() {
    let repo = Repository::new();
    fs::write(repo.main.join("scratch.txt"), "work in progress").unwrap();
    let output = repo.run(&repo.main, &["new", "feature", "--carry"]);
    assert_success(&output);
    let target = repo.root.join("remote-name-feature");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        cd_marker(&target)
    );
    assert!(!repo.main.join("scratch.txt").exists());
    assert_eq!(
        fs::read_to_string(target.join("scratch.txt")).unwrap(),
        "work in progress"
    );
    assert!(git(&repo.main, &["stash", "list"]).stdout.is_empty());
    for command in ["list", "status"] {
        let output = repo.run(&repo.main, &[command]);
        assert_success(&output);
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn failed_creation_keeps_carried_changes_in_the_recovery_stash() {
    let repo = Repository::new();
    git(&repo.main, &["branch", "feature"]);
    fs::write(repo.main.join("scratch.txt"), "recover me").unwrap();
    let output = repo.run(
        &repo.main,
        &["new", "feature", "--carry", "--setup", "--codex"],
    );
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "failed preparation must not emit actions"
    );
    assert!(!repo.root.join("remote-name-feature").exists());
    let stash = git(&repo.main, &["stash", "list"]);
    assert!(String::from_utf8_lossy(&stash.stdout).contains("copsy:carry:"));
    git(&repo.main, &["stash", "pop"]);
    assert_eq!(
        fs::read_to_string(repo.main.join("scratch.txt")).unwrap(),
        "recover me"
    );
}

#[test]
fn close_and_remove_delete_the_calling_worktree_and_branch_from_main() {
    // Each CLI runs in its own process: removing its cwd cannot race other tests.
    for command in ["close", "remove"] {
        let repo = Repository::new();
        assert_success(&repo.run(&repo.main, &["new", "feature"]));
        let target = repo.root.join("remote-name-feature");
        let nested = target.join("inside");
        fs::create_dir(&nested).unwrap();
        let args = if command == "close" {
            vec!["close", "--with-branch"]
        } else {
            vec!["remove", "feature", "--with-branch"]
        };
        let output = repo.run(&nested, &args);
        assert_success(&output);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            cd_marker(&repo.main)
        );
        assert!(!target.exists());
        assert!(
            git(&repo.main, &["branch", "--list", "feature"])
                .stdout
                .is_empty()
        );
    }
}

#[test]
fn dirty_calling_worktrees_are_preserved_without_relocation() {
    for command in ["close", "remove"] {
        let repo = Repository::new();
        assert_success(&repo.run(&repo.main, &["new", "feature"]));
        let target = repo.root.join("remote-name-feature");
        fs::write(target.join("scratch.txt"), "keep me").unwrap();
        let args = if command == "close" {
            vec!["close"]
        } else {
            vec!["remove", "feature"]
        };
        let output = repo.run(&target, &args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            fs::read_to_string(target.join("scratch.txt")).unwrap(),
            "keep me"
        );
    }
}

#[test]
fn partial_remove_all_relocates_only_when_the_calling_worktree_was_removed() {
    for caller_is_dirty in [false, true] {
        let repo = Repository::new();
        for branch in ["clean", "dirty"] {
            assert_success(&repo.run(&repo.main, &["new", branch]));
        }
        let clean = repo.root.join("remote-name-clean");
        let dirty = repo.root.join("remote-name-dirty");
        fs::write(dirty.join("scratch.txt"), "keep me").unwrap();
        let caller = if caller_is_dirty { &dirty } else { &clean };
        let output = repo.run(caller, &["remove", "--all", "--with-branch"]);
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            if caller_is_dirty {
                String::new()
            } else {
                cd_marker(&repo.main)
            }
        );
        assert!(!clean.exists());
        assert_eq!(
            fs::read_to_string(dirty.join("scratch.txt")).unwrap(),
            "keep me"
        );
        let branches = git(&repo.main, &["branch", "--format=%(refname:short)"]);
        let branches = String::from_utf8(branches.stdout).unwrap();
        assert!(!branches.lines().any(|branch| branch == "clean"));
        assert!(branches.lines().any(|branch| branch == "dirty"));
    }
}

#[test]
fn removing_last_nested_worktree_prunes_only_its_repository_directory() {
    let repo = Repository::new();
    let base = repo.root.join("worktrees");
    fs::write(
        repo.main.join(".git/copsy.toml"),
        format!(
            "[worktree]\nlayout = 'nested'\nbase_dir = '{}'\n",
            base.display()
        ),
    )
    .unwrap();
    for branch in ["first", "last"] {
        assert_success(&repo.run(&repo.main, &["new", branch]));
    }
    let repository_dir = base.join("remote-name");
    assert_success(&repo.run(&repo.main, &["remove", "first"]));
    assert!(repository_dir.join("last").exists());
    assert_success(&repo.run(&repo.main, &["remove", "last"]));
    assert!(!repository_dir.exists());
    assert!(base.exists());
    assert!(repo.main.exists());
}

#[cfg(unix)]
#[test]
fn ordinary_shell_setup_and_launches_handle_literal_paths_in_bash_and_zsh() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    for shell in ["bash", "zsh"] {
        let repo = Repository::new();
        let base = repo.root.join("work trees'$(false)");
        fs::write(repo.main.join(".git/copsy.toml"), format!(
            "[worktree]\nbase_dir = \"{}\"\n[setup]\ncommand = ['sh', '-c', 'pwd > setup.cwd']\n", base.display()
        )).unwrap();
        let bin = repo.root.join("bin");
        fs::create_dir(&bin).unwrap();
        let code = bin.join("code");
        fs::write(&code, "#!/bin/sh\n[ \"$1\" = -- ] || exit 20\n[ -f \"$2/setup.cwd\" ] || exit 21\nprintf '%s\\n' \"$2\" > code.target\n").unwrap();
        fs::set_permissions(&code, fs::Permissions::from_mode(0o755)).unwrap();
        let script = repo.root.join("integration.sh");
        fs::write(&script, repo.run(&repo.main, &["init", shell]).stdout).unwrap();
        let template = repo.command(&repo.main);
        let mut command = Command::new(shell);
        for (key, value) in template.get_envs() {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        let mut paths = vec![
            bin,
            Path::new(env!("CARGO_BIN_EXE_copsy"))
                .parent()
                .unwrap()
                .to_path_buf(),
        ];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        let output = command
            .current_dir(&repo.main)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .args([
                "-c",
                r#"
source "$1"
before="$PWD"
copsy new feature --setup --code --open 'pwd > open.cwd' || exit 1
[[ "$PWD" != "$before" ]] || exit 2
[[ "$(cat setup.cwd)" == "$PWD" ]] || exit 3
[[ "$(cat code.target)" == "$PWD" ]] || exit 4
[[ "$(cat open.cwd)" == "$PWD" ]] || exit 5
rm setup.cwd code.target open.cwd
copsy close --with-branch || exit 6
[[ "$PWD" == "$before" ]] || exit 7
"#,
                "shell-test",
            ])
            .arg(script)
            .output()
            .unwrap();
        assert_success(&output);
        assert!(output.stdout.is_empty(), "{shell}: {output:?}");
        assert!(!base.join("remote-name-feature").exists());
    }
}
