//! Repository identity, shared configuration, and ignored setup paths.

use super::git_output;
use super::git_output_in;
use super::remote::{primary_remote_in, repository_name_from_url};
use crate::info;
use crate::repository_path::RepositoryPath;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn repo_root() -> Result<PathBuf> {
    git_output(&["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

pub fn git_common_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_default();
    git_common_dir_in(&cwd)
}

fn git_common_dir_in(dir: &Path) -> Result<PathBuf> {
    git_output_in(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map(PathBuf::from)
}

pub fn repository_config_path() -> Result<PathBuf> {
    Ok(git_common_dir()?.join("copsy.toml"))
}

pub fn list_ignored_paths(main_worktree: &Path) -> Result<Vec<RepositoryPath>> {
    let output = Command::new("git")
        .args([
            "status",
            "--ignored",
            "--porcelain=v1",
            "--untracked-files=normal",
            "-z",
        ])
        .current_dir(main_worktree)
        .output()
        .context("Failed to list ignored files")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git status failed: {}", stderr.trim());
    }
    Ok(parse_ignored_paths(&output.stdout))
}

fn parse_ignored_paths(output: &[u8]) -> Vec<RepositoryPath> {
    let mut paths = Vec::new();
    for record in output.split(|byte| *byte == 0) {
        let Some(path) = record.strip_prefix(b"!! ") else {
            continue;
        };
        let Ok(path) = std::str::from_utf8(path) else {
            info!("Warning: skipped a non-UTF-8 ignored path");
            continue;
        };
        let path = path.trim_end_matches('/');
        if let Ok(path) = RepositoryPath::new(path) {
            paths.push(path);
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

/// Name that prefixes every worktree directory of this repository.
///
/// The remote name wins over the local directory name so that cloning into a
/// differently named directory (or renaming it later) does not change how
/// worktrees are named.
pub fn repository_name(main_worktree: &Path) -> String {
    repository_name_or_directory(remote_repository_name(main_worktree), main_worktree)
}

/// Herdr identifies repositories by their local Git common directory, not origin.
pub fn herdr_repository_name() -> Result<String> {
    let common_dir = git_common_dir()?;
    let common_dir = common_dir.canonicalize().unwrap_or(common_dir);
    let label_path = match common_dir.file_name().and_then(|name| name.to_str()) {
        Some(".git") => common_dir.parent().unwrap_or(&common_dir),
        Some(".bare") => {
            // Embedded bare clones may have a container .git file pointing at
            // .bare. A standalone .bare uses its own name in Herdr instead.
            common_dir
                .parent()
                .filter(|parent| {
                    git_output_in(parent, &["rev-parse", "--absolute-git-dir"])
                        .ok()
                        .map(PathBuf::from)
                        .is_some_and(|path| path.canonicalize().unwrap_or(path) == common_dir)
                })
                .unwrap_or(&common_dir)
        }
        _ => &common_dir,
    };
    Ok(label_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo")
        .to_string())
}

fn repository_name_or_directory(remote_name: Option<String>, main_worktree: &Path) -> String {
    remote_name.unwrap_or_else(|| {
        main_worktree
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string())
    })
}

fn remote_repository_name(main_worktree: &Path) -> Option<String> {
    let remote = primary_remote_in(main_worktree)?;
    let url = git_output_in(main_worktree, &["remote", "get-url", &remote]).ok()?;
    repository_name_from_url(&url)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::run_git;
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn repository_name_falls_back_to_the_main_worktree_directory() {
        let main_worktree = Path::new("/home/user/myapp-main");
        assert_eq!(
            repository_name_or_directory(None, main_worktree),
            "myapp-main"
        );
        assert_eq!(
            repository_name_or_directory(Some("myapp".to_string()), main_worktree),
            "myapp"
        );
    }

    #[test]
    fn repository_name_uses_a_remote_that_is_not_named_origin() {
        let repository = tempdir().unwrap();
        run_git(repository.path(), &["init", "-q"]);
        run_git(
            repository.path(),
            &[
                "remote",
                "add",
                "upstream",
                "https://example.com/org/myapp.git",
            ],
        );

        assert_eq!(repository_name(repository.path()), "myapp");
    }

    #[test]
    fn repository_name_prefers_origin_over_other_remotes() {
        let repository = tempdir().unwrap();
        run_git(repository.path(), &["init", "-q"]);
        run_git(
            repository.path(),
            &[
                "remote",
                "add",
                "origin",
                "https://example.com/org/canonical.git",
            ],
        );
        run_git(
            repository.path(),
            &[
                "remote",
                "add",
                "upstream",
                "https://example.com/org/other.git",
            ],
        );

        assert_eq!(repository_name(repository.path()), "canonical");
    }

    #[test]
    fn parses_ignored_paths_without_filtering_generated_directories() {
        let output = b"!! .env\0!! node_modules/\0!! target/\0?? untracked\0";
        assert_eq!(
            parse_ignored_paths(output),
            vec![
                RepositoryPath::new(".env").unwrap(),
                RepositoryPath::new("node_modules").unwrap(),
                RepositoryPath::new("target").unwrap(),
            ]
        );
    }

    #[test]
    fn ignored_paths_exclude_git_and_unsafe_paths() {
        let output = b"!! .git/\0!! nested/.git/config\0!! ../outside\0!! safe/file\0";
        assert_eq!(
            parse_ignored_paths(output),
            vec![RepositoryPath::new("safe/file").unwrap()]
        );
    }

    #[test]
    fn linked_worktrees_share_the_git_common_dir() {
        let root = tempdir().unwrap();
        let main = root.path().join("main");
        let linked = root.path().join("linked");
        fs::create_dir(&main).unwrap();
        run_git(&main, &["init", "-q"]);
        fs::write(main.join("README.md"), "test").unwrap();
        run_git(&main, &["add", "README.md"]);
        run_git(
            &main,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-qm",
                "initial",
            ],
        );
        run_git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature",
                linked.to_str().unwrap(),
            ],
        );

        assert_eq!(
            git_common_dir_in(&main).unwrap(),
            git_common_dir_in(&linked).unwrap()
        );
    }

    #[test]
    fn discovers_ignored_files_and_directories() {
        let repository = tempdir().unwrap();
        run_git(repository.path(), &["init", "-q"]);
        fs::write(repository.path().join(".gitignore"), ".env\n/target/\n").unwrap();
        fs::write(repository.path().join(".env"), "secret").unwrap();
        fs::create_dir(repository.path().join("target")).unwrap();
        fs::write(repository.path().join("target/cache"), "cache").unwrap();

        assert_eq!(
            list_ignored_paths(repository.path()).unwrap(),
            vec![
                RepositoryPath::new(".env").unwrap(),
                RepositoryPath::new("target").unwrap(),
            ]
        );
    }
}
