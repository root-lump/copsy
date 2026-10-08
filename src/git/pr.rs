//! Pull request discovery and fetching through the gh-selected remote.

use super::remote::{gh_default_remote, gh_remote};
use super::{git_output, git_run, git_succeeds};
use anyhow::{Context, Result, bail};
use std::process::Command;

fn check_gh_default_repo() -> Result<()> {
    let remotes = git_output(&["remote"])?;
    if remotes.lines().count() <= 1 {
        return Ok(());
    }
    if gh_default_remote().is_none() {
        bail!(
            "Multiple remotes found but no default repository has been set for gh.\n  \
             Run 'gh repo set-default' to select one."
        );
    }
    Ok(())
}

pub fn fetch_pr(target: &str) -> Result<String> {
    check_gh_default_repo()?;
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

    fetch_pr_branch(&pr_number, &branch)?;
    Ok(branch)
}

pub fn fetch_pr_branch(pr_number: &str, branch: &str) -> Result<()> {
    let remote = gh_remote().context("No git remote found to fetch the pull request from")?;

    // Try fetching the branch itself first — this sets up <remote>/<branch> so
    // `git worktree add` can auto-create a tracking branch.
    if !git_succeeds(&["fetch", &remote, branch]) {
        let pr_ref = format!("pull/{pr_number}/head:{branch}");
        git_run(&["fetch", &remote, &pr_ref])?;
    }

    // Set upstream tracking only when <remote>/<branch> exists (same-repo PRs)
    // and the local branch already exists (otherwise worktree add handles it).
    let remote_ref = format!("refs/remotes/{remote}/{branch}");
    let local_ref = format!("refs/heads/{branch}");
    if git_succeeds(&["rev-parse", "--verify", &remote_ref])
        && git_succeeds(&["rev-parse", "--verify", &local_ref])
    {
        let upstream = format!("{remote}/{branch}");
        let _ = Command::new("git")
            .args(["branch", "--set-upstream-to", &upstream, branch])
            .output();
    }

    Ok(())
}

fn extract_pr_number(target: &str) -> Result<String> {
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

#[derive(Debug, Eq, PartialEq)]
pub struct PullRequest {
    pub number: String,
    pub title: String,
    pub branch: String,
}

pub fn list_prs() -> Result<Vec<PullRequest>> {
    check_gh_default_repo()?;
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
    Ok(parse_prs(&stdout))
}

fn parse_prs(stdout: &str) -> Vec<PullRequest> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            Some(PullRequest {
                number: parts.next()?.to_string(),
                title: parts.next()?.to_string(),
                branch: parts.next()?.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_records_preserve_display_fields_and_skip_incomplete_rows() {
        let prs = parse_prs(
            "12\tTitle with spaces\tfeature/topic\ninvalid\n13\tmissing branch\n14\t\tfix\n",
        );
        assert_eq!(
            prs,
            vec![
                PullRequest {
                    number: "12".into(),
                    title: "Title with spaces".into(),
                    branch: "feature/topic".into(),
                },
                PullRequest {
                    number: "14".into(),
                    title: "".into(),
                    branch: "fix".into(),
                },
            ]
        );
        assert!(parse_prs("").is_empty());
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
