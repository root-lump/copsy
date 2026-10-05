use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

struct Repository {
    _temp: TempDir,
    root: PathBuf,
    main: PathBuf,
    home: PathBuf,
    config: PathBuf,
}

impl Repository {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let main = root.join("local-clone");
        let home = root.join("home");
        let config = root.join("config");
        for path in [&main, &home, &config] {
            fs::create_dir(path).unwrap();
        }
        git(&main, &["init", "-q", "-b", "main"]);
        git(&main, &["commit", "-q", "--allow-empty", "-m", "initial"]);
        git(
            &main,
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/remote-name.git",
            ],
        );
        Self {
            _temp: temp,
            root,
            main,
            home,
            config,
        }
    }

    fn command(&self, cwd: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_copsy"));
        command
            .current_dir(cwd)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("XDG_CONFIG_HOME", &self.config)
            .env_remove("HERDR_CONFIG_PATH")
            .env_remove("HERDR_ENV")
            .env_remove("HERDR_WORKSPACE_ID")
            .env_remove("HERDR_PANE_ID")
            .env_remove("HERDR_API_SOCKET")
            .env("GIT_CONFIG_GLOBAL", self.root.join("no-global-config"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        command
    }

    fn run(&self, cwd: &Path, arguments: &[&str]) -> Output {
        self.command(cwd).args(arguments).output().unwrap()
    }

    fn herdr_config(&self, contents: &str) -> PathBuf {
        let directory = self.config.join("herdr");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("config.toml");
        fs::write(&path, contents).unwrap();
        path
    }

    fn default_worktree(&self, slug: &str) -> PathBuf {
        self.home.join(".herdr/worktrees/local-clone").join(slug)
    }
}

fn git(cwd: &Path, arguments: &[&str]) -> Output {
    let output = Command::new("git")
        .current_dir(cwd)
        .args([
            "-c",
            "user.name=Copsy Test",
            "-c",
            "user.email=copsy@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_target(output: &Output, target: &Path, branch: &str) {
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_checkout(output, target, branch);
}

fn assert_checkout(output: &Output, target: &Path, branch: &str) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(target.join(".git").is_file());
    let actual = git(target, &["branch", "--show-current"]);
    assert_eq!(String::from_utf8_lossy(&actual.stdout).trim(), branch);
}

#[cfg(unix)]
fn install_mock_herdr(bin: &Path, fail: bool) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(bin).unwrap();
    let path = bin.join("herdr");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$HERDR_TEST_LOG\"\n\
             [ -f \"$6/.git\" ] || exit 9\n\
             printf '%s\\n' \"$HERDR_SOCKET_PATH\" > \"$HERDR_TEST_CONTEXT\"\n\
             printf '{{\"result\":{{\"workspace\":\"w2\"}}}}\\n'\n\
             {}\n",
            if fail {
                "printf 'server unavailable\\n' >&2; exit 1"
            } else {
                "exit 0"
            }
        ),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
fn mock_session_command(repo: &Repository, cwd: &Path, bin: &Path) -> Command {
    let mut paths = vec![bin.to_path_buf()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let mut command = repo.command(cwd);
    command
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("HERDR_ENV", "1")
        .env("HERDR_WORKSPACE_ID", "w7")
        .env("HERDR_SOCKET_PATH", repo.root.join("session.sock"))
        .env("HERDR_TEST_LOG", repo.root.join("herdr-call"))
        .env("HERDR_TEST_CONTEXT", repo.root.join("herdr-context"));
    command
}

#[cfg(unix)]
fn assert_registration(repo: &Repository, target: &Path) {
    let arguments = fs::read_to_string(repo.root.join("herdr-call")).unwrap();
    assert_eq!(
        arguments.lines().collect::<Vec<_>>(),
        [
            "worktree",
            "open",
            "--cwd",
            repo.main.to_str().unwrap(),
            "--path",
            target.to_str().unwrap(),
            "--no-focus",
        ]
    );
    assert_eq!(
        fs::read_to_string(repo.root.join("herdr-context"))
            .unwrap()
            .trim(),
        repo.root.join("session.sock").to_str().unwrap()
    );
}

#[cfg(unix)]
#[test]
fn herdr_registers_new_and_reused_worktrees_from_the_repository_parent() {
    let repo = Repository::new();
    let bin = repo.root.join("bin with spaces");
    install_mock_herdr(&bin, false);
    let target = repo.default_worktree("feature-register");
    let output = mock_session_command(&repo, &repo.main, &bin)
        .args(["new", "Feature/Register", "--herdr"])
        .output()
        .unwrap();
    assert_target(&output, &target, "Feature/Register");
    assert_registration(&repo, &target);
    fs::remove_file(repo.root.join("herdr-call")).unwrap();
    // A caller in a linked checkout must still identify the main parent.
    let output = mock_session_command(&repo, &target, &bin)
        .args(["add", "Feature/Register", "--herdr"])
        .output()
        .unwrap();
    assert_target(&output, &target, "Feature/Register");
    assert_registration(&repo, &target);
}

#[cfg(unix)]
#[test]
fn herdr_registration_failure_preserves_the_checkout_carry_and_retry() {
    let repo = Repository::new();
    let bin = repo.root.join("bin");
    install_mock_herdr(&bin, true);
    fs::write(repo.main.join("local.txt"), "keep this change").unwrap();
    let target = repo.default_worktree("feature-retry");
    let output = mock_session_command(&repo, &repo.main, &bin)
        .args(["new", "Feature/Retry", "--herdr", "--carry"])
        .output()
        .unwrap();
    assert_target(&output, &target, "Feature/Retry");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Herdr workspace registration failed"),
        "{stderr}"
    );
    assert!(stderr.contains("server unavailable"), "{stderr}");
    assert_eq!(
        fs::read_to_string(target.join("local.txt")).unwrap(),
        "keep this change"
    );
    assert!(!repo.main.join("local.txt").exists());
    install_mock_herdr(&bin, false);
    let output = mock_session_command(&repo, &repo.main, &bin)
        .args(["add", "Feature/Retry", "--herdr"])
        .output()
        .unwrap();
    assert_target(&output, &target, "Feature/Retry");
    assert_registration(&repo, &target);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("registration failed"));
}

#[cfg(unix)]
#[test]
fn outside_herdr_and_ordinary_mode_do_not_register_workspaces() {
    let repo = Repository::new();
    let bin = repo.root.join("bin");
    install_mock_herdr(&bin, false);
    let output = mock_session_command(&repo, &repo.main, &bin)
        .env("HERDR_ENV", "0")
        .args(["new", "Feature/Outside", "--herdr"])
        .output()
        .unwrap();
    assert_target(
        &output,
        &repo.default_worktree("feature-outside"),
        "Feature/Outside",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("registration requires running copsy inside Herdr")
    );
    assert!(!repo.root.join("herdr-call").exists());
    let output = mock_session_command(&repo, &repo.main, &bin)
        .args(["new", "ordinary"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!repo.root.join("herdr-call").exists());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Herdr"));
}

#[cfg(unix)]
#[test]
fn rejected_slug_collision_does_not_register_another_branch() {
    let repo = Repository::new();
    let bin = repo.root.join("bin");
    install_mock_herdr(&bin, false);
    let output = mock_session_command(&repo, &repo.main, &bin)
        .args(["new", "Feature/Collision", "--herdr"])
        .output()
        .unwrap();
    assert_target(
        &output,
        &repo.default_worktree("feature-collision"),
        "Feature/Collision",
    );
    fs::remove_file(repo.root.join("herdr-call")).unwrap();
    let output = mock_session_command(&repo, &repo.main, &bin)
        .args(["new", "feature-collision", "--herdr"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!repo.root.join("herdr-call").exists());
}

#[test]
fn new_uses_the_default_root_and_local_name_instead_of_origin() {
    let repo = Repository::new();
    let output = repo.run(&repo.main, &["new", "Feature/Auth_v2.1", "--herdr"]);
    assert_target(
        &output,
        &repo.default_worktree("feature-auth-v2-1"),
        "Feature/Auth_v2.1",
    );
}

#[test]
fn add_from_a_linked_worktree_uses_the_shared_repository_identity() {
    let repo = Repository::new();
    let first = repo.run(&repo.main, &["--herdr", "new", "Feature/First"]);
    let first_path = repo.default_worktree("feature-first");
    assert_target(&first, &first_path, "Feature/First");
    git(&repo.main, &["branch", "Feature/Second"]);
    let nested = first_path.join("nested");
    fs::create_dir(&nested).unwrap();
    let second = repo.run(&nested, &["add", "Feature/Second", "--herdr"]);
    assert_target(
        &second,
        &repo.default_worktree("feature-second"),
        "Feature/Second",
    );
}

#[test]
fn herdr_settings_override_both_copsy_configuration_scopes() {
    let repo = Repository::new();
    repo.herdr_config("[worktrees]\ndirectory = '~/custom-worktrees'\n");
    fs::create_dir(repo.config.join("copsy")).unwrap();
    fs::write(
        repo.config.join("copsy/config.toml"),
        "[worktree]\nbase_dir = '~/global'\nlayout = 'flat'\n",
    )
    .unwrap();
    fs::write(
        repo.main.join(".git/copsy.toml"),
        "[worktree]\nbase_dir = '~/repository'\nlayout = 'nested'\n",
    )
    .unwrap();
    let output = repo.run(&repo.main, &["new", "Feature/Login", "--herdr"]);
    assert_target(
        &output,
        &repo.home.join("custom-worktrees/local-clone/feature-login"),
        "Feature/Login",
    );
    assert!(!repo.home.join("global").exists());
    assert!(!repo.home.join("repository").exists());
}

#[test]
fn explicit_config_path_takes_precedence_over_xdg() {
    let repo = Repository::new();
    repo.herdr_config("invalid TOML");
    let override_path = repo.root.join("herdr-custom.toml");
    fs::write(&override_path, "[worktrees]\ndirectory = '~/override'\n").unwrap();
    let output = repo
        .command(&repo.main)
        .env("HERDR_CONFIG_PATH", override_path)
        .args(["--herdr", "new", "feature"])
        .output()
        .unwrap();
    assert_target(
        &output,
        &repo.home.join("override/local-clone/feature"),
        "feature",
    );
}

#[test]
fn config_defaults_to_home_when_xdg_is_unset() {
    let repo = Repository::new();
    let config = repo.home.join(".config/herdr");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("config.toml"),
        "[worktrees]\ndirectory = '~/home-root'\n",
    )
    .unwrap();
    let output = repo
        .command(&repo.main)
        .env_remove("XDG_CONFIG_HOME")
        .args(["new", "feature", "--herdr"])
        .output()
        .unwrap();
    assert_target(
        &output,
        &repo.home.join("home-root/local-clone/feature"),
        "feature",
    );
}

#[test]
fn malformed_herdr_config_is_rejected_before_creation() {
    let repo = Repository::new();
    repo.herdr_config("[worktrees]\ndirectory = 12\n");
    let output = repo.run(&repo.main, &["--herdr", "new", "feature"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Failed to parse Herdr config"));
    assert!(output.stdout.is_empty());
    assert!(!repo.home.join(".herdr").exists());
    let branches = git(&repo.main, &["branch", "--list", "feature"]);
    assert!(branches.stdout.is_empty());
}

#[test]
fn slug_collisions_do_not_switch_to_a_different_branch() {
    let repo = Repository::new();
    let first = repo.run(&repo.main, &["new", "Feature/Login", "--herdr"]);
    assert_target(
        &first,
        &repo.default_worktree("feature-login"),
        "Feature/Login",
    );
    let collision = repo.run(&repo.main, &["new", "feature-login", "--herdr"]);
    assert!(!collision.status.success());
    assert!(collision.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&collision.stderr)
            .contains("already used by branch 'Feature/Login'")
    );
    let same = repo.run(&repo.main, &["add", "Feature/Login", "--herdr"]);
    assert_target(
        &same,
        &repo.default_worktree("feature-login"),
        "Feature/Login",
    );
}

#[test]
fn unrelated_directories_are_preserved() {
    let repo = Repository::new();
    let target = repo.default_worktree("feature");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("keep"), "original").unwrap();
    let output = repo.run(&repo.main, &["new", "feature", "--herdr"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_to_string(target.join("keep")).unwrap(), "original");
}

#[test]
fn normal_mode_keeps_origin_naming_and_does_not_read_herdr_config() {
    let repo = Repository::new();
    repo.herdr_config("invalid TOML");
    let flat = repo.run(&repo.main, &["new", "Feature/Flat"]);
    assert_checkout(
        &flat,
        &repo.root.join("remote-name-Feature-Flat"),
        "Feature/Flat",
    );
    assert_eq!(
        String::from_utf8_lossy(&flat.stdout).trim(),
        format!(
            "__COPSY_CD__{}",
            repo.root.join("remote-name-Feature-Flat").display()
        )
    );
    fs::write(
        repo.main.join(".git/copsy.toml"),
        "[worktree]\nbase_dir = '~/nested'\nlayout = 'nested'\n",
    )
    .unwrap();
    let nested = repo.run(&repo.main, &["new", "Feature/Nested"]);
    assert_checkout(
        &nested,
        &repo.home.join("nested/remote-name/Feature-Nested"),
        "Feature/Nested",
    );
    assert_eq!(
        String::from_utf8_lossy(&nested.stdout).trim(),
        format!(
            "__COPSY_CD__{}",
            repo.home
                .join("nested/remote-name/Feature-Nested")
                .display()
        )
    );
}

#[cfg(unix)]
#[test]
fn herdr_shell_transitions_and_tools_preserve_the_calling_directory() {
    for shell in ["bash", "zsh"] {
        let repo = Repository::new();
        let bin = repo.root.join("bin");
        install_mock_herdr(&bin, false);
        repo.herdr_config("[worktrees]\ndirectory = \"~/work trees'root\"\n");
        fs::write(
            repo.main.join(".git/copsy.toml"),
            "[setup]\ncommand = ['sh', '-c', 'pwd > setup.cwd']\n",
        )
        .unwrap();
        let init = repo.run(&repo.main, &["init", shell]);
        let script = repo.root.join("integration.sh");
        fs::write(&script, init.stdout).unwrap();
        let checks = r#"
source "$1"
claude() { pwd > claude.cwd; cd /; }
codex() { pwd > codex.cwd; cd /; }
code() { printf '%s' "$2" > "$COPSY_TEST_ROOT/code.target"; cd /; }
cursor() { printf '%s' "$2" > "$COPSY_TEST_ROOT/cursor.target"; cd /; }
before="$PWD"
copsy new Feature/Stay --herdr --setup --claude --codex --code --cursor --open 'pwd > open.cwd; cd /' || exit 1
[[ "$PWD" == "$before" ]] || exit 2
copsy add Feature/Stay --herdr || exit 3
[[ "$PWD" == "$before" ]] || exit 4
copsy switch Feature/Stay --herdr || exit 5
[[ "$PWD" == "$before" ]] || exit 6
copsy add Feature/Stay --herdr --open "printf '%s' \"it's working\" > comment.result # local note" || exit 9
[[ "$PWD" == "$before" ]] || exit 10
copsy add Feature/Stay --herdr --open '' || exit 11
[[ "$PWD" == "$before" ]] || exit 12
copsy new Normal/Move || exit 7
[[ "$PWD" != "$before" ]] || exit 8
printf '%s\n' "$PWD"
"#;
        let template = mock_session_command(&repo, &repo.main, &bin);
        let mut command = Command::new(shell);
        for (key, value) in template.get_envs() {
            match value {
                Some(value) => {
                    command.env(key, value);
                }
                None => {
                    command.env_remove(key);
                }
            }
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
            .env("COPSY_TEST_ROOT", &repo.root)
            .args(["-c", checks, "shell-test"])
            .arg(script)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{shell}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let target = repo.home.join("work trees'root/local-clone/feature-stay");
        assert_registration(&repo, &target);
        assert_eq!(
            fs::read_to_string(target.join("comment.result")).unwrap(),
            "it's working",
            "{shell}: quoted command with a trailing comment"
        );
        for file in ["setup.cwd", "claude.cwd", "codex.cwd", "open.cwd"] {
            assert_eq!(
                fs::read_to_string(target.join(file)).unwrap().trim(),
                target.to_str().unwrap(),
                "{shell}: {file}"
            );
        }
        for file in ["code.target", "cursor.target"] {
            assert_eq!(
                fs::read_to_string(repo.root.join(file)).unwrap(),
                target.to_str().unwrap()
            );
        }
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            repo.root.join("remote-name-Normal-Move").to_str().unwrap()
        );
    }
}

#[test]
fn herdr_does_not_remove_or_close_the_calling_checkout() {
    let repo = Repository::new();
    let target = repo.default_worktree("feature-keep");
    assert_target(
        &repo.run(&repo.main, &["new", "Feature/Keep", "--herdr"]),
        &target,
        "Feature/Keep",
    );
    let sibling = repo.default_worktree("feature-sibling");
    assert_target(
        &repo.run(&repo.main, &["new", "Feature/Sibling", "--herdr"]),
        &sibling,
        "Feature/Sibling",
    );
    for args in [
        vec!["close", "--herdr", "--with-branch"],
        vec![
            "remove",
            "Feature/Keep",
            "--herdr",
            "--force",
            "--with-branch",
        ],
        vec!["remove", "--all", "--herdr", "--force", "--with-branch"],
    ] {
        let output = repo.run(&target, &args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(target.join(".git").exists());
        assert!(sibling.join(".git").exists());
        assert!(
            !git(&repo.main, &["branch", "--list", "Feature/Keep"])
                .stdout
                .is_empty()
        );
    }
    let output = repo.run(&repo.main, &["remove", "Feature/Sibling", "--herdr"]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!sibling.exists());
}

#[test]
fn embedded_bare_repositories_use_the_container_name() {
    let repo = Repository::new();
    let container = repo.root.join("bare-container");
    fs::create_dir(&container).unwrap();
    let bare = container.join(".bare");
    git(
        &repo.root,
        &[
            "clone",
            "-q",
            "--bare",
            repo.main.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    fs::write(container.join(".git"), "gitdir: ./.bare\n").unwrap();
    let output = repo.run(&container, &["new", "Feature/Bare", "--herdr"]);
    assert_target(
        &output,
        &repo
            .home
            .join(".herdr/worktrees/bare-container/feature-bare"),
        "Feature/Bare",
    );
}

#[test]
fn standalone_bare_and_separate_git_dirs_use_their_own_names() {
    let repo = Repository::new();
    let bare = repo.root.join("standalone.git");
    git(
        &repo.root,
        &[
            "clone",
            "-q",
            "--bare",
            repo.main.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    let output = repo.run(&bare, &["new", "Feature/Bare", "--herdr"]);
    assert_target(
        &output,
        &repo
            .home
            .join(".herdr/worktrees/standalone.git/feature-bare"),
        "Feature/Bare",
    );

    let dot_bare = repo.root.join(".bare");
    git(
        &repo.root,
        &[
            "clone",
            "-q",
            "--bare",
            repo.main.to_str().unwrap(),
            dot_bare.to_str().unwrap(),
        ],
    );
    let output = repo.run(&dot_bare, &["new", "Feature/DotBare", "--herdr"]);
    assert_target(
        &output,
        &repo.home.join(".herdr/worktrees/.bare/feature-dotbare"),
        "Feature/DotBare",
    );

    let separate = repo.root.join("separate-clone");
    fs::create_dir(&separate).unwrap();
    let git_dir = repo.root.join("separate-git-dir");
    git(
        &separate,
        &[
            "init",
            "-q",
            "--separate-git-dir",
            git_dir.to_str().unwrap(),
        ],
    );
    git(
        &separate,
        &["commit", "-q", "--allow-empty", "-m", "initial"],
    );
    let output = repo.run(&separate, &["new", "Feature/Separate", "--herdr"]);
    assert_target(
        &output,
        &repo
            .home
            .join(".herdr/worktrees/separate-git-dir/feature-separate"),
        "Feature/Separate",
    );
}

#[cfg(unix)]
#[test]
fn existing_worktrees_are_reused_through_a_symlinked_root() {
    use std::os::unix::fs::symlink;

    let repo = Repository::new();
    let root = repo.home.join("real-root");
    fs::create_dir(&root).unwrap();
    let alias = repo.home.join("alias-root");
    symlink(&root, &alias).unwrap();
    repo.herdr_config("[worktrees]\ndirectory = '~/alias-root'\n");
    let created = repo.run(&repo.main, &["new", "Feature/Login", "--herdr"]);
    assert_target(
        &created,
        &alias.join("local-clone/feature-login"),
        "Feature/Login",
    );
    let reused = repo.run(&repo.main, &["add", "Feature/Login", "--herdr"]);
    assert_target(
        &reused,
        &root.join("local-clone/feature-login"),
        "Feature/Login",
    );
}

#[cfg(unix)]
#[test]
fn pr_creation_forwards_herdr_without_using_network_or_real_gh() {
    use std::os::unix::fs::PermissionsExt;

    let repo = Repository::new();
    git(&repo.main, &["branch", "Feature/PR"]);
    let remote = repo.root.join("remote.git");
    git(
        &repo.root,
        &[
            "clone",
            "-q",
            "--bare",
            repo.main.to_str().unwrap(),
            remote.to_str().unwrap(),
        ],
    );
    git(
        &repo.main,
        &["remote", "set-url", "origin", remote.to_str().unwrap()],
    );
    let bin = repo.root.join("bin");
    fs::create_dir(&bin).unwrap();
    let gh = bin.join("gh");
    fs::write(&gh, "#!/bin/sh\nif [ \"$1\" = pr ] && [ \"$2\" = view ]; then printf 'Feature/PR\\n'; else exit 1; fi\n").unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    install_mock_herdr(&bin, false);
    let output = mock_session_command(&repo, &repo.main, &bin)
        .args(["pr", "123", "--herdr"])
        .output()
        .unwrap();
    assert_target(&output, &repo.default_worktree("feature-pr"), "Feature/PR");
    assert_registration(&repo, &repo.default_worktree("feature-pr"));
}

#[cfg(unix)]
#[test]
fn bash_completion_offers_herdr_before_and_after_commands() {
    let repo = Repository::new();
    let output = repo.run(&repo.main, &["init", "bash"]);
    assert!(output.status.success());
    let script = repo.root.join("completion.bash");
    fs::write(&script, output.stdout).unwrap();
    let checks = r#"
source "$1"
for command in '' new add pr switch close remove list status init config setup; do
    COMP_WORDS=(copsy)
    if [[ -n "$command" ]]; then COMP_WORDS+=("$command"); fi
    COMP_WORDS+=(--her)
    COMP_CWORD=$((${#COMP_WORDS[@]}-1))
    COMPREPLY=()
    _copsy_bash
    [[ "${COMPREPLY[*]}" == --herdr ]] || exit 1
done
"#;
    let checked = Command::new("bash")
        .args(["-c", checks, "completion-test"])
        .arg(script)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
}
