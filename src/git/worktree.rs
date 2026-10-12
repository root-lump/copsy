//! Worktree discovery, layout, and Git mutations.

use super::{git_output, git_output_in, git_run};
use crate::config::WorktreeLayout;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
    pub is_bare: bool,
}

pub fn main_worktree_path() -> Result<PathBuf> {
    // Git lists the main worktree first, including a bare repository entry.
    list_worktrees()?
        .into_iter()
        .next()
        .map(|worktree| worktree.path)
        .context("Could not determine main worktree path")
}

pub fn list_worktrees() -> Result<Vec<WorktreeInfo>> {
    let stdout = git_output(&["worktree", "list", "--porcelain"])?;
    Ok(parse_worktrees(&stdout))
}

fn parse_worktrees(stdout: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch = String::new();
    let mut is_bare = false;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(prev_path) = current_path.take() {
                worktrees.push(WorktreeInfo {
                    path: prev_path,
                    branch: std::mem::take(&mut current_branch),
                    is_bare,
                });
                is_bare = false;
            }
            current_path = Some(PathBuf::from(path));
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            current_branch = branch_ref
                .strip_prefix("refs/heads/")
                .unwrap_or(branch_ref)
                .to_string();
        } else if line == "bare" {
            is_bare = true;
        }
    }
    if let Some(path) = current_path {
        worktrees.push(WorktreeInfo {
            path,
            branch: current_branch,
            is_bare,
        });
    }

    worktrees
}

fn worktree_parent_dir<'a>(main_worktree: &'a Path, base_dir: Option<&'a Path>) -> &'a Path {
    base_dir.unwrap_or_else(|| main_worktree.parent().unwrap_or(main_worktree))
}

/// Directory that holds every worktree of this repository in the nested layout.
///
/// Falls back to a `-worktrees` suffix when the repository-name directory would
/// be the main worktree itself: `git clone` checks out into a directory named
/// after the repository, so without `base_dir` the two collide and worktrees
/// would land inside the main worktree.
pub fn nested_repository_dir(
    repo_name: &str,
    main_worktree: &Path,
    base_dir: Option<&Path>,
) -> PathBuf {
    let parent = worktree_parent_dir(main_worktree, base_dir);
    let repository_dir = parent.join(repo_name);
    if repository_dir == main_worktree {
        parent.join(format!("{repo_name}-worktrees"))
    } else {
        repository_dir
    }
}

/// Absolute path of the worktree directory for `branch`.
///
/// Slashes in the branch name are flattened to `-` in both layouts, so the
/// directory tree is never deeper than the layout itself prescribes.
pub fn worktree_dir_name(
    repo_name: &str,
    main_worktree: &Path,
    branch: &str,
    base_dir: Option<&Path>,
    layout: WorktreeLayout,
) -> PathBuf {
    let sanitized = branch.replace('/', "-");
    match layout {
        WorktreeLayout::Flat => {
            worktree_parent_dir(main_worktree, base_dir).join(format!("{repo_name}-{sanitized}"))
        }
        WorktreeLayout::Nested => {
            nested_repository_dir(repo_name, main_worktree, base_dir).join(sanitized)
        }
    }
}

pub fn add_worktree(
    path: &Path,
    branch: &str,
    create_branch: bool,
    start_point: Option<&str>,
) -> Result<()> {
    let path_str = path.to_str().context("Invalid path")?;
    if create_branch {
        let mut args = vec!["worktree", "add", "-b", branch, path_str];
        if let Some(base) = start_point {
            args.push(base);
        }
        git_run(&args)
    } else {
        git_run(&["worktree", "add", path_str, branch])
    }
}

// Both run from the main worktree rather than the inherited current directory:
// the caller may be standing in the worktree being removed, and git aborts with
// "Unable to read current working directory" once that directory is gone.
// git_output (not git_run) keeps git's stdout out of the shell function's marker
// stream and folds its stderr into the error, so callers can report the reason.
pub fn remove_worktree(main_worktree: &Path, path: &Path, force: bool) -> Result<()> {
    let path_str = path.to_str().context("Invalid path")?;
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path_str);
    git_output_in(main_worktree, &args).map(|_| ())
}

pub fn delete_local_branch(main_worktree: &Path, branch: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    git_output_in(main_worktree, &["branch", flag, branch]).map(|_| ())
}

/// Find a worktree by branch name or directory basename.
pub fn find_worktree<'a, W: AsRef<WorktreeInfo>>(worktrees: &'a [W], name: &str) -> Option<&'a W> {
    worktrees.iter().find(|w| {
        let wt = w.as_ref();
        wt.branch == name
            || wt
                .path
                .file_name()
                .is_some_and(|n| n.to_string_lossy() == name)
    })
}

impl AsRef<WorktreeInfo> for WorktreeInfo {
    fn as_ref(&self) -> &WorktreeInfo {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::init_repository_with_worktree;
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn porcelain_keeps_order_and_resets_bare_and_branch_state() {
        let worktrees = parse_worktrees(
            "worktree /repo/main\nbare\n\n\
             worktree /repo/feature with spaces\nHEAD abc\nbranch refs/heads/feature/topic\nlocked reason\n\n\
             worktree /repo/detached\nHEAD def\ndetached\nprunable reason\n",
        );
        assert_eq!(worktrees.len(), 3);
        assert_eq!(worktrees[0].path, Path::new("/repo/main"));
        assert!(worktrees[0].is_bare);
        assert_eq!(worktrees[0].branch, "");
        assert_eq!(worktrees[1].path, Path::new("/repo/feature with spaces"));
        assert!(!worktrees[1].is_bare);
        assert_eq!(worktrees[1].branch, "feature/topic");
        assert_eq!(worktrees[2].path, Path::new("/repo/detached"));
        assert!(!worktrees[2].is_bare);
        assert_eq!(worktrees[2].branch, "");
        assert!(parse_worktrees("").is_empty());
    }

    #[test]
    fn worktree_dir_name_basic() {
        let main_worktree = Path::new("/home/user/myapp");
        let result = worktree_dir_name(
            "myapp",
            main_worktree,
            "feature/login",
            None,
            WorktreeLayout::Flat,
        );
        assert_eq!(result, Path::new("/home/user/myapp-feature-login"));
    }

    #[test]
    fn worktree_dir_name_with_base_dir() {
        let main_worktree = Path::new("/home/user/myapp");
        let base = Path::new("/tmp/worktrees");
        let result = worktree_dir_name(
            "myapp",
            main_worktree,
            "fix-typo",
            Some(base),
            WorktreeLayout::Flat,
        );
        assert_eq!(result, Path::new("/tmp/worktrees/myapp-fix-typo"));
    }

    #[test]
    fn worktree_dir_name_nested_slashes() {
        let main_worktree = Path::new("/repo");
        let result = worktree_dir_name(
            "repo",
            main_worktree,
            "feat/ui/header",
            None,
            WorktreeLayout::Flat,
        );
        assert_eq!(result, Path::new("/repo-feat-ui-header"));
    }

    #[test]
    fn worktree_dir_name_ignores_the_main_worktree_directory_name() {
        let main_worktree = Path::new("/home/user/myapp-main");
        let result = worktree_dir_name(
            "myapp",
            main_worktree,
            "feat/ui",
            None,
            WorktreeLayout::Flat,
        );
        assert_eq!(result, Path::new("/home/user/myapp-feat-ui"));
    }

    #[test]
    fn worktree_dir_name_nested_layout_with_base_dir() {
        let main_worktree = Path::new("/home/user/myapp");
        let base = Path::new("/tmp/worktrees");
        let result = worktree_dir_name(
            "myapp",
            main_worktree,
            "fix-typo",
            Some(base),
            WorktreeLayout::Nested,
        );
        assert_eq!(result, Path::new("/tmp/worktrees/myapp/fix-typo"));
    }

    #[test]
    fn worktree_dir_name_nested_layout_without_base_dir() {
        let main_worktree = Path::new("/home/user/myapp-main");
        let result = worktree_dir_name(
            "myapp",
            main_worktree,
            "fix-typo",
            None,
            WorktreeLayout::Nested,
        );
        assert_eq!(result, Path::new("/home/user/myapp/fix-typo"));
    }

    #[test]
    fn worktree_dir_name_nested_layout_flattens_branch_slashes() {
        let main_worktree = Path::new("/home/user/myapp-main");
        let result = worktree_dir_name(
            "myapp",
            main_worktree,
            "feat/ui/header",
            None,
            WorktreeLayout::Nested,
        );
        assert_eq!(result, Path::new("/home/user/myapp/feat-ui-header"));
    }

    #[test]
    fn nested_repository_dir_avoids_the_main_worktree() {
        let main_worktree = Path::new("/home/user/myapp");
        assert_eq!(
            nested_repository_dir("myapp", main_worktree, None),
            Path::new("/home/user/myapp-worktrees")
        );
        assert_eq!(
            worktree_dir_name(
                "myapp",
                main_worktree,
                "feat/sample",
                None,
                WorktreeLayout::Nested
            ),
            Path::new("/home/user/myapp-worktrees/feat-sample")
        );
    }

    #[test]
    fn nested_repository_dir_keeps_the_repository_name_when_nothing_collides() {
        let main_worktree = Path::new("/home/user/myapp-main");
        assert_eq!(
            nested_repository_dir("myapp", main_worktree, None),
            Path::new("/home/user/myapp")
        );
    }

    #[test]
    fn nested_repository_dir_avoids_a_main_worktree_directly_below_base_dir() {
        let main_worktree = Path::new("/tmp/worktrees/myapp");
        let base = Path::new("/tmp/worktrees");
        assert_eq!(
            nested_repository_dir("myapp", main_worktree, Some(base)),
            Path::new("/tmp/worktrees/myapp-worktrees")
        );
    }

    #[test]
    fn refuses_to_remove_a_dirty_worktree_unless_forced() {
        let root = tempdir().unwrap();
        let (main, linked) = init_repository_with_worktree(root.path());
        fs::write(linked.join("scratch.txt"), "work in progress").unwrap();

        assert!(remove_worktree(&main, &linked, false).is_err());
        assert!(linked.exists());

        remove_worktree(&main, &linked, true).unwrap();
        assert!(!linked.exists());
    }
}
