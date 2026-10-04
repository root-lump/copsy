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
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("__COPSY_CD__{}", target.display())
    );
    assert!(target.join(".git").is_file());
    let actual = git(target, &["branch", "--show-current"]);
    assert_eq!(String::from_utf8_lossy(&actual.stdout).trim(), branch);
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
    assert_target(
        &flat,
        &repo.root.join("remote-name-Feature-Flat"),
        "Feature/Flat",
    );
    fs::write(
        repo.main.join(".git/copsy.toml"),
        "[worktree]\nbase_dir = '~/nested'\nlayout = 'nested'\n",
    )
    .unwrap();
    let nested = repo.run(&repo.main, &["new", "Feature/Nested"]);
    assert_target(
        &nested,
        &repo.home.join("nested/remote-name/Feature-Nested"),
        "Feature/Nested",
    );
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
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let output = repo
        .command(&repo.main)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .args(["pr", "123", "--herdr"])
        .output()
        .unwrap();
    assert_target(&output, &repo.default_worktree("feature-pr"), "Feature/PR");
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
