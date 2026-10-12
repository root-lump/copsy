//! Git subprocess boundary. Domain modules keep their parsing and policy private.

mod carry;
mod pr;
mod remote;
mod repository;
mod worktree;

pub use carry::{carry_stash, carry_unstash, get_status};
pub use pr::{fetch_pr, fetch_pr_branch, list_prs};
pub use remote::{list_branches, list_remote_branches};
pub use repository::{
    herdr_repository_name, list_ignored_paths, repo_root, repository_config_path, repository_name,
};
pub use worktree::{
    add_worktree, delete_local_branch, find_worktree, list_worktrees, main_worktree_path,
    nested_repository_dir, remove_worktree, worktree_dir_name,
};

use anyhow::{Context, Result, bail};
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

fn git_output_in(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("Failed to run: git {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn git_output(args: &[&str]) -> Result<String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    git_output_in(&cwd, args)
}

fn git_run(args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        // Git prints checkout summaries to stdout; keep the shell marker
        // channel clean while still displaying those messages to the user.
        .stdout(Stdio::from(io::stderr()))
        .status()
        .with_context(|| format!("Failed to run: git {}", args.join(" ")))?;
    if !status.success() {
        bail!("git {} failed", args.join(" "));
    }
    Ok(())
}

fn git_succeeds(args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(test)]
mod test_support {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    pub(super) fn init_repository_with_worktree(root: &Path) -> (PathBuf, PathBuf) {
        let main = root.join("main");
        let linked = root.join("linked");
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
        (main, linked)
    }

    pub(super) fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {} failed", args.join(" "));
    }
}
