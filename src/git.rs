use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
    pub is_bare: bool,
}

fn git_output(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("Failed to run: git {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn git_run(args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .status()
        .with_context(|| format!("Failed to run: git {}", args.join(" ")))?;
    if !status.success() {
        bail!("git {} failed", args.join(" "));
    }
    Ok(())
}

pub fn repo_root() -> Result<PathBuf> {
    git_output(&["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

// The first "worktree" entry in porcelain output is always the main worktree
pub fn main_worktree_path() -> Result<PathBuf> {
    let stdout = git_output(&["worktree", "list", "--porcelain"])?;
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            return Ok(PathBuf::from(path));
        }
    }
    bail!("Could not determine main worktree path")
}

pub fn list_worktrees() -> Result<Vec<WorktreeInfo>> {
    let stdout = git_output(&["worktree", "list", "--porcelain"])?;

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

    Ok(worktrees)
}

pub fn worktree_dir_name(repo_root: &Path, branch: &str, base_dir: Option<&Path>) -> PathBuf {
    let repo_name = repo_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());
    let sanitized = branch.replace('/', "-");
    let dir_name = format!("{repo_name}-{sanitized}");
    let parent = base_dir.unwrap_or_else(|| repo_root.parent().unwrap_or(repo_root));
    parent.join(dir_name)
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

pub fn remove_worktree(path: &Path) -> Result<()> {
    let path_str = path.to_str().context("Invalid path")?;
    git_run(&["worktree", "remove", path_str])
}

pub fn get_status(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(path)
        .output()
        .context("Failed to run git status")?;
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

pub fn list_branches() -> Result<Vec<String>> {
    let stdout = git_output(&["branch", "--format=%(refname:short)"])?;
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

pub fn list_remote_branches() -> Result<Vec<String>> {
    let stdout = git_output(&["branch", "-r", "--format=%(refname:short)"])?;
    Ok(stdout
        .lines()
        .filter(|s| !s.contains("HEAD"))
        .map(|s| s.strip_prefix("origin/").unwrap_or(s).to_string())
        .collect())
}

pub fn fetch_pr(target: &str) -> Result<String> {
    let pr_number = extract_pr_number(target)?;

    let output = Command::new("gh")
        .args([
            "pr",
            "view",
            &pr_number,
            "--json",
            "headRefName",
            "-q",
            ".headRefName",
        ])
        .output()
        .context("Failed to run gh. Is gh installed?")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to get PR info: {}", stderr.trim());
    }
    let branch = String::from_utf8(output.stdout)?.trim().to_string();
    if branch.is_empty() {
        bail!("Could not determine branch for PR {pr_number}");
    }

    // Try fast-forward fetch first; fall back to tracking-only fetch if the branch
    // already exists locally with divergent history
    if Command::new("git")
        .args(["fetch", "origin", &format!("{branch}:{branch}")])
        .status()
        .is_ok_and(|s| !s.success())
    {
        let _ = Command::new("git")
            .args(["fetch", "origin", &branch])
            .status();
    }

    Ok(branch)
}

pub(crate) fn extract_pr_number(target: &str) -> Result<String> {
    if target.chars().all(|c| c.is_ascii_digit()) {
        return Ok(target.to_string());
    }
    if let Some(num) = target.rsplit('/').next()
        && target.contains("/pull/")
        && num.chars().all(|c| c.is_ascii_digit())
    {
        return Ok(num.to_string());
    }
    bail!("Invalid PR target: {target}. Provide a PR number or URL.")
}

pub fn list_prs() -> Result<Vec<(String, String, String)>> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--json",
            "number,title,headRefName",
            "-q",
            ".[] | \"\\(.number)\\t\\(.title)\\t\\(.headRefName)\"",
        ])
        .output()
        .context("Failed to run gh. Is gh installed?")?;
    if !output.status.success() {
        bail!("Failed to list PRs");
    }
    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() == 3 {
                Some((
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                ))
            } else {
                None
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn worktree_dir_name_basic() {
        let root = Path::new("/home/user/myapp");
        let result = worktree_dir_name(root, "feature/login", None);
        assert_eq!(result, Path::new("/home/user/myapp-feature-login"));
    }

    #[test]
    fn worktree_dir_name_with_base_dir() {
        let root = Path::new("/home/user/myapp");
        let base = Path::new("/tmp/worktrees");
        let result = worktree_dir_name(root, "fix-typo", Some(base));
        assert_eq!(result, Path::new("/tmp/worktrees/myapp-fix-typo"));
    }

    #[test]
    fn worktree_dir_name_nested_slashes() {
        let root = Path::new("/repo");
        let result = worktree_dir_name(root, "feat/ui/header", None);
        assert_eq!(result, Path::new("/repo-feat-ui-header"));
    }

    #[test]
    fn extract_pr_number_plain_number() {
        assert_eq!(extract_pr_number("123").unwrap(), "123");
    }

    #[test]
    fn extract_pr_number_github_url() {
        let url = "https://github.com/owner/repo/pull/456";
        assert_eq!(extract_pr_number(url).unwrap(), "456");
    }

    #[test]
    fn extract_pr_number_invalid() {
        assert!(extract_pr_number("not-a-number").is_err());
        assert!(extract_pr_number("https://github.com/owner/repo/issues/123").is_err());
    }
}
